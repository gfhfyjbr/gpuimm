//! Source-measured Glimm examples, drawn with native quads, text and image primitives.
//! Only photographs, SVG icons and decorative CSS gradients are image assets.

use gpui::{
    AnyElement, App, Bounds, BoxShadow, ContentMask, Corners, Edges, FontWeight, GlimmDemo, Hsla,
    Image, ImageFormat, IntoElement, ParentElement, Pixels, Point, RenderImage, Rgba, Styled,
    TextRun, canvas, div, fill, font, point, px, size,
};
use serde::Deserialize;
use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

#[path = "glimm_assets/images.rs"]
mod images;

pub const WIDTH: f32 = 634.;
pub const HEIGHT: f32 = 476.;

#[derive(Clone, Deserialize)]
pub struct Scene {
    pub scope: [f32; 4],
    pub nodes: Vec<Node>,
    pub controls: Vec<Control>,
    pub anchors: BTreeMap<String, [f32; 4]>,
    #[cfg(test)]
    #[serde(default, rename = "effectOrigin")]
    pub effect_origin: Option<[f32; 2]>,
}
#[derive(Clone, Deserialize)]
pub struct Control {
    pub label: String,
    pub bounds: [f32; 4],
    pub disabled: bool,
    pub role: String,
    pub checked: bool,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Node {
    pub kind: String,
    pub bounds: [f32; 4],
    pub clip: [f32; 4],
    pub opacity: f32,
    pub inside: bool,
    pub color: String,
    pub radii: [f32; 4],
    pub border: [f32; 4],
    pub border_color: String,
    pub shadow: String,
    pub text: String,
    pub font_size: f32,
    pub weight: f32,
    pub tracking: f32,
    pub asset: String,
}
#[derive(Clone, Copy)]
pub enum Layer {
    Before,
    Scope,
    After,
}

pub struct Scenes {
    pub states: BTreeMap<String, Arc<Scene>>,
    images: BTreeMap<String, Arc<RenderImage>>,
}
impl Scenes {
    pub fn load(cx: &mut App) -> Self {
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(include_bytes!(
                "glimm_assets/Inter.ttf"
            ))])
            .expect("original Inter font");
        let states: BTreeMap<String, Scene> =
            serde_json::from_str(include_str!("glimm_assets/scenes.json"))
                .expect("source-measured Glimm scene data");
        let images = images::IMAGES
            .iter()
            .map(|(name, bytes)| {
                let image = Image::from_bytes(ImageFormat::Png, bytes.to_vec())
                    .to_image_data(cx.svg_renderer())
                    .expect("original decorative asset");
                ((*name).to_owned(), image)
            })
            .collect();
        Self {
            states: states.into_iter().map(|(k, v)| (k, Arc::new(v))).collect(),
            images,
        }
    }
    pub fn get(&self, key: &str) -> Arc<Scene> {
        self.states
            .get(key)
            .unwrap_or_else(|| panic!("missing native scene {key}"))
            .clone()
    }
    pub fn picture(
        &self,
        scene: Arc<Scene>,
        layer: Layer,
        opacity: f32,
        countdown: Option<u8>,
    ) -> AnyElement {
        let first = scene
            .nodes
            .iter()
            .position(|n| n.inside)
            .unwrap_or(scene.nodes.len());
        let offset = if matches!(layer, Layer::Scope) {
            [scene.scope[0], scene.scope[1]]
        } else {
            [0., 0.]
        };
        let dimensions = if matches!(layer, Layer::Scope) {
            [scene.scope[2], scene.scope[3]]
        } else {
            [WIDTH, HEIGHT]
        };
        let mut picture = div()
            .relative()
            .w(px(dimensions[0]))
            .h(px(dimensions[1]))
            .opacity(opacity);
        for (ix, node) in scene.nodes.iter().enumerate() {
            let include = match layer {
                Layer::Before => !node.inside && ix < first,
                Layer::Scope => node.inside,
                Layer::After => !node.inside && ix >= first,
            };
            if !include {
                continue;
            }
            let mut node = node.clone();
            if let Some(seconds) = countdown {
                if node.text.starts_with("0:") {
                    node.text = format!("0:{seconds:02}");
                }
            }
            let image = self.images.get(&node.asset).cloned();
            let b = node.bounds;
            let node_opacity = node.opacity;
            let text_node = node.clone();
            picture = picture.child(
                div()
                    .absolute()
                    .left(px(b[0] - offset[0]))
                    .top(px(b[1] - offset[1]))
                    .w(px(b[2]))
                    .h(px(b[3]))
                    .opacity(node_opacity)
                    .child(
                        canvas(
                            move |_, window, _| {
                                if text_node.kind != "text" {
                                    return None;
                                }
                                let mut face = font("Inter");
                                face.weight = FontWeight(text_node.weight);
                                let text: gpui::SharedString = text_node.text.clone().into();
                                let run = TextRun {
                                    len: text.len(),
                                    font: face,
                                    color: css_color(&text_node.color),
                                    ..TextRun::default()
                                };
                                Some(window.text_system().shape_line(
                                    text,
                                    px(text_node.font_size),
                                    &[run],
                                    None,
                                ))
                            },
                            move |bounds, line, window, _| {
                                let origin = bounds.origin - point(px(b[0]), px(b[1]));
                                let clip = translated(node.clip, origin);
                                window.with_content_mask(
                                    Some(ContentMask { bounds: clip }),
                                    |window| match node.kind.as_str() {
                                        "box" => {
                                            let corners = corners(node.radii);
                                            let shadows = css_shadows(&node.shadow);
                                            window.paint_drop_shadows(bounds, corners, &shadows);
                                            let mut quad = fill(bounds, css_color(&node.color));
                                            quad.corner_radii = corners;
                                            quad.border_widths = Edges {
                                                top: px(node.border[0]),
                                                right: px(node.border[1]),
                                                bottom: px(node.border[2]),
                                                left: px(node.border[3]),
                                            };
                                            quad.border_color = css_color(&node.border_color);
                                            window.paint_quad(quad);
                                            window.paint_inset_shadows(bounds, corners, &shadows);
                                        }
                                        "text" => {
                                            if let Some(line) = line {
                                                let baseline = bounds.origin
                                                    + point(
                                                        px(0.),
                                                        (px(b[3]) - line.ascent - line.descent)
                                                            / 2.
                                                            + line.ascent,
                                                    );
                                                for run in &line.runs {
                                                    for glyph in &run.glyphs {
                                                        let tracking = px(node.tracking
                                                            * node.text[..glyph.index]
                                                                .chars()
                                                                .count()
                                                                as f32);
                                                        let origin = baseline
                                                            + glyph.position
                                                            + point(tracking, px(0.));
                                                        let result = if glyph.is_emoji {
                                                            window.paint_emoji(
                                                                origin,
                                                                run.font_id,
                                                                glyph.id,
                                                                line.font_size,
                                                            )
                                                        } else {
                                                            window.paint_glyph(
                                                                origin,
                                                                run.font_id,
                                                                glyph.id,
                                                                line.font_size,
                                                                css_color(&node.color),
                                                            )
                                                        };
                                                        if let Err(error) = result {
                                                            log::error!("Glimm text: {error}");
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        "image" => {
                                            if let Some(image) = image {
                                                if let Err(error) = window.paint_image(
                                                    bounds,
                                                    bounds,
                                                    0f32.into(),
                                                    image,
                                                    0,
                                                    false,
                                                ) {
                                                    log::error!("Glimm image: {error}");
                                                }
                                            }
                                        }
                                        _ => {}
                                    },
                                );
                            },
                        )
                        .size_full(),
                    ),
            );
        }
        picture.into_any_element()
    }
}
pub fn route(demo: GlimmDemo) -> &'static str {
    match demo {
        GlimmDemo::Generate => "sheet",
        GlimmDemo::Teammate => "join",
        GlimmDemo::Invoice => "invoice",
        GlimmDemo::Upgrade => "upgrade",
        GlimmDemo::UsageReset => "reset",
        GlimmDemo::Publish => "publish",
        GlimmDemo::Autopilot => "autopilot",
        GlimmDemo::Theme => "theme",
        GlimmDemo::Tasks => "tasks",
    }
}
pub fn translated(b: [f32; 4], offset: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(offset + point(px(b[0]), px(b[1])), size(px(b[2]), px(b[3])))
}
fn corners([tl, tr, br, bl]: [f32; 4]) -> Corners<Pixels> {
    Corners {
        top_left: px(tl),
        top_right: px(tr),
        bottom_right: px(br),
        bottom_left: px(bl),
    }
}
fn css_color(value: &str) -> Hsla {
    let channels = value
        .split(['(', ')', ',', ' '])
        .filter_map(|s| s.parse::<f32>().ok())
        .collect::<Vec<_>>();
    if channels.len() < 3 {
        return gpui::transparent_black();
    }
    let [r, g, b] = if value.starts_with("oklab") {
        let [l, a, b] = [channels[0], channels[1], channels[2]];
        let ll = (l + 0.39633778 * a + 0.21580376 * b).powi(3);
        let mm = (l - 0.105561346 * a - 0.06385417 * b).powi(3);
        let ss = (l - 0.08948418 * a - 1.2914855 * b).powi(3);
        [
            4.0767417 * ll - 3.3077116 * mm + 0.23096994 * ss,
            -1.268438 * ll + 2.6097574 * mm - 0.3413194 * ss,
            -0.0041960863 * ll - 0.7034186 * mm + 1.7076147 * ss,
        ]
        .map(|v| {
            if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1. / 2.4) - 0.055
            }
        })
    } else {
        [channels[0] / 255., channels[1] / 255., channels[2] / 255.]
    };
    Rgba {
        r: r.clamp(0., 1.),
        g: g.clamp(0., 1.),
        b: b.clamp(0., 1.),
        a: channels.get(3).copied().unwrap_or(1.),
    }
    .into()
}
fn css_shadows(value: &str) -> Vec<BoxShadow> {
    let mut depth = 0;
    let mut start = 0;
    let mut parts = Vec::new();
    for (ix, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&value[start..ix]);
                start = ix + 1;
            }
            _ => {}
        }
    }
    parts.push(&value[start..]);
    parts
        .into_iter()
        .filter_map(|part| {
            let color_start = part.find("rgb")?;
            let color_end = color_start + part[color_start..].find(')')? + 1;
            let color = css_color(&part[color_start..color_end]);
            if color.a == 0. {
                return None;
            }
            let lengths = part[color_end..]
                .split_whitespace()
                .filter_map(|s| s.strip_suffix("px").and_then(|s| s.parse::<f32>().ok()))
                .collect::<Vec<_>>();
            (lengths.len() == 4).then(|| BoxShadow {
                color,
                offset: point(px(lengths[0]), px(lengths[1])),
                blur_radius: px(lengths[2]),
                spread_radius: px(lengths[3]),
                inset: part.contains("inset"),
            })
        })
        .rev()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Scene, css_color};
    use std::collections::BTreeMap;

    #[test]
    fn original_scope_boundaries_and_content() {
        let states: BTreeMap<String, Scene> =
            serde_json::from_str(include_str!("glimm_assets/scenes.json")).unwrap();
        let sheet = &states["sheet-final"];
        assert_eq!(sheet.scope, [0., 116., 634., 360.]);
        assert_eq!(sheet.anchors["composer"], [12., 354., 610., 110.]);
        assert!(
            sheet
                .nodes
                .iter()
                .any(|n| n.inside && n.text == "Northwind Labs")
        );
        assert!(sheet.nodes.iter().any(|n| !n.inside && n.text == "Clear"));
        for name in [
            "join",
            "invoice",
            "upgrade",
            "reset",
            "publish",
            "autopilot",
            "theme",
            "tasks",
        ] {
            assert_eq!(
                states[&format!("{name}-initial")].scope,
                [0., 0., 634., 476.]
            );
        }
        assert!(
            states["invoice-final"]
                .nodes
                .iter()
                .any(|n| !n.inside && n.text == "Your invoice has been paid")
        );
        assert!(
            states["join-final"]
                .nodes
                .iter()
                .any(|n| !n.inside && n.text == "Tom Becker")
        );
        for mask in 0u8..32 {
            assert_eq!(
                states[&format!("tasks-{mask}")]
                    .controls
                    .iter()
                    .filter(|c| c.checked)
                    .count(),
                mask.count_ones() as usize
            );
        }
    }
    #[test]
    fn original_css_alpha_is_preserved() {
        assert_eq!(css_color("rgba(0, 0, 0, 0.08)").a, 0.08);
        assert_eq!(css_color("transparent").a, 0.);
        let blue: gpui::Rgba = css_color("oklab(0.602745 -0.0473846 -0.212476 / 0.1)").into();
        assert!((blue.g - 122. / 255.).abs() < 0.001);
        assert!((blue.b - 1.).abs() < 0.001);
        assert_eq!(
            super::css_shadows(
                "rgba(0, 0, 0, 0.12) 0px 8px 28px 0px, rgba(0, 0, 0, 0.06) 0px 0px 0px 1px"
            )
            .len(),
            2
        );
    }
}

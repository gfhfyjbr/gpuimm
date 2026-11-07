//! The original examples' small CSS entrances, independent of their scoped shader.
use super::scenes::{Control, HEIGHT, Node, Scene, WIDTH};
use gpui::GlimmEasing;
use std::sync::Arc;

#[path = "glimm_assets/motion.rs"]
mod reference;

fn ease_out(t: f32) -> f32 {
    GlimmEasing::CubicBezier([0., 0., 0.58, 1.]).sample(t.clamp(0., 1.))
}
fn soft(t: f32) -> f32 {
    GlimmEasing::CubicBezier([0.22, 1., 0.36, 1.]).sample(t.clamp(0., 1.))
}
fn spring(t: f32) -> f32 {
    if t >= 1. {
        return 1.;
    }
    let samples = reference::TOAST_SPRING;
    let position = t.max(0.) * (samples.len() - 1) as f32;
    let index = position.floor() as usize;
    let a = samples[index];
    let b = samples[(index + 1).min(samples.len() - 1)];
    a + (b - a) * (position - index as f32)
}
fn within(b: [f32; 4], parent: [f32; 4]) -> bool {
    b[0] >= parent[0] - 0.1
        && b[1] >= parent[1] - 0.1
        && b[0] + b[2] <= parent[0] + parent[2] + 0.1
        && b[1] + b[3] <= parent[1] + parent[3] + 0.1
}
fn transform_rect(b: &mut [f32; 4], pivot: [f32; 2], scale: f32, dy: f32) {
    b[0] = pivot[0] + (b[0] - pivot[0]) * scale;
    b[1] = pivot[1] + (b[1] - pivot[1]) * scale + dy;
    b[2] *= scale;
    b[3] *= scale;
}
fn transform(node: &mut Node, pivot: [f32; 2], scale: f32, dy: f32) {
    transform_rect(&mut node.bounds, pivot, scale, dy);
    if node.clip != [0., 0., WIDTH, HEIGHT] {
        transform_rect(&mut node.clip, pivot, scale, dy);
    }
    node.radii = node.radii.map(|v| v * scale);
    node.border = node.border.map(|v| v * scale);
    node.font_size *= scale;
    node.tracking *= scale;
}
fn transform_controls(
    controls: &mut [Control],
    bounds: [f32; 4],
    pivot: [f32; 2],
    scale: f32,
    dy: f32,
) {
    for control in controls {
        if within(control.bounds, bounds) {
            transform_rect(&mut control.bounds, pivot, scale, dy);
        }
    }
}

pub fn invoice(scene: Arc<Scene>, seconds: f32) -> Arc<Scene> {
    if seconds >= 1. {
        return scene;
    }
    let mut scene = (*scene).clone();
    let toast = scene
        .nodes
        .iter()
        .find(|n| {
            !n.inside
                && n.shadow != "none"
                && !n.shadow.is_empty()
                && n.bounds[3] > 40.
                && n.bounds[3] < 100.
        })
        .map(|n| n.bounds);
    for node in &mut scene.nodes {
        if node.inside {
            continue;
        }
        if let Some(bounds) = toast.filter(|b| within(node.bounds, *b)) {
            let root = node.bounds == bounds;
            let p = spring(seconds / 0.75);
            node.opacity *= ease_out(seconds / 0.22)
                * if root {
                    1.
                } else {
                    soft((seconds - 0.11) / 0.28)
                };
            transform(
                node,
                [bounds[0] + bounds[2] / 2., bounds[1]],
                0.92 + 0.08 * p,
                -(bounds[3] + 28.) * (1. - p),
            );
        } else if node.kind == "image" && node.bounds[2] > WIDTH {
            node.opacity *= GlimmEasing::Ease.sample((seconds / 0.7).clamp(0., 1.));
            if node.bounds[1] > 0. {
                let b = node.bounds;
                let scale = (0.9 + 0.18 * soft(seconds / 0.9)) / 1.08;
                transform(node, [b[0] + b[2] / 2., b[1] + b[3] / 2.], scale, 0.);
            }
        }
    }
    if let Some(b) = toast {
        let p = spring(seconds / 0.75);
        transform_controls(
            &mut scene.controls,
            b,
            [b[0] + b[2] / 2., b[1]],
            0.92 + 0.08 * p,
            -(b[3] + 28.) * (1. - p),
        );
    }
    Arc::new(scene)
}

pub fn popover(scene: Arc<Scene>, seconds: f32) -> Arc<Scene> {
    if seconds >= 0.6 {
        return scene;
    }
    let Some(bounds) = scene
        .nodes
        .iter()
        .find(|n| {
            !n.inside
                && n.bounds[1] >= 52.
                && n.bounds[2] > 150.
                && !n.shadow.is_empty()
                && n.shadow != "none"
        })
        .map(|n| n.bounds)
    else {
        return scene;
    };
    let mut scene = (*scene).clone();
    let p = spring(seconds / 0.5);
    let pivot = [bounds[0] + bounds[2], bounds[1]];
    for node in &mut scene.nodes {
        if !node.inside && within(node.bounds, bounds) {
            let root = node.bounds == bounds;
            node.opacity *= ease_out(seconds / 0.2)
                * if root {
                    1.
                } else {
                    soft((seconds - 0.08) / 0.24)
                };
            transform(node, pivot, 0.94 + 0.06 * p, -6. * (1. - p));
        }
    }
    transform_controls(
        &mut scene.controls,
        bounds,
        pivot,
        0.94 + 0.06 * p,
        -6. * (1. - p),
    );
    Arc::new(scene)
}

pub fn joined_faces(initial: &Scene, scene: Arc<Scene>, seconds: f32) -> Arc<Scene> {
    if seconds >= 1.2 {
        return scene;
    }
    let mut scene = (*scene).clone();
    let mut nodes = Vec::with_capacity(scene.nodes.len() + 4);
    for mut node in scene.nodes {
        if !node.inside && node.kind == "image" && node.bounds[0] > 400. && node.bounds[1] < 52. {
            if let Some(old) = initial
                .nodes
                .iter()
                .find(|n| n.kind == "image" && n.bounds == node.bounds)
            {
                if old.asset != node.asset {
                    let p = GlimmEasing::Ease.sample((seconds / 0.8).clamp(0., 1.));
                    let mut old = old.clone();
                    old.opacity *= 1. - p;
                    nodes.push(old);
                    node.opacity *= p;
                }
            } else {
                for old in initial
                    .nodes
                    .iter()
                    .filter(|n| n.kind == "image" && within(n.bounds, node.bounds))
                {
                    let mut old = old.clone();
                    old.opacity *= 1. - GlimmEasing::Ease.sample((seconds / 0.6).clamp(0., 1.));
                    nodes.push(old);
                }
                let b = node.bounds;
                let p = soft(seconds / 1.2);
                node.opacity *= GlimmEasing::Ease.sample((seconds / 0.9).clamp(0., 1.));
                transform(
                    &mut node,
                    [b[0] + b[2] / 2., b[1] + b[3] / 2.],
                    0.8 + 0.2 * p,
                    2. * (1. - p),
                );
            }
        }
        nodes.push(node);
    }
    scene.nodes = nodes;
    Arc::new(scene)
}

#[cfg(test)]
mod tests {
    use super::{ease_out, spring};
    #[test]
    fn source_spring_has_its_original_overshoot_and_settles() {
        assert_eq!(spring(0.), 0.);
        assert_eq!(spring(1.), 1.);
        assert!(spring(18. / 40.) > 1.019);
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
    }
}

use super::palette::GlimmPalette;
use serde::{Deserialize, Serialize};

/// Sweep travel direction, in top-down view coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GlimmDirection {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
    /// Top to bottom.
    #[default]
    Ttb,
    /// Bottom to top.
    Btt,
}
impl GlimmDirection {
    /// All four supported directions.
    pub const ALL: [Self; 4] = [Self::Ltr, Self::Rtl, Self::Ttb, Self::Btt];
    /// Unit travel vector, in top-down coordinates.
    pub fn vector(self) -> [f32; 2] {
        match self {
            Self::Ltr => [1., 0.],
            Self::Rtl => [-1., 0.],
            Self::Ttb => [0., 1.],
            Self::Btt => [0., -1.],
        }
    }
    /// Whether the travel axis is horizontal.
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Ltr | Self::Rtl)
    }
}

/// The ten easing curves in glimm, plus custom cubic Beziers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlimmEasing {
    /// Constant speed.
    Linear,
    /// CSS ease.
    #[default]
    Ease,
    /// Holds, then accelerates.
    Snap,
    /// Fast start with an overshoot.
    Back,
    /// Quartic deceleration.
    EaseOutQuart,
    /// Cubic deceleration.
    EaseOutCubic,
    /// Cubic acceleration.
    EaseInCubic,
    /// Cubic ease in/out.
    EaseInOutCubic,
    /// Exponential deceleration.
    EaseOutExpo,
    /// Quintic ease in/out.
    EaseInOutQuint,
    /// CSS-compatible `[x1, y1, x2, y2]` control points.
    CubicBezier([f32; 4]),
}
impl GlimmEasing {
    /// All named reference curves.
    pub const ALL: [Self; 10] = [
        Self::Snap,
        Self::Linear,
        Self::Ease,
        Self::EaseOutQuart,
        Self::EaseOutCubic,
        Self::EaseInCubic,
        Self::EaseInOutCubic,
        Self::EaseOutExpo,
        Self::EaseInOutQuint,
        Self::Back,
    ];
    /// Evaluate a curve. The shader traversal clamps overshoot, as glimm does.
    pub fn sample(self, p: f32) -> f32 {
        let p = finite(p, 0., 0., 1.);
        match self {
            Self::Linear => p,
            Self::EaseOutQuart => 1. - (1. - p).powi(4),
            Self::EaseOutCubic => 1. - (1. - p).powi(3),
            Self::EaseInCubic => p * p * p,
            Self::EaseInOutCubic => {
                if p < 0.5 {
                    4. * p * p * p
                } else {
                    1. - (-2. * p + 2.).powi(3) / 2.
                }
            }
            Self::EaseInOutQuint => {
                if p < 0.5 {
                    16. * p.powi(5)
                } else {
                    1. - (-2. * p + 2.).powi(5) / 2.
                }
            }
            Self::EaseOutExpo => {
                if p == 1. {
                    1.
                } else {
                    1. - 2f32.powf(-10. * p)
                }
            }
            Self::Snap => bezier([1., 0., 0.35, 0.95], p),
            Self::Ease => bezier([0.25, 0.1, 0.25, 1.], p),
            Self::Back => bezier([0.175, 0.885, 0.32, 1.1], p),
            Self::CubicBezier(c) => bezier(c, p),
        }
    }
}
fn bezier([x1, y1, x2, y2]: [f32; 4], x: f32) -> f32 {
    if x <= 0. {
        return 0.;
    }
    if x >= 1. {
        return 1.;
    }
    let value = |t: f32, a: f32, b: f32| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t * t * b + t * t * t
    };
    let mut lo = 0.;
    let mut hi = 1.;
    let mut t = x;
    for _ in 0..20 {
        let error = value(t, x1, x2) - x;
        if error.abs() < 1e-6 {
            break;
        }
        if error < 0. {
            lo = t;
        } else {
            hi = t;
        }
        let derivative =
            3. * (1. - 4. * t + 3. * t * t) * x1 + 3. * (2. * t - 3. * t * t) * x2 + 3. * t * t;
        let next = t - error / derivative;
        t = if next.is_finite() && next > lo && next < hi {
            next
        } else {
            (lo + hi) / 2.
        };
    }
    value(t, y1, y2)
}

/// What a run draws. `Band` and `Scope` are the two paths of glimm.dev; the others are radial
/// distortions of the scope's content around the point of action (`at`, or `origin` with
/// `origin_offset`), which may lie outside the scope.
///
/// Calling `anticipate` with a radial effect holds a breathing lens around the point until the
/// work resolves, then releases into the effect; `cancel` lets the lens relax instead.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlimmEffect {
    /// The regular premultiplied iridescent sweep.
    Band,
    /// Gather, progressive blur, content ripple and old/new dissolve.
    #[default]
    Scope,
    /// New content comes out of the point as if pulled through a membrane: squeezed into a
    /// growing bubble, refracted and dispersed at its rim, relaxing into place behind it, while
    /// the old content ahead of the rim is pushed aside and ripples.
    Emerge,
    /// The content stays; a ring of refraction runs out from the point across it, weakening
    /// with distance, with an optional second ring.
    Shock,
    /// The reverse of `Emerge`: the old content is drawn into the point and gone, and what
    /// replaces it is pulled in after it.
    Collapse,
}
impl GlimmEffect {
    /// Whether this is one of the radial distortions rather than a glimm.dev sweep.
    pub fn is_radial(self) -> bool {
        matches!(self, Self::Emerge | Self::Shock | Self::Collapse)
    }
    /// The shader's mode number: 0 for the glimm.dev paths.
    pub(super) fn mode(self) -> f32 {
        match self {
            Self::Band | Self::Scope => 0.,
            Self::Emerge => 1.,
            Self::Shock => 2.,
            Self::Collapse => 3.,
        }
    }
}

/// Serializable settings. Names and defaults follow glimm 0.4.1's GlimmScope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GlimmOptions {
    /// Band-only or content deformation.
    pub effect: GlimmEffect,
    /// Cosine colour palette, including custom coefficients.
    pub palette: GlimmPalette,
    /// Travel axis and sign.
    pub direction: GlimmDirection,
    /// Band traversal easing. Scope uses the reference's fixed power-2.4 curve.
    pub easing: GlimmEasing,
    /// Sweep traversal duration, in milliseconds.
    pub sweep_ms: f32,
    /// Band-only alpha outro duration, in milliseconds.
    pub outro_ms: f32,
    /// Band-only midpoint event threshold (eased progress).
    pub midpoint: f32,
    /// Anticipation gather duration, in milliseconds.
    pub gather_ms: f32,
    /// Maximum band alpha, in 0..1.5.
    pub peak_alpha: f32,
    /// Colour multiplier, in 0..1.5.
    pub brightness: f32,
    /// Gaussian band tightness, in 0.1..200.
    pub band_tight: f32,
    /// Edge displacement, in 0..2.
    pub wave_amount: f32,
    /// Foil texture ripple, in 0..2.
    pub ripple_amount: f32,
    /// Internal shader time multiplier, in 0..3.
    pub wave_speed: f32,
    /// Crest highlights and Fresnel intensity, in 0..1.
    pub swell_amount: f32,
    /// Parabolic cross-axis lag, as a fraction of travel length.
    pub curve: f32,
    /// Fraction of the cross axis covered, in 0.05..1.
    pub span: f32,
    /// Normalised origin, in top-down scope coordinates.
    pub origin: [f32; 2],
    /// Origin offset, in logical pixels.
    pub origin_offset: [f32; 2],
    /// Optional point of action in scope-local logical pixels.
    pub at: Option<[f32; 2]>,
    /// Content displacement and blur multiplier.
    pub intensity: f32,
    /// Optional progress interval in which the effect fades and ends.
    pub fade: Option<[f32; 2]>,
    /// Logical-pixel band edge feather; None uses glimm's automatic 24px cap.
    pub feather: Option<f32>,
    /// Override viewport-sized treatment when a full-page demo is embedded in a larger window.
    /// None compares the scope to the native window, as the browser compares it to its viewport.
    pub full_bleed: Option<bool>,
    /// Disable content displacement without disabling the colour band.
    pub page_effect: bool,
    /// Stable palette phase, in 0..0.4; explicit for reproducible captures.
    pub hue_shift: f32,
    /// Radial effects: how far the wavefront's refraction moves the content, in logical pixels,
    /// before `intensity`; 0..96.
    pub amplitude: f32,
    /// Radial effects: width of the wavefront ring, in logical pixels; 4..600.
    pub ring_width: f32,
    /// Radial effects: distance between the ripples trailing the front, in logical pixels;
    /// 8..600.
    pub wavelength: f32,
    /// Radial effects: how long the train of trailing ripples is, in wavelengths; 0 leaves the
    /// front alone. 0..6.
    pub ripples: f32,
    /// Shock: strength of a second ring that follows the first at six tenths of its radius,
    /// relative to the first; 0..1.
    pub echo: f32,
    /// Radial effects: distance from the point at which the wave has lost half its strength,
    /// in logical pixels; 16..100000.
    pub falloff: f32,
    /// Radial effects: how far the colours of the refracted light separate, relative to the
    /// refraction itself; 0 keeps them together. 0..2.
    pub chroma: f32,
    /// Radial effects: how much the palette colours the refracted light, in its dispersion and
    /// in a rim along the front; 0 is uncoloured refraction. 0..1.
    pub tint: f32,
    /// Radial effects: blur per unit of stretch, so squeezed and stretched content softens with
    /// the warp; 0..2.
    pub stretch_blur: f32,
    /// Emerge and Collapse: share of the content's distance to the scope's edge that is pulled
    /// into the bubble, so the content inside is squeezed toward the point; 0..1.
    pub squeeze: f32,
    /// Emerge and Collapse: how much of the bubble's area pushes the content outside it aside
    /// (Emerge) or pulls it in after it (Collapse); 0..0.9.
    pub push: f32,
    /// Radial effects while `anticipate` waits: strength of the breathing lens; 0..2.
    pub hold_amount: f32,
    /// Radial effects while `anticipate` waits: radius of the breathing lens, in logical pixels;
    /// 8..2000.
    pub hold_radius: f32,
    /// Radial effects while `anticipate` waits: one breath, in milliseconds; 200..10000.
    pub breath_ms: f32,
    /// Radial effects: opacity of the iridescent band drawn as a ring on the front, an optional
    /// layer over the refraction; 0 draws none. 0..1.5.
    pub band_overlay: f32,
}
impl Default for GlimmOptions {
    fn default() -> Self {
        Self {
            effect: GlimmEffect::Scope,
            palette: GlimmPalette::default(),
            direction: GlimmDirection::Ttb,
            easing: GlimmEasing::Ease,
            sweep_ms: 1600.,
            outro_ms: 220.,
            midpoint: 0.56,
            gather_ms: 620.,
            peak_alpha: 1.1,
            brightness: 1.15,
            band_tight: 28.,
            wave_amount: 0.,
            ripple_amount: 1.,
            wave_speed: 1.,
            swell_amount: 0.15,
            curve: 0.,
            span: 1.,
            origin: [0.5, 0.],
            origin_offset: [0., -12.],
            at: None,
            intensity: 1.,
            fade: None,
            feather: None,
            full_bleed: None,
            page_effect: true,
            hue_shift: 0.,
            amplitude: 18.,
            ring_width: 44.,
            wavelength: 60.,
            ripples: 2.5,
            echo: 0.,
            falloff: 900.,
            chroma: 0.6,
            tint: 0.45,
            stretch_blur: 1.,
            squeeze: 0.7,
            push: 0.4,
            hold_amount: 1.,
            hold_radius: 90.,
            breath_ms: 1400.,
            band_overlay: 0.,
        }
    }
}
impl GlimmOptions {
    /// The original provider/overlay defaults, rather than GlimmScope defaults.
    pub fn band() -> Self {
        Self {
            effect: GlimmEffect::Band,
            direction: GlimmDirection::Ltr,
            sweep_ms: 1100.,
            peak_alpha: 1.,
            brightness: 1.,
            band_tight: 14.,
            swell_amount: 0.55,
            origin: [0.5, 0.5],
            origin_offset: [0., 0.],
            feather: Some(0.),
            ..Self::default()
        }
    }
    /// New content pulled out of the point of action through a membrane, the old content
    /// around it pushed aside. Set `at` to the point, in scope pixels; it may lie outside the
    /// scope, for example on the edge of the element the content comes from.
    pub fn emerge() -> Self {
        Self {
            effect: GlimmEffect::Emerge,
            easing: GlimmEasing::CubicBezier([0.33, 0.15, 0.55, 1.]),
            sweep_ms: 1150.,
            origin: [0.5, 0.5],
            origin_offset: [0., 0.],
            ..Self::default()
        }
    }
    /// A ring of refraction crossing unchanged content, with a weaker second ring: something
    /// landed at the point, or finished there.
    pub fn shock() -> Self {
        Self {
            effect: GlimmEffect::Shock,
            easing: GlimmEasing::CubicBezier([0.3, 0.3, 0.6, 1.]),
            sweep_ms: 1200.,
            origin: [0.5, 0.5],
            origin_offset: [0., 0.],
            amplitude: 14.,
            ring_width: 36.,
            wavelength: 48.,
            ripples: 1.2,
            echo: 0.45,
            falloff: 1000.,
            ..Self::default()
        }
    }
    /// The old content drawn into the point of action and gone.
    pub fn collapse() -> Self {
        Self {
            effect: GlimmEffect::Collapse,
            easing: GlimmEasing::CubicBezier([0.4, 0.15, 0.6, 1.]),
            sweep_ms: 950.,
            origin: [0.5, 0.5],
            origin_offset: [0., 0.],
            squeeze: 0.85,
            push: 0.3,
            ..Self::default()
        }
    }
    /// Replace invalid and unbounded inputs before they reach the GPU.
    pub fn validated(mut self) -> Self {
        self.sweep_ms = finite(self.sweep_ms, 1600., 0., 3_600_000.);
        self.outro_ms = finite(self.outro_ms, 220., 0., 3_600_000.);
        self.gather_ms = finite(self.gather_ms, 620., 0., 3_600_000.);
        self.midpoint = finite(self.midpoint, 0.56, 0., 1.);
        self.peak_alpha = finite(self.peak_alpha, 1.1, 0., 1.5);
        self.brightness = finite(self.brightness, 1.15, 0., 1.5);
        self.band_tight = finite(self.band_tight, 28., 0.1, 200.);
        self.wave_amount = finite(self.wave_amount, 0., 0., 2.);
        self.ripple_amount = finite(self.ripple_amount, 1., 0., 2.);
        self.wave_speed = finite(self.wave_speed, 1., 0., 3.);
        self.swell_amount = finite(self.swell_amount, 0.15, 0., 1.);
        self.span = finite(self.span, 1., 0.05, 1.);
        self.curve = finite(self.curve, 0., -2., 2.);
        self.intensity = finite(self.intensity, 1., 0., 4.);
        self.hue_shift = finite(self.hue_shift, 0., 0., 0.4);
        self.origin = self.origin.map(|v| finite(v, 0.5, -2., 3.));
        self.origin_offset = self
            .origin_offset
            .map(|v| finite(v, 0., -100_000., 100_000.));
        self.at = self
            .at
            .map(|p| p.map(|v| finite(v, 0., -100_000., 100_000.)));
        self.feather = self.feather.map(|v| finite(v, 0., 0., 1000.));
        self.amplitude = finite(self.amplitude, 18., 0., 96.);
        self.ring_width = finite(self.ring_width, 44., 4., 600.);
        self.wavelength = finite(self.wavelength, 60., 8., 600.);
        self.ripples = finite(self.ripples, 2.5, 0., 6.);
        self.echo = finite(self.echo, 0., 0., 1.);
        self.falloff = finite(self.falloff, 900., 16., 100_000.);
        self.chroma = finite(self.chroma, 0.6, 0., 2.);
        self.tint = finite(self.tint, 0.45, 0., 1.);
        self.stretch_blur = finite(self.stretch_blur, 1., 0., 2.);
        self.squeeze = finite(self.squeeze, 0.7, 0., 1.);
        self.push = finite(self.push, 0.4, 0., 0.9);
        self.hold_amount = finite(self.hold_amount, 1., 0., 2.);
        self.hold_radius = finite(self.hold_radius, 90., 8., 2000.);
        self.breath_ms = finite(self.breath_ms, 1400., 200., 10_000.);
        self.band_overlay = finite(self.band_overlay, 0., 0., 1.5);
        self.fade = self.fade.and_then(|[a, b]| {
            let a = finite(a, 0., 0., 1.);
            let b = finite(b, 1., 0., 1.);
            (b > a).then_some([a, b])
        });
        for (value, fallback) in [
            &mut self.palette.a,
            &mut self.palette.b,
            &mut self.palette.c,
            &mut self.palette.d,
        ]
        .into_iter()
        .zip([0.5, 0.5, 1., 0.])
        {
            *value = value.map(|v| finite(v, fallback, -100., 100.));
        }
        if let GlimmEasing::CubicBezier(c) = &mut self.easing {
            *c = [
                finite(c[0], 0.25, 0., 1.),
                finite(c[1], 0.1, -10., 10.),
                finite(c[2], 0.25, 0., 1.),
                finite(c[3], 1., -10., 10.),
            ];
        }
        self
    }
}

/// Configurations used by the nine demos on glimm.dev.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GlimmDemo {
    /// Bottom-up data generation.
    Generate,
    /// Short section sweep.
    Teammate,
    /// Payment confirmation sweep.
    Invoice,
    /// Async page-sized upgrade.
    Upgrade,
    /// Curved bottom-up reset.
    UsageReset,
    /// Publishing confirmation.
    Publish,
    /// Scoped mode switch.
    Autopilot,
    /// Theme change.
    Theme,
    /// Completion confirmation.
    Tasks,
}
impl GlimmDemo {
    /// All reference demos, in carousel order.
    pub const ALL: [Self; 9] = [
        Self::Generate,
        Self::Teammate,
        Self::Invoice,
        Self::Upgrade,
        Self::UsageReset,
        Self::Publish,
        Self::Autopilot,
        Self::Theme,
        Self::Tasks,
    ];
    /// Demo name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Generate => "Generate data",
            Self::Teammate => "Teammate joins",
            Self::Invoice => "Invoice paid",
            Self::Upgrade => "Upgrade to Pro",
            Self::UsageReset => "Usage reset",
            Self::Publish => "Publish",
            Self::Autopilot => "Autopilot",
            Self::Theme => "Theme",
            Self::Tasks => "Tasks",
        }
    }
    /// The reference's shape, palette and 2700ms demonstration timing.
    pub fn options(self) -> GlimmOptions {
        let mut o = GlimmOptions {
            sweep_ms: 2700.,
            curve: 0.12,
            full_bleed: Some(self != Self::Generate),
            ..GlimmOptions::default()
        };
        match self {
            Self::Generate => {
                o.direction = GlimmDirection::Btt;
                o.origin = [0.5, 1.];
            }
            Self::Teammate => {
                o.curve = 0.22;
                o.span = 0.64;
                o.fade = Some([0.16, 0.64]);
            }
            Self::Invoice => {
                o.curve = 0.22;
                o.span = 0.64;
                o.fade = Some([0.16, 0.64]);
                o.palette = GlimmPalette::from_colors(&[0x34C759, 0x00C8B3, 0x00C0E8, 0x0088FF]);
            }
            Self::Upgrade => {
                o.origin = [0.5, 0.5];
                o.origin_offset = [0.; 2];
                o.palette = GlimmPalette::from_colors(&[0x00C0E8, 0x0088FF, 0x6155F5, 0xCB30E0]);
            }
            Self::UsageReset => {
                o.direction = GlimmDirection::Btt;
                o.curve = 0.2;
                o.span = 0.92;
                o.band_tight = 16.;
                o.fade = Some([0.28, 0.82]);
                o.palette = GlimmPalette::from_colors(&[0x0088FF, 0x6155F5, 0xCB30E0, 0xFF2D55]);
            }
            Self::Publish => {
                o.palette = GlimmPalette::from_colors(&[0xCB30E0, 0xFF2D55, 0xFF8D28, 0xFFCC00]);
            }
            Self::Autopilot => {
                o.curve = 0.22;
                o.span = 0.64;
                o.fade = Some([0.16, 0.64]);
            }
            Self::Theme => {
                o.palette = GlimmPalette::from_colors(&[0xFF8D28, 0xFF2D55, 0xCB30E0, 0x6155F5]);
            }
            Self::Tasks => {
                o.palette = GlimmPalette::from_colors(&[0x30D158, 0x3CD3FE, 0x6B5DFF]);
            }
        }
        o
    }
}
pub(super) fn finite(v: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if v.is_finite() {
        v.clamp(min, max)
    } else {
        fallback
    }
}

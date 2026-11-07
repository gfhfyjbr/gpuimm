//! Cosine palettes and perceptual fitting, ported from glimm 0.4.1 (MIT).

use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};

/// A cosine palette: `a + b * cos(TAU * (c * t + d))`, in sRGB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlimmPalette {
    /// Per-channel centre.
    pub a: [f32; 3],
    /// Per-channel amplitude.
    pub b: [f32; 3],
    /// Per-channel frequency.
    pub c: [f32; 3],
    /// Per-channel phase.
    pub d: [f32; 3],
}

/// The seven palettes shipped with glimm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlimmPaletteName {
    /// The default full hue cycle.
    #[default]
    Spectrum,
    /// Cyan, indigo, magenta.
    Prism,
    /// Pink, magenta.
    Berry,
    /// Blue, cyan, green.
    Lagoon,
    /// Green, yellow, orange.
    Citrus,
    /// Cyan, blue, indigo.
    Azure,
    /// Yellow, orange, pink.
    Ember,
}

impl GlimmPaletteName {
    /// Presets in the same order as the reference.
    pub const ALL: [Self; 7] = [
        Self::Spectrum,
        Self::Prism,
        Self::Berry,
        Self::Lagoon,
        Self::Citrus,
        Self::Azure,
        Self::Ember,
    ];

    /// The reference's palette name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Spectrum => "spectrum",
            Self::Prism => "prism",
            Self::Berry => "berry",
            Self::Lagoon => "lagoon",
            Self::Citrus => "citrus",
            Self::Azure => "azure",
            Self::Ember => "ember",
        }
    }

    /// Resolve the preset using glimm's OKLCH interpolation and cosine fit.
    pub fn palette(self) -> GlimmPalette {
        match self {
            Self::Spectrum => GlimmPalette {
                a: [0.74, 0.72, 0.76],
                b: [0.5, 0.52, 0.48],
                c: [1.; 3],
                d: [0.92, 0.25, 0.58],
            },
            Self::Prism => GlimmPalette::from_colors(&[0x00C0E8, 0x6155F5, 0xCB30E0]),
            Self::Berry => GlimmPalette::from_colors(&[0xFF2D55, 0xCB30E0]),
            Self::Lagoon => GlimmPalette::from_colors(&[0x0088FF, 0x00C0E8, 0x34C759]),
            Self::Citrus => GlimmPalette::from_colors(&[0x34C759, 0xFFCC00, 0xFF8D28]),
            Self::Azure => GlimmPalette::from_colors(&[0x00C0E8, 0x0088FF, 0x6155F5]),
            Self::Ember => GlimmPalette::from_colors(&[0xFFCC00, 0xFF8D28, 0xFF2D55]),
        }
    }
}

impl Default for GlimmPalette {
    fn default() -> Self {
        GlimmPaletteName::Spectrum.palette()
    }
}

impl GlimmPalette {
    /// Fit a palette to `0xRRGGBB` anchors, along the shortest OKLCH hue arc.
    /// An empty chain uses indigo/cyan; one colour produces a constant palette.
    pub fn from_colors(colors: &[u32]) -> Self {
        if colors.is_empty() {
            return Self::from_colors(&[0x7B4FFF, 0x1FC8FF]);
        }
        if colors.len() == 1 {
            return Self {
                a: rgb(colors[0]).map(|v| v as f32),
                b: [0.; 3],
                c: [1.; 3],
                d: [0.; 3],
            };
        }
        let anchors: Vec<_> = colors.iter().map(|v| to_oklch(rgb(*v))).collect();
        let samples: Vec<_> = (0..17)
            .map(|i| {
                let seg = i as f64 / 16. * (anchors.len() - 1) as f64;
                let index = (seg.floor() as usize).min(anchors.len() - 2);
                let t = seg - index as f64;
                let a = anchors[index];
                let b = anchors[index + 1];
                let ha = if a[1] < 1e-4 { b[2] } else { a[2] };
                let hb = if b[1] < 1e-4 { a[2] } else { b[2] };
                let mut dh = hb - ha;
                if dh > PI {
                    dh -= TAU;
                }
                if dh < -PI {
                    dh += TAU;
                }
                from_oklch([
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    ha + dh * t,
                ])
            })
            .collect();
        let cs: Vec<_> = (0..17).map(|i| (PI * i as f64 / 16.).cos()).collect();
        let sn: Vec<_> = (0..17).map(|i| (PI * i as f64 / 16.).sin()).collect();
        let sum = |v: &[f64]| v.iter().sum::<f64>();
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
        let m = [
            [17., sum(&cs), sum(&sn)],
            [sum(&cs), dot(&cs, &cs), dot(&cs, &sn)],
            [sum(&sn), dot(&cs, &sn), dot(&sn, &sn)],
        ];
        let mut out = Self {
            a: [0.; 3],
            b: [0.; 3],
            c: [0.5; 3],
            d: [0.; 3],
        };
        for ch in 0..3 {
            let ys: Vec<_> = samples.iter().map(|s| s[ch]).collect();
            let v = [sum(&ys), dot(&ys, &cs), dot(&ys, &sn)];
            let det = determinant(m);
            let solution: [f64; 3] = std::array::from_fn(|column| {
                let mut matrix = m;
                for row in 0..3 {
                    matrix[row][column] = v[row];
                }
                determinant(matrix) / det
            });
            out.a[ch] = solution[0] as f32;
            out.b[ch] = solution[1].hypot(solution[2]) as f32;
            out.d[ch] = ((-solution[2]).atan2(solution[1]) / TAU).rem_euclid(1.) as f32;
        }
        out
    }

    /// Evaluate the unclamped sRGB cosine formula.
    pub fn sample(self, t: f32) -> [f32; 3] {
        std::array::from_fn(|i| {
            self.a[i] + self.b[i] * (std::f32::consts::TAU * (self.c[i] * t + self.d[i])).cos()
        })
    }
}

fn rgb(hex: u32) -> [f64; 3] {
    [
        ((hex >> 16) & 255) as f64 / 255.,
        ((hex >> 8) & 255) as f64 / 255.,
        (hex & 255) as f64 / 255.,
    ]
}
fn determinant(m: [[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}
fn to_oklch(rgb: [f64; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(|c| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
    let b = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
    [
        0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
        a.hypot(b),
        b.atan2(a),
    ]
}
fn from_oklch([light, chroma, hue]: [f64; 3]) -> [f64; 3] {
    let a = chroma * hue.cos();
    let b = chroma * hue.sin();
    let l = (light + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (light - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (light - 0.0894841775 * a - 1.291485548 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
    ]
    .map(|c| {
        (if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1. / 2.4) - 0.055
        })
        .clamp(0., 1.)
    })
}

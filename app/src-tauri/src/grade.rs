//! Film-negative color grading, ported from NexFilm
//! (https://github.com/BillyDu-TJ/NexFilm). This is the linear-light density
//! pipeline: transmittance -> density (`-log10 T`), dye-coupler de-crosstalk
//! (Status-M), base-density (film mask / orange mask) subtraction, per-channel
//! exposure offset, then D-Min/D-Max normalization, gamma, highlight/shadow
//! tone shaping, white-balance (temperature/tint) and saturation, encoded back
//! to sRGB for display. The pixel math mirrors NexFilm's WebGL develop shader
//! and `src/pipeline.rs` + `src/core_math.rs` so preview and export agree.

use serde::{Deserialize, Serialize};

use fd_io::{ImageBuf, PixelData};

/// Density-domain de-crosstalk (Status-M to print density), row-major as in
/// NexFilm's `status_m_crosstalk_matrix`. Applied as `M * delta_d`.
#[rustfmt::skip]
const STATUS_M: [[f32; 3]; 3] = [
    [1.0197,  0.0317,  0.0091],
    [-0.0052, 0.8933,  0.0521],
    [ 0.0131, -0.0011, 0.9712],
];

/// Rec. 709 / linear-sRGB luminance weights; also the monochrome analysis
/// contract (NexFilm `DENSITY_LUMA_COEFFICIENTS`).
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

fn density_luma(rgb: [f32; 3]) -> f32 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

#[inline]
fn encode_srgb(linear: f32) -> f32 {
    let l = linear.max(0.0);
    if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear sRGB -> display (sRGB gamma-encoded), clamped to [0,1].
fn linear_srgb_to_display(rgb: [f32; 3]) -> [f32; 3] {
    [
        encode_srgb(rgb[0]).clamp(0.0, 1.0),
        encode_srgb(rgb[1]).clamp(0.0, 1.0),
        encode_srgb(rgb[2]).clamp(0.0, 1.0),
    ]
}

/// Post-gamma highlight/shadow tone shaping (NexFilm `tonePostGamma`).
#[inline]
fn tone_post_gamma(value: f32, highlights: f32, shadows: f32) -> f32 {
    let c = value.clamp(0.0, 1.0);
    (value + shadows * (1.0 - c).powi(2) * value + highlights * c.powi(2) * (1.0 - value))
        .clamp(0.0, 1.0)
}

/// Color grade settings for one frame. Mirrors NexFilm's Develop controls.
/// All floats are finite; ranges are enforced by the frontend sliders but the
/// engine clamps defensively (gamma > 0, etc).
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct GradeSettings {
    /// Invert a negative film to a positive. When false the shader's "staging"
    /// view is reproduced (a straight exposure-scaled display).
    pub invert: bool,
    /// 0 = Color, 1 = B&W (NexFilm `FilmMode`).
    pub mode: u8,
    /// Base (film mask) density per channel, subtracted in the density domain.
    pub base_density: [f32; 3],
    /// Per-channel density exposure offset (Printer Lights live here too).
    pub exposure: [f32; 3],
    /// Per-channel density minimum (0.0 white-point reference in density).
    pub d_min: [f32; 3],
    /// Per-channel density maximum.
    pub d_max: [f32; 3],
    /// Gamma (inverse power applied to normalized density).
    pub gamma: f32,
    /// Highlight tone push (post-gamma).
    pub highlights: f32,
    /// Shadow tone pull (post-gamma).
    pub shadows: f32,
    /// Saturation in [-1, 1] (0 = neutral).
    pub saturation: f32,
    /// Color temperature in [-1, 1] (positive = warmer).
    pub temperature: f32,
    /// Tint (green-magenta) in [-1, 1].
    pub tint: f32,
}

impl Default for GradeSettings {
    fn default() -> Self {
        GradeSettings {
            invert: false,
            mode: 0,
            base_density: [0.0; 3],
            exposure: [0.0; 3],
            // Sensible density range when the operator has not Auto-Inverted:
            // film densities span roughly 0..2 after base subtraction.
            d_min: [0.0; 3],
            d_max: [2.0; 3],
            gamma: 1.0,
            highlights: 0.0,
            shadows: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

/// Core per-pixel grade: returns display sRGB in [0,1]. Mirrors NexFilm's
/// develop shader main() (invert branch) + pipeline.rs.
pub fn grade_pixel(linear_rgb: [f32; 3], s: &GradeSettings) -> [f32; 3] {
    const EPS: f32 = 1e-6;
    let mode = if s.mode == 0 { 0 } else { 1 };

    // Staging view (invert off): keep the negative visible until the operator
    // runs Auto Invert -- straight exposure-scaled display, no density math.
    if !s.invert {
        let mut staged = [
            (linear_rgb[0] * 2f32.powf(s.exposure[0])).clamp(0.0, 1.0),
            (linear_rgb[1] * 2f32.powf(s.exposure[1])).clamp(0.0, 1.0),
            (linear_rgb[2] * 2f32.powf(s.exposure[2])).clamp(0.0, 1.0),
        ];
        if mode == 1 {
            let g = density_luma(staged);
            staged = [g, g, g];
        }
        let safe_gamma = s.gamma.max(1e-6);
        return linear_srgb_to_display([
            staged[0].powf(1.0 / safe_gamma),
            staged[1].powf(1.0 / safe_gamma),
            staged[2].powf(1.0 / safe_gamma),
        ]);
    }

    // Density: -log10(max(T, eps)).
    let t = [
        linear_rgb[0].max(EPS),
        linear_rgb[1].max(EPS),
        linear_rgb[2].max(EPS),
    ];
    let d = [-t[0].log10(), -t[1].log10(), -t[2].log10()];

    let mut density: [f32; 3];
    if mode == 0 {
        // d = STATUS_M * (d - base); then += exposure.
        let dd = [d[0] - s.base_density[0], d[1] - s.base_density[1], d[2] - s.base_density[2]];
        density = [
            STATUS_M[0][0] * dd[0] + STATUS_M[0][1] * dd[1] + STATUS_M[0][2] * dd[2],
            STATUS_M[1][0] * dd[0] + STATUS_M[1][1] * dd[1] + STATUS_M[1][2] * dd[2],
            STATUS_M[2][0] * dd[0] + STATUS_M[2][1] * dd[1] + STATUS_M[2][2] * dd[2],
        ];
        density = [
            density[0] + s.exposure[0],
            density[1] + s.exposure[1],
            density[2] + s.exposure[2],
        ];
    } else {
        let dd = [d[0] - s.base_density[0], d[1] - s.base_density[1], d[2] - s.base_density[2]];
        let gray = density_luma(dd) + s.exposure[0];
        density = [gray, gray, gray];
    }

    // Effective D-Min/D-Max (B&W collapses to luma of the color bounds).
    let (dmin, dmax) = if mode == 0 {
        (s.d_min, s.d_max)
    } else {
        let lo = density_luma(s.d_min);
        let hi = density_luma(s.d_max);
        ([lo, lo, lo], [hi, hi, hi])
    };

    let mut out = [0f32; 3];
    for c in 0..3 {
        let range = dmax[c] - dmin[c];
        let norm = if range.abs() > 1e-6 {
            (density[c] - dmin[c]) / range
        } else {
            0.0
        };
        let gamma_corrected = norm.clamp(0.0, 1.0).powf(1.0 / s.gamma.max(1e-6));
        out[c] = gamma_corrected;
    }

    // Post-gamma adjustments (tone + white balance + saturation).
    if mode == 1 {
        // B&W: tone then force gray; skip color balance/saturation.
        let g = tone_post_gamma(out[0], s.highlights, s.shadows);
        return linear_srgb_to_display([g, g, g]);
    }

    out = [
        tone_post_gamma(out[0], s.highlights, s.shadows),
        tone_post_gamma(out[1], s.highlights, s.shadows),
        tone_post_gamma(out[2], s.highlights, s.shadows),
    ];
    let temp = s.temperature.clamp(-1.0, 1.0);
    let tint = s.tint.clamp(-1.0, 1.0);
    out[0] *= 1.0 + temp * 0.20;
    out[2] *= 1.0 - temp * 0.20;
    out[0] *= 1.0 + tint * 0.10;
    out[1] *= 1.0 - tint * 0.20;
    out[2] *= 1.0 + tint * 0.10;
    let luma = density_luma(out);
    let sat = 1.0 + s.saturation.clamp(-1.0, 1.0);
    out = [
        luma + (out[0] - luma) * sat,
        luma + (out[1] - luma) * sat,
        luma + (out[2] - luma) * sat,
    ];

    linear_srgb_to_display(out)
}

/// Applies grading to a whole image. The output keeps the source depth (U16
/// stays 16-bit so downstream dust removal/export keeps precision; U8 stays
/// 8-bit). Greyscale sources are graded as tri-channel (density in one
/// channel) and emitted as RGB. Values are quantized back to the source depth.
pub fn apply_grade(img: &ImageBuf, s: &GradeSettings) -> ImageBuf {
    let w = img.width as usize;
    let h = img.height as usize;
    let n = w * h;
    let src = img.to_f32();

    // Read pixel as 3 channels (grey -> triplicated).
    let rgb_at = |i: usize, f: &[f32]| -> [f32; 3] {
        if img.channels >= 3 {
            [f[i * 3], f[i * 3 + 1], f[i * 3 + 2]]
        } else {
            let v = f[i];
            [v, v, v]
        }
    };

    match &img.data {
        PixelData::U16(_) => {
            let mut out: Vec<u16> = Vec::with_capacity(n * 3);
            for i in 0..n {
                let p = grade_pixel(rgb_at(i, &src), s);
                out.push((p[0] * 65535.0).round() as u16);
                out.push((p[1] * 65535.0).round() as u16);
                out.push((p[2] * 65535.0).round() as u16);
            }
            ImageBuf {
                width: img.width,
                height: img.height,
                channels: 3,
                data: PixelData::U16(out),
                icc: img.icc.clone(),
                exif: img.exif.clone(),
            }
        }
        PixelData::U8(_) => {
            let mut out: Vec<u8> = Vec::with_capacity(n * 3);
            for i in 0..n {
                let p = grade_pixel(rgb_at(i, &src), s);
                out.push((p[0] * 255.0).round() as u8);
                out.push((p[1] * 255.0).round() as u8);
                out.push((p[2] * 255.0).round() as u8);
            }
            ImageBuf {
                width: img.width,
                height: img.height,
                channels: 3,
                data: PixelData::U8(out),
                icc: img.icc.clone(),
                exif: img.exif.clone(),
            }
        }
    }
}

/// Auto-invert analysis: estimate base (film-mask) density and the D-Min/D-Max
/// density bounds from the image's own distribution. `base` is the density of
/// the darkest (least-exposed) tail -- the film base / mask -- sampled as the
/// low percentile of each channel; D-Min/D-Max are the low/high density
/// percentiles after base subtraction, giving the operator a neutral starting
/// point for `invert` + normalization. Mirrors NexFilm's Auto Invert intent.
pub fn analyze_base_and_bounds(img: &ImageBuf, low_quantile: f32, high_quantile: f32) -> GradeSettings {
    let n = (img.width as usize) * (img.height as usize);
    if n == 0 {
        return GradeSettings::default();
    }
    let src = img.to_f32();
    // Per-channel transmittance -> density.
    let mut chans = [Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n)];
    for i in 0..n {
        let r = if img.channels >= 3 { src[i * 3] } else { src[i] };
        let g = if img.channels >= 3 { src[i * 3 + 1] } else { src[i] };
        let b = if img.channels >= 3 { src[i * 3 + 2] } else { src[i] };
        chans[0].push(-r.max(1e-6).log10());
        chans[1].push(-g.max(1e-6).log10());
        chans[2].push(-b.max(1e-6).log10());
    }
    // Percentile helper over an ascending-sorted density array. Density is
    // -log10(T), so a HIGH density value = dark / unexposed film base; a LOW
    // density value = bright / exposed.
    let q = |arr: &mut Vec<f32>, ql: f32| -> f32 {
        arr.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((arr.len() - 1) as f32 * ql.clamp(0.0, 1.0)).round() as usize;
        arr[idx.min(arr.len() - 1)]
    };
    // Base density ~ the darkest ~99th percentile (film base / orange mask).
    let base_q = (1.0 - low_quantile.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let base = [q(&mut chans[0], base_q), q(&mut chans[1], base_q), q(&mut chans[2], base_q)];
    // D-Min (bright, low density) / D-Max (dark, high density), measured
    // relative to base so the normalized range starts near zero.
    let dmin = [
        (q(&mut chans[0], 0.02) - base[0]).max(0.0),
        (q(&mut chans[1], 0.02) - base[1]).max(0.0),
        (q(&mut chans[2], 0.02) - base[2]).max(0.0),
    ];
    let hi_q = high_quantile.clamp(0.0, 1.0);
    let dmax = [
        (q(&mut chans[0], hi_q) - base[0]).max(dmin[0] + 0.05),
        (q(&mut chans[1], hi_q) - base[1]).max(dmin[1] + 0.05),
        (q(&mut chans[2], hi_q) - base[2]).max(dmin[2] + 0.05),
    ];
    GradeSettings {
        invert: true,
        base_density: base,
        d_min: dmin,
        d_max: dmax,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_img(v: f32) -> ImageBuf {
        let px = (v * 65535.0) as u16;
        ImageBuf {
            width: 1,
            height: 1,
            channels: 3,
            data: PixelData::U16(vec![px; 3]),
            icc: None,
            exif: None,
        }
    }

    #[test]
    fn black_negative_inverts_to_white_with_default_bounds() {
        // A dense (black) negative pixel should invert toward white.
        let img = gray_img(0.02);
        let ss = GradeSettings {
            invert: true,
            ..Default::default()
        };
        let out = apply_grade(&img, &ss);
        let out_px = match &out.data {
            PixelData::U16(v) => v[0] as f32 / 65535.0,
            _ => 0.0,
        };
        assert!(out_px > 0.8, "dense negative should go near-white, got {out_px}");
    }

    #[test]
    fn bnmode_forces_gray() {
        let img = gray_img(0.5);
        let s = GradeSettings {
            invert: true,
            mode: 1,
            ..Default::default()
        };
        let out = apply_grade(&img, &s);
        match &out.data {
            PixelData::U16(v) => {
                assert_eq!(v[0], v[1]);
                assert_eq!(v[1], v[2]);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn staging_view_without_invert_keeps_negative() {
        let img = gray_img(0.9); // light negative pixel stays light when not inverted
        let s = GradeSettings::default(); // invert = false
        let out = apply_grade(&img, &s);
        match &out.data {
            PixelData::U16(v) => {
                let v0 = v[0] as f32 / 65535.0;
                assert!(v0 > 0.5, "non-inverted staging keeps source brightness, got {v0}");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn analyze_picks_dark_base_and_bounds() {
        // Image with a dark mask region and a brighter subject.
        let n = 100;
        let mut data = Vec::with_capacity(n * 3);
        for i in 0..n {
            // 20% very dark (base) pixels, 80% mid.
            let v = if i < 20 { 0.01 } else { 0.5 };
            let px = (v * 65535.0) as u16;
            data.extend_from_slice(&[px, px, px]);
        }
        let img = ImageBuf {
            width: 10,
            height: 10,
            channels: 3,
            data: PixelData::U16(data),
            icc: None,
            exif: None,
        };
        let s = analyze_base_and_bounds(&img, 0.01, 0.98);
        assert!(s.invert);
        assert!(s.base_density[0] > 1.0, "dark base => high density, got {}", s.base_density[0]);
        assert!(s.d_max[0] > s.d_min[0]);
    }
}

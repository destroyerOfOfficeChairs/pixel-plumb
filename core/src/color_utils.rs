use crate::PixelizerError::HexParseError;
use crate::PixelizerError::NoColorsError;

/// Half-width of the a/b normalization range. The sRGB gamut's OkLab a and b
/// both fall within about ±0.32; 0.35 covers all of it with a little margin, so
/// no real color clips. L is already in [0, 1] and needs no such bound.
const OKLAB_AB_BOUND: f32 = 0.35;

#[derive(Clone, Copy)]
pub struct Oklab {
    l: f32,
    a: f32,
    b: f32,
}

pub enum BayerMatrix<'a> {
    Four(&'a [[f32; 4]; 4]),
    Eight(&'a [[f32; 8]; 8]),
}

pub struct PaletteData {
    pub rgb: Vec<[u8; 3]>,
    pub lab: Vec<Oklab>,
    pub linear: Vec<[f32; 3]>,
    pub max_per_channel: [f32; 3],
}

pub fn prepare_palette(colors: &[String]) -> Result<PaletteData, crate::PixelizerError> {
    let rgb: Vec<[u8; 3]> = colors
        .iter()
        .map(|s| parse_hex(s))
        .collect::<Result<_, _>>()?;
    if rgb.is_empty() {
        return Err(NoColorsError(
            "There are no colors in the palette.".to_owned(),
        ));
    }
    let lab: Vec<Oklab> = rgb.iter().map(|c| rgb_to_oklab(c[0], c[1], c[2])).collect();
    let linear: Vec<[f32; 3]> = rgb
        .iter()
        .map(|c| {
            [
                srgb_to_linear(c[0]),
                srgb_to_linear(c[1]),
                srgb_to_linear(c[2]),
            ]
        })
        .collect();
    let mut max_per_channel = [0.0_f32; 3];
    for &[lr, lg, lb] in &linear {
        max_per_channel[0] = max_per_channel[0].max(lr);
        max_per_channel[1] = max_per_channel[1].max(lg);
        max_per_channel[2] = max_per_channel[2].max(lb);
    }
    Ok(PaletteData {
        rgb,
        lab,
        linear,
        max_per_channel,
    })
}

pub fn parse_hex(s: &str) -> Result<[u8; 3], crate::PixelizerError> {
    let s = s.strip_prefix('#').unwrap_or(s);

    if s.len() != 6 {
        return Err(HexParseError("This is not a hex color.".to_owned()));
    }

    let r = u8::from_str_radix(&s[0..2], 16)
        .map_err(|_| HexParseError("Red is malformed.".to_owned()))?;

    let g = u8::from_str_radix(&s[2..4], 16)
        .map_err(|_| HexParseError("Green is malformed.".to_owned()))?;

    let b = u8::from_str_radix(&s[4..6], 16)
        .map_err(|_| HexParseError("Blue is malformed.".to_owned()))?;

    Ok([r, g, b])
}

/// A representative color that keeps a cluster's tone smooth but its color
/// vibrant. Takes the population-weighted **average lightness** of `entries`
/// (so palette entries step smoothly in brightness) but the **chroma** (the
/// a/b, i.e. hue and saturation) of `mode` — a real, vivid color from the box,
/// not a washed-out average. Reassembles in OkLab and converts back to sRGB.
///
/// This fixes the desaturation a full average causes: averaging a/b pulls
/// opposing hues toward grey, but lightness averages cleanly. So we average
/// only what's safe to average (L) and borrow chroma from an actual color.
///
/// `entries` is (rgb, count) pairs; `mode` is the box's most-populous color.
pub fn hybrid_lightness_chroma(entries: &[([u8; 3], u32)], mode: [u8; 3]) -> [u8; 3] {
    // Population-weighted average lightness.
    let mut total = 0f64;
    let mut l_sum = 0f64;
    for &(rgb, count) in entries {
        let lab = rgb_to_oklab(rgb[0], rgb[1], rgb[2]);
        let w = count as f64;
        l_sum += lab.l as f64 * w;
        total += w;
    }
    if total == 0.0 {
        return mode;
    }
    let avg_l = (l_sum / total) as f32;

    // Chroma from the mode.
    let mode_lab = rgb_to_oklab(mode[0], mode[1], mode[2]);

    oklab_to_rgb(Oklab {
        l: avg_l,
        a: mode_lab.a,
        b: mode_lab.b,
    })
}

/// Map an sRGB color into a normalized-OkLab cube as `[u8; 3]`, so the octree
/// (which subdivides a 0..255 cube) can quantize in *perceptual* space instead
/// of RGB. L maps [0,1] -> [0,255]; a and b map [-BOUND, +BOUND] -> [0,255].
/// Values are clamped, so out-of-gamut inputs stay in range.
pub fn rgb_to_norm_oklab(r: u8, g: u8, b: u8) -> [u8; 3] {
    let c = rgb_to_oklab(r, g, b);
    let nl = (c.l * 255.0).round().clamp(0.0, 255.0) as u8;
    let na = ((c.a + OKLAB_AB_BOUND) / (2.0 * OKLAB_AB_BOUND) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let nb = ((c.b + OKLAB_AB_BOUND) / (2.0 * OKLAB_AB_BOUND) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    [nl, na, nb]
}

/// Inverse of `rgb_to_norm_oklab`: a normalized-OkLab `[u8; 3]` back to sRGB.
/// Used at assignment to turn the octree's averaged (normalized-OkLab) palette
/// entries back into real colors.
pub fn norm_oklab_to_rgb(n: [u8; 3]) -> [u8; 3] {
    let l = n[0] as f32 / 255.0;
    let a = n[1] as f32 / 255.0 * (2.0 * OKLAB_AB_BOUND) - OKLAB_AB_BOUND;
    let b = n[2] as f32 / 255.0 * (2.0 * OKLAB_AB_BOUND) - OKLAB_AB_BOUND;
    oklab_to_rgb(Oklab { l, a, b })
}

pub fn rgb_to_oklab(r: u8, g: u8, b: u8) -> Oklab {
    let r = srgb_to_linear(r);
    let g = srgb_to_linear(g);
    let b = srgb_to_linear(b);

    // Linear RGB -> LMS
    let l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
    let m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
    let s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

    // Nonlinearity
    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    // LMS' -> OkLab
    Oklab {
        l: 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        a: 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        b: 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    }
}

/// Inverse of `rgb_to_oklab`: OkLab back to 8-bit sRGB. The exact algebraic
/// reverse — OkLab -> LMS' (inverse matrix), cube to undo the cbrt, LMS ->
/// linear RGB (inverse matrix), then linear -> sRGB (which clamps). These are
/// the standard published OkLab inverse constants.
pub fn oklab_to_rgb(c: Oklab) -> [u8; 3] {
    // OkLab -> LMS'
    let l_ = c.l + 0.3963377774 * c.a + 0.2158037573 * c.b;
    let m_ = c.l - 0.1055613458 * c.a - 0.0638541728 * c.b;
    let s_ = c.l - 0.0894841775 * c.a - 1.2914855480 * c.b;

    // Undo the cube root
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    // LMS -> linear RGB
    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

    // linear_to_srgb clamps to [0,1] internally.
    [linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b)]
}

/// Scale an OkLab color's chroma (saturation) about the neutral axis. Chroma is
/// the (a, b) distance from grey; scaling a and b by `factor` scales saturation
/// while leaving lightness untouched. factor 1.0 = unchanged, 0.0 = greyscale,
/// >1.0 = more saturated. Lightness `l` is preserved exactly.
pub fn scale_chroma(c: Oklab, factor: f32) -> Oklab {
    Oklab {
        l: c.l,
        a: c.a * factor,
        b: c.b * factor,
    }
}

/// Adjust an OkLab color's contrast by pushing lightness away from (factor > 1)
/// or toward (factor < 1) the mid-grey point of 0.5. Operating on perceptual
/// lightness `l` — rather than stretching RGB channels independently — keeps
/// hue and saturation stable; only the light/dark relationship changes. `l` is
/// clamped to [0, 1] afterward, since strong contrast can push it out of range.
/// Chroma (a, b) is untouched.
pub fn shift_contrast(c: Oklab, factor: f32) -> Oklab {
    Oklab {
        l: ((c.l - 0.5) * factor + 0.5).clamp(0.0, 1.0),
        a: c.a,
        b: c.b,
    }
}

pub fn nearest_oklab(palette: &[Oklab], target: Oklab) -> usize {
    palette
        .iter()
        .enumerate()
        .min_by(|(_, x), (_, y)| {
            let dx = (x.l - target.l).powi(2) + (x.a - target.a).powi(2) + (x.b - target.b).powi(2);
            let dy = (y.l - target.l).powi(2) + (y.a - target.a).powi(2) + (y.b - target.b).powi(2);
            dx.partial_cmp(&dy).unwrap()
        })
        .map(|(i, _)| i)
        .unwrap()
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let v = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round() as u8
}

pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn quantize(
    palette_lab: &[Oklab],
    palette_linear: &[[f32; 3]],
    pixel_linear: [f32; 3],
    error_damping: f32,
) -> (usize, [f32; 3]) {
    let [lr, lg, lb] = pixel_linear;
    let r_u8 = linear_to_srgb(lr);
    let g_u8 = linear_to_srgb(lg);
    let b_u8 = linear_to_srgb(lb);
    let idx = nearest_oklab(palette_lab, rgb_to_oklab(r_u8, g_u8, b_u8));
    let [plr, plg, plb] = palette_linear[idx];
    (
        idx,
        [
            (lr - plr) * error_damping,
            (lg - plg) * error_damping,
            (lb - plb) * error_damping,
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_to_linear_endpoints() {
        // Black and white should round-trip exactly.
        assert_eq!(srgb_to_linear(0), 0.0);
        assert!((srgb_to_linear(255) - 1.0).abs() < 0.001);
    }

    #[test]
    fn srgb_to_linear_midpoint() {
        // 50% gray in sRGB is about 21.8% in linear light — this is the
        // whole point of gamma encoding.
        let mid = srgb_to_linear(128);
        assert!(mid > 0.20 && mid < 0.23, "got {}", mid);
    }

    #[test]
    fn linear_to_srgb_roundtrip() {
        // Round-tripping through linear should preserve sRGB byte values.
        for v in [0u8, 1, 50, 128, 200, 254, 255] {
            let linear = srgb_to_linear(v);
            let back = linear_to_srgb(linear);
            assert!(
                (back as i32 - v as i32).abs() <= 1,
                "round-trip failed for {}: got {}",
                v,
                back
            );
        }
    }

    #[test]
    fn oklab_rgb_roundtrip() {
        // rgb -> oklab -> rgb should return (nearly) the same color. Allow ±2
        // per channel for float rounding through the two matrix conversions.
        for &(r, g, b) in &[
            (0u8, 0u8, 0u8),
            (255, 255, 255),
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (128, 64, 200),
            (30, 90, 150),
            (200, 180, 40),
        ] {
            let [rr, gg, bb] = oklab_to_rgb(rgb_to_oklab(r, g, b));
            assert!(
                (rr as i32 - r as i32).abs() <= 2
                    && (gg as i32 - g as i32).abs() <= 2
                    && (bb as i32 - b as i32).abs() <= 2,
                "roundtrip failed for ({r},{g},{b}): got ({rr},{gg},{bb})"
            );
        }
    }

    #[test]
    fn scale_chroma_zero_is_grey() {
        // Scaling chroma to 0 should produce a neutral grey: r == g == b.
        let grey = oklab_to_rgb(scale_chroma(rgb_to_oklab(200, 40, 40), 0.0));
        assert!(
            (grey[0] as i32 - grey[1] as i32).abs() <= 1
                && (grey[1] as i32 - grey[2] as i32).abs() <= 1,
            "expected grey, got {grey:?}"
        );
    }

    #[test]
    fn parse_hex_basic() {
        assert_eq!(parse_hex("#ff0000").unwrap(), [255, 0, 0]);
        assert_eq!(parse_hex("00ff00").unwrap(), [0, 255, 0]);
        assert_eq!(parse_hex("#0000FF").unwrap(), [0, 0, 255]);
    }

    #[test]
    fn parse_hex_rejects_bad_input() {
        assert!(parse_hex("").is_err());
        assert!(parse_hex("#fff").is_err()); // too short
        assert!(parse_hex("#ff00000").is_err()); // too long
        assert!(parse_hex("#gggggg").is_err()); // not hex
        assert!(parse_hex("not a color").is_err());
    }

    #[test]
    fn nearest_oklab_picks_exact_match() {
        // A palette containing the target color should always pick that color.
        let red = rgb_to_oklab(255, 0, 0);
        let green = rgb_to_oklab(0, 255, 0);
        let blue = rgb_to_oklab(0, 0, 255);
        let palette = vec![red, green, blue];

        assert_eq!(nearest_oklab(&palette, rgb_to_oklab(255, 0, 0)), 0);
        assert_eq!(nearest_oklab(&palette, rgb_to_oklab(0, 255, 0)), 1);
        assert_eq!(nearest_oklab(&palette, rgb_to_oklab(0, 0, 255)), 2);
    }

    #[test]
    fn nearest_oklab_picks_perceptually_closer() {
        // A slightly-off red should map to pure red, not to green or blue.
        let palette = vec![
            rgb_to_oklab(255, 0, 0),
            rgb_to_oklab(0, 255, 0),
            rgb_to_oklab(0, 0, 255),
        ];
        let off_red = rgb_to_oklab(240, 10, 10);
        assert_eq!(nearest_oklab(&palette, off_red), 0);
    }

    #[test]
    fn prepare_palette_rejects_empty() {
        let result = prepare_palette(&[]);
        assert!(matches!(
            result,
            Err(crate::PixelizerError::NoColorsError(_))
        ));
    }

    #[test]
    fn prepare_palette_rejects_bad_hex() {
        let result = prepare_palette(&["#ff0000".into(), "garbage".into()]);
        assert!(matches!(
            result,
            Err(crate::PixelizerError::HexParseError(_))
        ));
    }

    #[test]
    fn prepare_palette_computes_max_correctly() {
        let palette =
            prepare_palette(&["#000000".into(), "#808080".into(), "#ffffff".into()]).unwrap();
        // White should be the max in every channel.
        assert!((palette.max_per_channel[0] - 1.0).abs() < 0.001);
        assert!((palette.max_per_channel[1] - 1.0).abs() < 0.001);
        assert!((palette.max_per_channel[2] - 1.0).abs() < 0.001);
    }
}

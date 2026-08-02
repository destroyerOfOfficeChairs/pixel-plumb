use crate::Image;
use image::imageops::FilterType::Nearest;
use image::imageops::resize;

/// Resize an image with nearest-neighbor (crisp, pixel-art-appropriate).
///
/// Two modes, selected by `exact`:
/// - `exact == false` (longest-side): scale so the longer dimension becomes
///   `max_size`, the shorter scales proportionally. Aspect ratio preserved.
///   This is the usual pixel-art need ("make the biggest side 64px").
/// - `exact == true`: resize straight to `width` × `height`, no aspect
///   preservation — the user asked for those exact dimensions.
///
/// No cropping in either mode: the whole source is scaled to fit the target.
pub fn pixelizer_resize(
    image: Image,
    exact: bool,
    max_size: u32,
    width: u32,
    height: u32,
) -> Image {
    let (nw, nh) = if exact {
        (width.max(1), height.max(1))
    } else {
        longest_side_dims(image.width(), image.height(), max_size)
    };
    resize(&image, nw, nh, Nearest)
}

/// Given source dimensions and a target for the longer side, compute the output
/// dimensions that preserve aspect ratio. The longer source dimension maps to
/// `max_size`; the shorter scales by the same factor (min 1 so it never
/// collapses to zero).
fn longest_side_dims(w: u32, h: u32, max_size: u32) -> (u32, u32) {
    if w >= h {
        (max_size, (h * max_size / w).max(1))
    } else {
        ((w * max_size / h).max(1), max_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_side_landscape() {
        assert_eq!(longest_side_dims(200, 100, 64), (64, 32));
    }

    #[test]
    fn longest_side_portrait() {
        assert_eq!(longest_side_dims(100, 200, 64), (32, 64));
    }

    #[test]
    fn longest_side_square() {
        assert_eq!(longest_side_dims(50, 50, 64), (64, 64));
    }

    #[test]
    fn longest_side_never_zero() {
        let (_, h) = longest_side_dims(1000, 5, 64);
        assert!(h >= 1);
    }
}

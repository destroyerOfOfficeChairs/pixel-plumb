use crate::Image;
use crate::color_utils::{oklab_to_rgb, rgb_to_oklab, shift_contrast};

/// Adjust image contrast in OkLab space. `factor` 1.0 leaves the image
/// unchanged, >1.0 increases contrast (lights lighter, darks darker), and <1.0
/// flattens it toward mid-grey. Operating on perceptual lightness keeps hue and
/// saturation stable — unlike stretching RGB channels, which shifts color.
/// Pointwise; alpha is passed through untouched.
pub fn contrast(image: Image, factor: f32) -> Image {
    let (w, h) = image.dimensions();
    let mut out = Image::new(w, h);

    for (x, y, px) in image.enumerate_pixels() {
        let [r, g, b, a] = px.0;
        let [nr, ng, nb] = oklab_to_rgb(shift_contrast(rgb_to_oklab(r, g, b), factor));
        out.put_pixel(x, y, image::Rgba([nr, ng, nb, a]));
    }

    out
}

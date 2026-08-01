use crate::Image;
use crate::color_utils::{oklab_to_rgb, rgb_to_oklab, scale_chroma};

/// Scale image saturation in OkLab space. `factor` 1.0 leaves the image
/// unchanged, 0.0 makes it greyscale, and >1.0 increases saturation. Operating
/// in OkLab (rather than scaling RGB channels) keeps lightness and hue stable —
/// only the chroma changes — which is what "more/less saturated" should mean
/// perceptually. Pointwise; alpha is passed through untouched.
pub fn saturation(image: Image, factor: f32) -> Image {
    let (w, h) = image.dimensions();
    let mut out = Image::new(w, h);

    for (x, y, px) in image.enumerate_pixels() {
        let [r, g, b, a] = px.0;
        let [nr, ng, nb] = oklab_to_rgb(scale_chroma(rgb_to_oklab(r, g, b), factor));
        out.put_pixel(x, y, image::Rgba([nr, ng, nb, a]));
    }

    out
}

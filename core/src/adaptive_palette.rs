use crate::palette_map::palette_map;
use crate::{DitherConfig, Image, PixelizerError};

pub fn adaptive_palette(
    image: Image,
    colors: u32,
    dither: Option<DitherConfig>,
    preserve_alpha: Option<bool>,
    space: crate::color_utils::MappingSpace,
) -> Result<Image, PixelizerError> {
    use crate::color_utils::MappingSpace;
    let n = colors.max(1) as usize;
    let rgb = match space {
        MappingSpace::Oklab => crate::octree::octree_palette_oklab(&image, n),
        MappingSpace::Rgb => crate::octree::octree_palette(&image, n),
    };
    let hex: Vec<String> = rgb
        .iter()
        .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
        .collect();
    palette_map(image, &hex, dither, preserve_alpha, space)
}

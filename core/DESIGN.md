# Design and implementation notes

Rationale for the architectural and algorithmic choices in `pixelizer-core`. For the user-facing operations reference, see [README.md](README.md). For not-yet-built operations and features, see [ROADMAP.md](ROADMAP.md). For a prospective GPU backend, see [GPU_NOTES.md](GPU_NOTES.md).

## Why perceptual color matching?

`palette_map` uses OkLab distance rather than RGB distance to decide which palette color is "nearest" to each pixel. OkLab is a perceptually uniform color space — equal numeric distances correspond to equal perceived color differences. In RGB, the difference between two greens can numerically equal the difference between a green and a brown, even though the second pair looks more different to a human. OkLab fixes this.

## Adjusting color in OkLab (saturation, contrast)

`palette_map` only ever converts *into* OkLab — it compares distances and then writes out palette colors it already holds in sRGB, so it never needs the reverse conversion. `saturation` and `contrast` are different: they *modify* a color and must convert back, so they needed an OkLab→sRGB inverse (`oklab_to_rgb`) that didn't exist until they did. Both operate in OkLab for the same reason `palette_map` matches there — it's perceptually honest:

- **Saturation** scales chroma — the `(a, b)` distance from the neutral axis — leaving lightness `L` untouched. A more-saturated red stays the same brightness; it just gets more colorful. Scaling RGB channels instead would shift brightness and hue.
- **Contrast** pushes lightness `L` away from (or toward) the mid-grey point of 0.5, leaving chroma untouched, then clamps `L` to `[0, 1]`. Doing this on perceptual lightness rather than stretching RGB channels keeps hue and saturation stable while only the light/dark relationship changes.

Both are pointwise and preserve alpha.

## Why error diffusion happens in linear-light RGB

sRGB values stored in image files are gamma-encoded — they're nonlinear with respect to actual light intensity. Adding 0.1 to an sRGB value doesn't add a consistent amount of light depending on where you start.

When dithering propagates quantization error to neighboring pixels, that error needs to be arithmetic on light intensities, not on gamma-encoded numbers. Otherwise, the algorithm generates the wrong corrections and produces too-dark midtones and color casts. `palette_map_diffuse` converts to linear-light floats, dithers in that space, and converts back to sRGB only when writing each output pixel.

## Averaging in a linear space

The same gamma concern applies to any operation that averages pixels, not just dithering. Averaging gamma-encoded sRGB values darkens the result, because the midpoint of two encoded values is not the encoding of the midpoint of their intensities. `blur` therefore averages in a linear space and converts back only when writing output. Any future operation that combines pixels (e.g. the planned `kuwahara` and `sharpen`, see ROADMAP.md) should do the same.

## Why the palette is stored three ways

`prepare_palette` returns palette data in three representations:
- `rgb` — Original sRGB bytes, used for writing output pixels.
- `lab` — OkLab values, used for nearest-color decisions.
- `linear` — Linear-light floats, used for error propagation during dithering.

Each representation serves a different purpose. We compute them once during palette setup and pass references to them through the inner loops.

## Adaptive palette generation (median-cut)

`adaptive_palette_map` generates a palette *from the image* instead of taking a fixed one, then reduces the image to it. Crucially, it doesn't reimplement mapping or dithering — it generates a list of hex colors and hands them to the existing `palette_map`, whose input is already `&[String]`. The only new code is the palette generation; the mapping path is reused unchanged.

Generation is **median-cut**: subsample the pixels (a few thousand capture the color distribution and keep it fast inside the synchronous run), place them in one box, then repeatedly split the box with the widest color range — along its widest channel, at that channel's median — until we reach the requested count or no box can be split further. Each box's average color becomes a palette entry.

Two deliberate choices:
- **Split by widest range, not by pixel count.** The classic variant splits the most-populous box; splitting the widest-range box instead allocates palette entries where color *variation* is, which gives better perceptual coverage.
- **Average each box in linear light**, for the same gamma reason as blur and dithering above — the mean of gamma-encoded values isn't the encoding of the mean intensity.

The requested count is a *maximum*: an image with fewer distinct colors than requested yields fewer entries, since a single-color box can't be split. k-means and the pixel-count median-cut variant are possible future generators behind the same "generate a palette, then delegate to `palette_map`" seam (see ROADMAP.md).

## Output encoding: indexed PNG when it pays

`encode_png` chooses the PNG color type based on the image. Pixel-art output usually has few distinct colors — that's the whole point — but a truecolor PNG stores 4 bytes per pixel regardless. When an image is fully opaque *and* has ≤256 distinct colors, `encode_png` writes an **indexed** PNG instead: the palette is stored once, and each pixel becomes a small index into it (an 8× cut before compression, and indexed data compresses better too). Anything with transparency or more than 256 colors falls back to truecolor.

The opaque-only limit is deliberate: indexed PNGs carry transparency through a separate `tRNS` chunk, and supporting it means counting distinct `(R,G,B,A)` tuples and building that chunk — more code for the less common case. Opaque outputs (a downsampled, palette-mapped image is usually opaque) get the win now; transparent-image indexing is a possible later refinement (see ROADMAP.md).

## Where the schema descriptors live

Core also carries descriptor tables (`op_schema`) describing each operation's parameters — names, kinds, ranges, defaults — as plain data, so a frontend derives its config controls from one source of truth rather than restating them. This is a software-structure decision rather than a color-science one; its full rationale (the value-bag, and why the typed `Operation` enum is reconstructed only at a single boundary) lives in the repo's [ARCHITECTURE.md](../ARCHITECTURE.md).

## Operation order matters

The pipeline is just an ordered list, but the order has real consequences:
- **Palette mapping should come after operations that blend or average pixels** (`blur`, and a smooth `resize`). Blending produces intermediate colors, so mapping earlier and then blending reintroduces off-palette colors and undoes the quantization. This applies to both `palette_map` and `adaptive_palette_map`.
- **`downsample` should generally come before palette mapping too**, though for a different reason: it's nearest-neighbor (it selects pixels, it doesn't average), so it won't reintroduce colors — but you want the chunky downsampled *structure* fixed before quantizing, and mapping the full-resolution image first is just wasted work. (Mapping before downsampling still produces valid output, only slower.)
- **`adaptive_palette_map` especially wants to run late**, since it generates its palette from its input image — you want it to see the already-downsampled pixels so the palette is optimized for what actually gets output.
- **`normalize` should come before any quantization step** (`posterize`, `palette_map`, `adaptive_palette_map`) whose output depends on the input's brightness distribution.
- **`upscale` should be the last step** — it's for display, and anything after it operates on the blown-up pixels.

None of this is enforced in code; the pipeline trusts the user. This ordering guidance is the kind of thing worth surfacing in user-facing docs rather than guarding against.

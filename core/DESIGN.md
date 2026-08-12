# Design and implementation notes

Rationale for the architectural and algorithmic choices in `pixelizer-core`. For the user-facing operations reference, see [README.md](README.md). For not-yet-built operations and features, see [ROADMAP.md](ROADMAP.md). For a prospective GPU backend, see [GPU_NOTES.md](GPU_NOTES.md).

## Why perceptual color matching?

By default, `palette_map` uses OkLab distance rather than RGB distance to decide which palette color is "nearest" to each pixel. OkLab is a perceptually uniform color space — equal numeric distances correspond to equal perceived color differences. In RGB, the difference between two greens can numerically equal the difference between a green and a brown, even though the second pair looks more different to a human. OkLab fixes this.

The mapping space is a per-op choice (`mapping_space`, default OkLab): naive RGB nearest-distance is also available. OkLab is the better default and wins on most images, but at low color counts the two can trade places, so the choice is surfaced rather than hardcoded. Only the *nearest-entry* decision changes between the two — dithering's error diffusion stays in linear light either way (it's about light quantities, not perceptual matching), so the two modes differ only in which palette color each pixel is assigned to. The default adaptive palette is selected in OkLab too (see below), so the perceptual choice is consistent across selection and mapping.

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

## Adaptive palette generation (octree quantization)

`adaptive_palette_map` generates a palette *from the image* instead of taking a fixed one, then reduces the image to it. Crucially, it doesn't reimplement mapping or dithering — it generates a list of hex colors and hands them to the existing `palette_map`, whose input is already `&[String]`. The only new code is the palette generation; the mapping path is reused unchanged. This "generate a palette, then delegate" seam is what let the generator be swapped wholesale (see below) without touching mapping.

Generation is **octree quantization** (`octree.rs`), the algorithm ImageMagick uses, built from its specification (https://imagemagick.org/quantize/). It runs in three phases:

- **Classify** — each pixel walks an octree that subdivides the color cube into eight equal octants per level. The cubes are a *fixed* geometric grid; what adapts to the image is which cubes get instantiated (lazily, only where pixels fall) and how deep the tree goes in dense color regions. Because the cuts are always at midpoints, the octant a color belongs to at each level is just a 3-bit value read from that level's bit of each channel — no comparisons. Each node accumulates a pixel count, colour sums, and a squared-error term.
- **Reduce** — the tree is collapsed, deepest level first, until at most the requested number of nodes own a color. At each step the lowest-error nodes are pruned, folding their color statistics up into their parents. Pruning removes the *cheapest* distinctions first, so the budget is spent where it matters — and, unlike a fixed heuristic, this adapts correctly at both 4 colors and 64.
- **Assign** — each surviving owner emits its mean color (sums ÷ count).

Two things worth recording:

- **Depth is a function of the requested color count, not fixed.** A full-depth tree distinguishes all 16M colors and is catastrophically slow to build and reduce on a photo. Capping depth (roughly `log2(colors)` plus a small margin, clamped to a ceiling) keeps the tree small while still giving reduction plenty of distinctions to choose from. Combined with a level-order (deepest-first) single-pass reduction and a maintained owner count, this makes the whole thing linear-ish in practice rather than the quadratic a naive rescan would cost.
- **It can subdivide OkLab space instead of RGB.** `octree_palette_oklab` runs the *same* octree on pixels first mapped into a normalized-OkLab cube, so colors group by perceptual proximity and the pruning error is a perceptual distance. The octree itself is untouched — only the coordinates going in and the palette coming out are transformed. This is Pixel Plumb's default for adaptive palettes: perceptual selection to match the perceptual mapping. The RGB variant (`octree_palette`) is kept as a comparison baseline.

The requested count is a *maximum*: an image with fewer distinct colors than requested yields fewer entries. (An earlier version used median-cut; the octree replaced it because median-cut heuristics couldn't allocate palette entries well across both low and high color counts on the same image — see ROADMAP.md for the history and possible future generators like Wu's algorithm or k-means, which would slot into the same delegation seam.)

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

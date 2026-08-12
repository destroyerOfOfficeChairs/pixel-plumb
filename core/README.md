# pixelizer-core

A pipelined image-processing library for transforming photographs into pixel art.
Operations are described as a list of steps in YAML (or any serde format) and applied in sequence to produce a final image.

## Quickstart

```rust
use pixelizer_core::{apply, Pipeline};

let yaml = std::fs::read_to_string("pipeline.yaml")?;
let pipeline: Pipeline = serde_yaml::from_str(&yaml)?;
let image = image::open("input.png")?.to_rgba8();
let output = apply(&pipeline, image)?;
output.save("output.png")?;
```

## Pipeline format

A pipeline is a list of operations. Each operation has a `type` field identifying it and optional parameters. Operations are applied top-to-bottom.

### Example yaml:

```yaml
operations:
  - type: downsample
    pixel_size: 16
  - type: posterize
    levels: 5
  - type: blur
    sigma: 4.0
  - type: normalize
    low: 0.01
    high: 0.99
  - type: palette_map
    colors:
      - "#2c2416"
      - "#6e5a3d"
      - "#a08658"
      - "#c4a875"
    dither:
      algorithm: atkinson
      bleed: 0.2
      clamp: false
  - type: upscale
    factor: 16
```

### Operations

**`downsample`** — Nearest-neighbor downscale by `pixel_size`. Crops the image so dimensions are evenly divisible by `pixel_size` (avoiding fractional pixels), then samples.
- `pixel_size: u32`

**`resize`** — Nearest-neighbor resize with two modes.
- `exact: bool` (default false) — when false, longest-side mode; when true, exact mode.
- `max_size: u32` (longest-side mode) — the longer output dimension; the shorter scales proportionally, preserving aspect ratio.
- `width: u32`, `height: u32` (exact mode) — resize straight to these dimensions, no aspect preservation.

**`upscale`** — Nearest-neighbor upscale by an integer factor. Used at the end of a pipeline to make output pixel art viewable at sensible sizes.
- `factor: u32`

**`posterize`** — Reduces each color channel to N evenly-spaced levels. Produces classic banded color regions. `levels: 4` gives 64 total colors.
- `levels: u32` (minimum 2)

**`blur`** — Gaussian blur (computed in linear light). Smooths the input so adjacent similar pixels collapse together when quantized.
- `sigma: f32`

**`normalize`** — Stretches each channel so a chosen percentile of pixels fills the 0–255 range. Useful when the image's brightness distribution doesn't match the palette's.
- `low: f32` — Percentile cutoff for the dark end (default 0.01)
- `high: f32` — Percentile cutoff for the bright end (default 0.99)

**`saturation`** — Scales chroma in OkLab, leaving lightness unchanged.
- `factor: f32` (default 1.0) — 0.0 is greyscale, >1.0 more saturated.

**`contrast`** — Pushes lightness away from mid-grey in OkLab, leaving chroma unchanged.
- `factor: f32` (default 1.0) — >1.0 more contrast, <1.0 flatter.

**`palette_map`** — Maps each pixel to its nearest color in a user-specified palette. The distance metric is chosen by `mapping_space` (default `oklab`, perceptual; `rgb` for naive Euclidean).
- `colors: Vec<String>` — Hex color strings, e.g. `"#ff0000"`
- `preserve_alpha: bool` (optional) — keep the source alpha channel rather than making output opaque.
- `dither` (optional) — One of:
  - `algorithm: floyd_steinberg | atkinson | jjn` plus:
    - `bleed: f32` — Fraction of quantization error to propagate (default 1.0). Lower values reduce bleeding for palettes that can't represent the input's brightness range.
    - `clamp: bool` — Constrain the error-diffusion buffer to the palette's range. Helps when the palette can't represent brights or darks (default true).
  - `algorithm: bayer4 | bayer8` plus:
    - `strength: f32` — Magnitude of the per-pixel dither bias (default 32.0).
- `mapping_space` (optional) — `oklab` (default, perceptual) or `rgb` (naive Euclidean).

**`adaptive_palette_map`** — Generates a palette *from the image* (octree quantization) and maps to it. Same `dither`, `preserve_alpha`, and `mapping_space` options as `palette_map`; instead of a color list, takes a count. The palette is selected in the same space as mapping (OkLab by default).
- `colors: u32` (default 16, range 2–256) — the maximum palette size to generate.
- `mapping_space` (optional) — as above; also selects which space the palette is generated in.

## Module layout

- `lib.rs` — Pipeline definition, `Operation` enum, `DitherConfig`, `apply` / `apply_stages` orchestrators.
- `color_utils.rs` — OkLab conversion (both directions), palette preparation, hex parsing, chroma/contrast helpers, `MappingSpace`, and the RGB/OkLab nearest-entry functions.
- `palette_map.rs` — Three palette-mapping algorithms (flat, error-diffusion, ordered).
- `octree.rs` — Octree color quantization (classify/reduce/assign), in RGB or OkLab space.
- `adaptive_palette.rs` — Adaptive palette generation; runs the octree, then delegates mapping to `palette_map`.
- `downsample.rs`, `upscale.rs`, `pixelizer_resize.rs`, `posterize.rs`, `blur.rs`, `normalize.rs`, `saturation.rs`, `contrast.rs` — One per pipeline operation.
- `encode.rs` — PNG output, choosing indexed color when the image is small and opaque.
- `op_schema.rs` (+ `op_schema/tables.rs`, `op_schema/labels.rs`) — Descriptor tables (parameter names, types, defaults, ranges) that let a frontend render operation and dither config UI without hardcoding it. See [DESIGN.md](DESIGN.md).

For the rationale behind these design choices — perceptual color matching, linear-light error diffusion, octree palette generation (in RGB or OkLab space), indexed encoding, operation ordering, and more — see [DESIGN.md](DESIGN.md). For notes on a possible GPU backend, see [GPU_NOTES.md](GPU_NOTES.md).

## References

- Tanner Helland, ["Image Dithering: Eleven Algorithms and Source Code"](https://tannerhelland.com/2012/12/28/dithering-eleven-algorithms-source-code.html)
- Björn Ottosson, ["A perceptual color space for image processing"](https://bottosson.github.io/posts/oklab/)

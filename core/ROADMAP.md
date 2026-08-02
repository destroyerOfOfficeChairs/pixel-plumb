# Roadmap

Operations and features that are **not yet built**. This file exists so the design thinking behind each idea survives a long gap away from the project — each entry records not just *what* but *why* and a rough *how*, enough to start from rather than re-deriving from scratch.

For the rationale behind code that already exists, see [DESIGN.md](DESIGN.md). For a separate, larger exploration of a possible GPU backend, see [GPU_NOTES.md](GPU_NOTES.md).

---

## Op Schema Cleanup

Resize op — collapse flat fields into a mode enum

The resize op currently carries four flat fields — exact: bool, max_size, width, height — where the bool selects which of the other three are meaningful (longest-side mode uses max_size; exact mode uses width/height). This was the pragmatic choice for v0.1: it fits the flat value-bag with no special boundary work, at the cost of the YAML carrying fields the active mode ignores, and of "exact mode with no dimensions" being a representable (if defaulted) state.

The cleaner shape is a nested enum:

```rust
enum ResizeMode {
    LongestSide { max_size: u32 },
    Exact { width: u32, height: u32 },
}
```

This makes illegal states unrepresentable (can't have Exact without both dimensions), and each variant serializes only its own fields — cleaner YAML, self-describing. The cost is at the boundary (to_operation): reconstructing a nested enum from the flat bag is bespoke code — read exact, then read either max_size or width+height and build the matching variant — rather than reading four flat keys.

## Operations

### `kuwahara` — edge-preserving smoothing

**Why.** Does what `blur` can't: smooth *within* regions without smoothing *across* edges. A Gaussian softens everything uniformly, including the boundaries worth keeping sharp before quantization. Kuwahara flattens flat areas into solid color while leaving edges intact — exactly the input quantization wants, since it collapses noise and gradients into the uniform patches a small palette represents cleanly. It's arguably a better aesthetic fit for this pipeline than any other planned op.

**How.** For each pixel, consider several overlapping sub-regions of the surrounding window, compute each region's variance, and output the *mean* of the lowest-variance region. Low-variance regions are the ones not straddling an edge, so the output is drawn from whichever neighborhood is most internally uniform — which is why edges survive.

Design decisions to make:
- **Variance on a single channel, not per-channel.** Per-channel variance gives three different "winning" regions with no coherent way to combine them. Compute variance on OkLab L (perceptual lightness) to pick the region, then copy that region's mean *color*.
- **Means computed in a linear space** (linear-light RGB or OkLab), not gamma-encoded sRGB — see "Averaging in a linear space" in DESIGN.md. Averaging sRGB darkens the flat regions.
- **Square-quadrant variant first.** Simplest and most instructive, but has visible blocky artifacts on close inspection. Generalized Kuwahara (smooth weighting over more sectors) and anisotropic Kuwahara (sectors following local structure) look substantially better at substantially more cost — future refinements, not the first pass.

**Parameter.** `radius: u32` — half-size of the sampling window.

**Cost.** Heavier than the separable Gaussian — each output pixel computes mean and variance over multiple overlapping regions. Matters most in the webui (synchronous, main-thread): a strong candidate for an "expensive op" warning. Note it's fully parallel, so it ports cleanly to the GPU (see GPU_NOTES.md).

**Ordering.** Before quantization (its whole purpose), and usually before `downsample` so it works on full-resolution detail.

### `brightness`

**Why.** `normalize` stretches the brightness distribution automatically; explicit brightness curves give manual control over how the input tone maps onto a palette, which `normalize` alone can't express. Cheap and frequently wanted.

**How.** Pointwise tone curve. Apply in linear-light to avoid the gamma artifacts that plague sRGB-space arithmetic.

**Parameters.** `amount: f32`.

**Ordering.** Before quantization, like `normalize`.

### `hue_rotate`

**Why.** Cheap in OkLab, and opens up palette-shifting / color-grading effects. Low priority but nearly free once the OkLab pointwise plumbing for `saturation` exists.

**How.** Rotate the a/b chroma vector by a fixed angle in OkLab. Pointwise.

**Parameter.** `degrees: f32`.

### `sharpen`

**Why.** A natural counterpart to the existing `blur`, useful for recovering definition lost to `downsample`.

**How.** Unsharp mask: blur the image, then add back a scaled difference between the original and the blurred version. Reuses the existing blur path. Average in linear space, same as blur.

**Parameters.** `amount: f32`, and a `sigma: f32` for the underlying blur.

### `edge_detect` / `outline`

**Why.** Pixel art frequently benefits from darkened outlines around regions. Detected edges can be composited as the darkest palette color after `palette_map`.

**How.** Sobel, or difference-of-gaussians, to produce an edge map. Open question: whether this is one op that outputs an edge mask, or a combined op that detects edges and composites them onto the image. The compositing-onto-palette behavior is the genuinely useful end goal but couples it to `palette_map`; worth thinking through before implementing.

**Parameters.** TBD — likely a threshold and an output/blend mode.

### `crop`

**Why.** `downsample`'s trimming handles divisibility, but deliberate compositional cropping is a separate need not currently expressible.

**How.** Straightforward rectangular crop.

**Parameters.** `x`, `y`, `width`, `height` (all `u32`).

This is a low priority item that might not be implemented. Cropping is better with drag controls, not just sliders on a card. Also, the user should crop their image before bringing it to Pixelizer.

---

## Larger explorations

- **GPU backend** — a possible wgpu rewrite of the core operations. Self-contained analysis with its own design fork; see [GPU_NOTES.md](GPU_NOTES.md).

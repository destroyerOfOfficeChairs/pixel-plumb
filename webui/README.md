# pixelizer · webui

A browser front-end for the `pixelizer-core` pixel-art pipeline. Rust compiled to WebAssembly via [Leptos](https://leptos.dev/) (CSR), bundled by [Trunk](https://trunkrs.dev/). Assemble an ordered pipeline of operations, load an image, run it, view the result — entirely client-side, no server, no application JavaScript.

The `webui` member of the `pixelizer` workspace, alongside `core` (the pipeline library) and `cli`.

Planned work: [ROADMAP.md](ROADMAP.md).

## Status

Feature-complete for v0.1. Build a pipeline, load an image, run it, inspect the result — including per-operation stage previews and a resolution/file-size readout. The pipeline runs **synchronously on the main thread**, so the UI freezes for the length of a run; a spinner now signals that a run is in progress so the freeze reads as "working" rather than "hung." Actually removing the freeze via a web worker is the headline remaining task (ROADMAP).

## Running locally

From `webui/`:

```
trunk serve                       # http://localhost:8080
trunk serve --address 0.0.0.0     # reachable from other LAN devices
```

For LAN access, find your IP with `ip addr show | grep "inet "` (a `192.168.x.x` / `10.x.x.x`) and browse to `http://<ip>:8080`. Layout is desktop-oriented; small screens are a known gap.

## Building for release

Trunk mirrors `cargo`'s dev/release split, and the gap matters more here than usual: a dev `.wasm` is both slower to run and larger to download. **Measure and share only release builds** — dev numbers are meaningless for an image pipeline.

```
trunk build --release
trunk serve --release
```

Two WASM-specific levers beyond the flag:

**Release profile** (workspace-root `Cargo.toml`):

```toml
[profile.release]
opt-level = "s"     # "s"/"z" = size; "3" (default) = speed
lto = true          # near-free size+speed win
codegen-units = 1   # marginally better codegen, slower build
```

Compute is the bottleneck here, not download, so `opt-level = "3"` is likely better — measure both against a representative image. `lto = true` regardless.

**`wasm-opt`** (Binaryen) — post-`rustc` pass, shrinks the `.wasm` further; Trunk runs it, configured in `Trunk.toml`. See Trunk docs for current option names.

## What it does

Load an image, assemble an ordered list of operations, run the pipeline, and view the result — all in the browser.

**Operations** (see [core's DESIGN.md](../core/DESIGN.md) for the color-science rationale behind them):

- **downsample** — shrink by nearest-neighbor sampling (cropping to an even multiple first), the basis of the pixelated look.
- **resize** — nearest-neighbor resize, either to a longest-side target (aspect preserved) or exact width×height.
- **palette map** — map every pixel to the nearest color in a chosen palette, with optional error-diffusion or ordered dithering. The nearest-match metric is selectable per op (OkLab perceptual by default, or naive RGB).
- **adaptive palette map** — generate a palette *from the image* (octree quantization, in the same space as mapping) and map to it; same dithering and mapping-space options, no palette to pick.
- **posterize** — reduce the number of levels per channel.
- **blur** — Gaussian blur (in linear light).
- **normalize** — stretch the brightness range by percentile.
- **saturation** / **contrast** — adjust chroma / lightness in OkLab.
- **upscale** — integer nearest-neighbor scale-up, for viewing pixel art at size.

Operation order matters — palette mapping should come after averaging steps like downsample and blur. See [DESIGN.md](../core/DESIGN.md) for the full ordering guidance.

**Features:**

- **Drag-and-drop reordering** of the pipeline, with insert-anywhere between ops.
- **Stage previews** — after a run, a filmstrip shows the image after each operation (plus the original); click to inspect any stage in the viewport. Editing any op invalidates the previews until the next run.
- **Live YAML preview** — a toggle below Run shows the pipeline serialized to YAML, with copy-to-clipboard. The output is exactly what the `cli` parses: copy, save as `.yaml`, and it runs unmodified through the CLI.
- **Instant image display** on upload (via an object URL — the decode for the pipeline happens in the background).
- **Size-optimized output** — results are encoded as indexed PNGs when small and opaque, often several times smaller than truecolor (see DESIGN.md).

## How it's built

The one design decision to load into your head first: **the live pipeline is stored as data — a "value bag" — not as core's typed `Operation` enum.** A widget reads and writes bag entries directly; the typed enum is reconstructed once, at Run, at a single boundary. This is what lets the scalar ops share one generic config component (adding one is a single schema-table row). The full walk-through — schema, bag, boundary, run path — is in the repo's [ARCHITECTURE.md](../ARCHITECTURE.md). The color-science and algorithmic rationale (OkLab, linear-light dithering, octree palette generation, indexed encoding) is in [core's DESIGN.md](../core/DESIGN.md).

A deliberate consequence worth stating here: the heavy functions (`decode`, `encode_png`, `apply`) are plain, free of Leptos and DOM types, so the pipeline can move into a web worker later without dragging UI code along.

## Dependencies of note

- `leptos` (csr) — reactive UI.
- `leptos-use` — `use_element_size` drives the op card's collapse animation.
- `gloo-file` — async read of uploaded bytes; `gloo-timers` — the paint-before-run yield and the "Copied!" delay.
- `pixelizer-core` (workspace path) — the pipeline; re-exports `image`, keeping decode/encode on core's `image` version.
- `base64` — the result `data:` URL.
- `serde_yaml` — YAML preview; same crate+version `cli` parses with, so it round-trips.
- `yaml_serde` — parsing the bundled `palettes.yaml`. (Consolidating the two YAML crates is a ROADMAP item.)
- `wasm-bindgen-futures` — `spawn_local` for the file read, the run yield, and clipboard write.
- `web-sys` — DOM types for the file input, object URLs, and clipboard.
- `console_error_panic_hook` — readable panics in the console.

# Roadmap · webui

Planned work for the `webui` crate, ordered by value-to-effort. For what works, see [README.md](README.md).

The biggest item — moving the pipeline off the main thread — sits at the bottom despite high value: everything above it is cheaper and independently shippable.

---

## In Progress

### Bottom Bar

Add a bottom bar with info about the output image:

- Native resolution
- Actual resolution
- File size in human readable format (KB and MB)

### Viewport Bug Fix

Fix bug where uploading a new image after running the pipeline does not show the new image in the viewport.

### Web workers — move the pipeline off the main thread

The pipeline runs on the main thread, freezing the UI for a run's duration. A worker fixes it — but a worker is a separate thread with no shared memory, so this is a request/response restructuring, not a drop-in swap.

Shape:

1. **Worker entry point** — a separately-compiled artifact the browser loads. Receives source image bytes + the pipeline, runs decode → apply → encode, posts the PNG data URL back. The DOM-free helpers (`decode`, `encode_to_data_url`) move here largely unchanged — they were kept Leptos-free for exactly this.
2. **Run handler becomes a send**, not a compute: serialize inputs, post to the worker, return immediately. This is what removes the freeze.
3. **Result handler** receives the reply and writes `output_url`. The signal write migrates out of the click handler into this message handler, because the result now arrives asynchronously.

What crosses the boundary is serialized, so a live `RgbaImage` can't be sent. Plan: send the original encoded file bytes + the pipeline, let the worker own the whole decode/apply/encode chain, get back a string. **Decide early:** `source` may need to hold (or also retain) the original file bytes, not just the decoded `RgbaImage`. [`gloo-worker`](https://docs.rs/gloo-worker/) gives a typed request/response layer over raw `postMessage` and is the likely path.

The pipeline crosses this boundary as a `Vec<Operation>` (already `Serialize`), so serialization is solved — the fiddly part is the build system: a second target, and Trunk emitting and serving both artifacts. High value (the headline UX problem), gated behind real build work — hence bottom.

---

## Version 0.1.1 goals

### Viewport polish

- Clear image button
- Zoom controls
- Image translation (left/right/up/down in the viewport) with mouse.

### Palette file download

Allow the user to download any custom palette they've created.

### Undo/Redo buttons

Probably more work than it seems, but it would be worth it.

### Pipeline import

The YAML preview covers *export* — the displayed YAML round-trips with the CLI. Missing half is *import*: paste/drop a YAML pipeline and deserialize back into `rows`. More involved: it must parse a `Pipeline`, then rebuild each `OpRow` — and here's the wrinkle the value-bag introduces, a `Pipeline` holds typed `Operation`s, but `rows` holds `OpInstance` bags, so import needs the *inverse* of `to_operation()`: an `Operation -> OpInstance` conversion that doesn't exist yet. Plus fresh ids, and a decision on parse failure (surface the error, don't clobber the current pipeline). With both halves, the preview becomes a full export/import surface.

### Custom Save File

Further expanding on the `pipeline import` goal, create a custom file type (Or use some existing file type that makes sense for this purpose) to save an entire workflow, images included.

This will need a way to save/load files, and the associated UI.

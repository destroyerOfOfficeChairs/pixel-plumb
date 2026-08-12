# Architecture

A walk from the schema down to a running pipeline, framed around the design decision that shaped everything: the **value-bag**. Once that clicks, the rest falls into place.

This doc covers the *software structure* — how the UI stores and edits pipeline state. For the *color-science and algorithmic* rationale (why OkLab, why linear-light dithering, how adaptive palettes are generated), see [core's DESIGN.md](core/DESIGN.md). For how to run and build the frontend, see [webui's README](webui/README.md).

## The central tension

The `core` crate has a typed enum, `Operation`, that describes every possible thing the pipeline can do:

```rust
enum Operation {
    Downsample { pixel_size: u32 },
    Blur { sigma: f32 },
    PaletteMap { colors: Vec<String>, dither: Option<DitherConfig>, ... },
    // ...
}
```

This enum is *perfect* for the pipeline runner in `apply()` — it match-arms over each variant, calls the right function, image goes in one side and out the other. Every field is typed, every case is exhaustive, the compiler holds your hand.

But the UI has a different job. The UI needs to render a slider for `pixel_size`, another slider for `sigma`, a checkbox for `preserve_alpha`. And when the user drags a slider, the UI needs to *write back* into whichever field of whichever variant of the enum is currently being edited.

A generic slider component doesn't know that "the value it's editing" is `Operation::Blur::sigma`. So under a typed-first design, you'd need either a bespoke config component per op or a serde bridge that lets a generic slider read/write "the field named `sigma` of the currently-selected op." An earlier version of the codebase actually did the second — serialize the op to JSON, poke a field, deserialize back. It worked, but every edit paid a full serde round-trip.

**The value-bag is the alternative:** don't use the typed enum as the live UI state at all. Use a shape the UI can naturally read and write, and reconstruct the typed enum only at the moment you need it.

## The core side

`core/src/op_schema.rs` describes each op *as data*: name, params, each param's kind and range and default. This is separate from `Operation` the enum — the enum is the runtime type; the schema is metadata *about* the enum.

The schema types are small:

- `ParamDescriptor` — one param: its `key` (the field name), `label` (human string), and `kind`.
- `ParamKind` — an enum of what a param *is*: `Float { default, min, max, step }`, `Int { ... }`, `Bool { default }`, `Palette { colors }`, `Dither { default_tag }`, `Enum { options, default_tag }`. This is where widget-relevant metadata lives — the min/max/step a slider needs, the options a dropdown offers, which the type system alone can't express. `Enum` is the generic one-of-N choice (e.g. a palette op's `mapping_space`), distinct from `Dither` in that the chosen value carries no sub-parameters.
- `VariantDescriptor` — one op or dither variant: its `tag`, `label`, and `params` (a slice of `ParamDescriptor`).

Then two big `const` tables (in `tables.rs`):

- `OP_VARIANTS` — one entry per operation. Downsample has one param, palette_map has three, and so on. Every field name, every default, every slider range is here.
- `DITHER_VARIANTS` — same shape, one entry per dither algorithm.

These tables are the **single source of truth for what a param is.** The types file (`op_schema.rs`) says what shape the truth takes; the tables file has the actual data; and `labels.rs` has small helpers (`label_for_tag`, `all_op_menu`) that answer human-string questions.

A test guards the one place strings could drift silently: `dither_default_tags_exist` checks that every `ParamKind::Dither { default_tag }` names a real entry in `DITHER_VARIANTS`, and `enum_default_tags_are_in_options` checks that every `ParamKind::Enum`'s `default_tag` is one of its own options. Typo either and `cargo test` fails.

The rest of `core` is unchanged from a design perspective — the `Operation` enum, `apply()`, the actual image operations. `op_schema` is *additional* metadata riding alongside the runtime types, not a replacement for them.

## The webui side

Now the value-bag itself. In `op_instance.rs`:

```rust
pub struct OpInstance {
    pub tag: String,                                 // "blur"
    pub values: BTreeMap<String, ParamValue>,        // "sigma" → Num(1.0)
}

pub enum ParamValue {
    Num(f64),
    Bool(bool),
    Palette(Vec<String>),
    Dither(Option<DitherChoice>),
    Enum(String),                                   // "mapping_space" → Enum("rgb")
}
```

An op instance is a tag (which op it is) plus a map from field name to value. Every widget in the UI reads and writes `values[key]` — the slider for sigma does `values.get("sigma")` and, on drag, `values.insert("sigma", Num(new_value))`. No serde. No knowledge of which typed variant this is. Just: read a key, write a key.

The arms of `ParamValue` map onto the `ParamKind`s: `Float` and `Int` both fold into `Num(f64)` (the schema carries which is which), `Bool` becomes `Bool`, `Palette` becomes `Palette`, `Dither` becomes `Dither` (nesting a mini-instance for the chosen algorithm and its params), and `Enum` becomes `Enum(String)` holding the selected tag.

The webui's source-of-truth signals live at the root (in `main.rs` / `App`):

- `rows: signal(Vec<OpRow>)` — the pipeline. `OpRow` wraps `OpInstance` with a stable UI-only `id` for the keyed `<For/>`.
- `source: RwSignal<Option<Image>>` — the decoded input image.
- `output_url: RwSignal<Option<String>>` — the output as a data URL.
- `stage_urls`, `show_stages`, `active_stage` — per-op stage previews (the Stages bar): the encoded image after each op, whether the bar is shown, and which stage is selected for viewing.
- `is_running` — drives the run spinner.
- `stats` — resolution and file-size readout for the bottom bar.

The last three groups are viewport features layered on later; the first three are the original core. All are edited-invalidated together — any change to `rows` clears the stage previews and stats, since a fresh run is needed to recompute them.

The edit path is one closure, in `pipeline_list.rs`:

```rust
let edit_op = Callback::new(move |(id, key, value): EditPayload| {
    set_rows.update(|rows| {
        if let Some(r) = rows.iter_mut().find(|r| r.id == id) {
            r.inst.values.insert(key, value);
        }
    });
});
```

That's the *entire write path*. Every config card, every widget, every edit funnels through this one `values.insert`. Even nested things like dither commit the *whole* `ParamValue::Dither(Some(choice))` under the key `"dither"` — no nested path type, no recursive edit machinery.

## Widget generic-ness

Because the bag matches the schema shape, one config component handles all the *scalar* ops. `generic_config.rs` walks `OP_VARIANTS`, and for each param it renders a slider or checkbox by dispatching on the schema's `ParamKind`:

```rust
for param in variant.params {
    match param.kind {
        ParamKind::Float { .. } => <FloatSlider .../>,
        ParamKind::Int { .. } => <IntSlider .../>,
        ParamKind::Bool { .. } => <BoolWidget .../>,
        ParamKind::Enum { .. } => <EnumWidget .../>,   // generic dropdown
        // ...
    }
}
```

Adding a scalar op is one row in `OP_VARIANTS`. That's the payoff — and it extends past scalars: `Enum` is generically renderable too, so a one-of-N choice like the palette ops' `mapping_space` was added as a schema row plus a reusable `EnumWidget`, no bespoke code. (`Palette` and `Dither` are the kinds that *can't* be generic — a color-picker grid and a nested variant-with-sub-params — so they live only in bespoke cards.)

Three ops have **bespoke** config cards, dispatched by tag in `config.rs` before the generic fallback:

- `palette_map` — non-scalar params (a palette editor, a dither picker). Also shows a `mapping_space` dropdown, placed by hand since the card is bespoke.
- `adaptive_palette_map` — the palette-map card minus the palette editor, plus a "colors" count slider; reuses the same dither, preserve-alpha, and mapping-space widgets.
- `resize` — a two-mode layout (a checkbox toggles between a longest-side slider and exact width/height sliders, greying the inactive half) that the generic per-param loop can't express.

Even the bespoke cards reuse the generic scalar widgets (`BoolWidget`, `IntSlider`) rather than reimplementing them — the bespokeness is in the *layout*, not the controls.

The op menu is likewise schema-derived: `all_op_menu()` lists every op from `OP_VARIANTS`, and `default_instance(tag)` builds a fresh instance from the schema's defaults. The dropdown of addable operations and the starting values both come from the table — no separate list to maintain.

## The boundary — the one place types come back

`op_instance/boundary.rs` is where the bag becomes typed again. Every time the user hits Run, each `OpInstance` calls `to_operation()`, which hand-matches on `self.tag.as_str()` and reads each key out of the bag:

```rust
"blur" => Operation::Blur { sigma: self.f32_field("sigma")? },
```

`f32_field` looks up the key, expects a `Num`, narrows to `f32`. Returns `Result` because the bag *could* have a missing or wrong-typed key (imagine importing a YAML pipeline that predates a schema change). A malformed bag surfaces as a logged error at Run, not a panic.

This is the *only place* the schema-vs-bag contract is checked. If a key is missing, if a `ParamValue::Bool` shows up where a `Num` was expected, if the tag is unknown — this is where it fails. Everywhere else in the UI, the bag is just a `BTreeMap<String, ParamValue>` that you read and write freely.

That failure surface being *singular* is the point. Under a typed-first design, every widget-to-op interaction was a potential failure point (the serde round-trip could go wrong anywhere). Under the value-bag, every widget interaction is just a map operation, and the one place things *can* fail is the one place you already needed a boundary — the transition from "the user is editing this" to "the runtime needs to consume this."

## The run path

When the Run button in `pipeline_list.rs` fires:

1. Its `on_click` calls the `on_run` callback (passed down from `App`).
2. `App`'s `on_run` reads `source.get()`, then maps each row's `inst.to_operation()` — that's the boundary — collecting into `Vec<Operation>` or short-circuiting on the first `BuildError` (logged, run aborts).
3. It sets `is_running` and yields a frame (via `spawn_local` + a short timeout) so the spinner paints *before* the synchronous compute freezes the main thread — then runs `pixelizer_core::apply_stages(&pipeline, img)`, which returns the image after each op.
4. The final stage becomes `output_url`; all stages (prepended with the original) become `stage_urls` for the Stages bar; resolution and encoded size become `stats`. Encoding goes through `pixelizer_core::encode_png` (indexed when the output is small and opaque, see DESIGN.md).
5. The viewport reactively displays the result, and the Stages bar / bottom bar update.

The run is still **synchronous** — it blocks the main thread for its duration. The spinner makes that freeze legible; actually removing it (a web worker) is the biggest open ROADMAP item. The Run button lives in `PipelineList` (the child) but the *logic* lives in `App` (the root): `PipelineList` gets a `Callback<()>` to trigger and a `Signal<bool>` for the disabled state, and never holds `source` or `output_url` itself.

## Why this all coheres

The whole design turns on one substitution: instead of the UI state being *typed but generic access is expensive*, the UI state is *stringly-keyed and generic access is free, but you check the shape once at the boundary*.

That substitution is only good because the schema table (`OP_VARIANTS`) exists to *describe* the bag — so widgets aren't flying blind, they're driven by the same table that the boundary reader is going to validate against. The schema is the contract; the bag is the storage; the boundary is the enforcement.

The three-file splits make each of those parts findable on disk: `op_schema/tables.rs` is the contract, `op_instance.rs` is the storage type, `op_instance/boundary.rs` is the enforcement. If you come back to this in six months and want to know "where does the runtime check happen," the filename answers. If you want to know "what params does an op have," different filename. If you want to know "what shape can a value take," a third.

The payoff, again: the scalar ops share one config component, and adding another is one table row. Adding a new dither algorithm is one table row. Adding a one-of-N choice param (like `mapping_space`) is one table row plus the shared `EnumWidget`. The bespoke ops (`palette_map`, `adaptive_palette_map`, `resize`) are hand-written, but that bespokeness is *localized* to their own cards — it doesn't push its shape onto anything else, and even they reuse the shared scalar widgets.

That's the architecture, top to bottom.

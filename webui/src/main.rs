mod op_card;
mod op_instance;
mod pipeline_list;
mod viewport;

use leptos::prelude::*;
use pipeline_list::PipelineList;
use viewport::Viewport;

use pixelizer_core::Pipeline;

use crate::op_instance::{OpInstance, ParamValue, default_instance};
use crate::viewport::encode_to_data_url;

/// An edit emitted upward by a Config: set `key` on op `id` to `value`.
/// (id, key, value)
pub type EditPayload = (usize, String, ParamValue);

#[derive(Clone)]
pub struct OpRow {
    pub id: usize,
    pub inst: OpInstance,
}

#[derive(Clone)]
pub struct Palettes {
    palettes: Vec<(String, Vec<String>)>,
}

impl Palettes {
    fn load() -> Self {
        let raw = include_str!("../palettes.yaml");
        let map: std::collections::HashMap<String, Vec<String>> =
            yaml_serde::from_str(raw).expect("palettes.yaml failed to parse");
        let mut palettes: Vec<(String, Vec<String>)> = map.into_iter().collect();
        palettes.sort_by(|a, b| a.0.cmp(&b.0));
        Palettes { palettes }
    }
}

#[component]
fn App() -> impl IntoView {
    let (rows, set_rows) = signal(vec![OpRow {
        id: 0,
        // Safe: "downsample" is a known schema tag.
        inst: default_instance("downsample").expect("downsample is a known op"),
    }]);
    let source = RwSignal::new(None::<pixelizer_core::Image>);
    let output_url = RwSignal::new(None::<String>);
    // Encoded image after each pipeline stage, prepended with the original:
    // stage_urls[0] = original, stage_urls[i] = after op i-1. Empty = stale
    // (no fresh run, or edited since), which greys the Stages button.
    let stage_urls = RwSignal::new(Vec::<String>::new());
    // Whether the Stages bar is shown.
    let show_stages = RwSignal::new(false);
    // Which stage segment is selected for viewing. None = show final output.
    let active_stage = RwSignal::new(None::<usize>);
    // True while a run is computing. Drives the viewport spinner.
    let is_running = RwSignal::new(false);
    // Stats about the last run, shown in the bottom bar.
    let stats = RwSignal::new(viewport::ViewportStats::default());

    // Run the pipeline of operations on an image.
    let on_run = Callback::new(move |_: ()| {
        let Some(img) = source.get() else { return };

        let ops: Result<Vec<_>, _> = rows
            .get()
            .into_iter()
            .map(|r| r.inst.to_operation())
            .collect();
        let ops = match ops {
            Ok(ops) => ops,
            Err(e) => {
                leptos::logging::error!("couldn't build pipeline: {e}");
                return;
            }
        };

        // The trailing upscale factor (if the last op is an upscale) lets us
        // report the "native" pre-upscale resolution separately from the final.
        let trailing_upscale = match ops.last() {
            Some(pixelizer_core::Operation::Upscale { factor }) => Some(*factor),
            _ => None,
        };

        is_running.set(true);

        // Let the spinner paint before the synchronous run freezes the thread.
        // Awaiting a brief timeout yields to the event loop, so Leptos flushes
        // the is_running=true DOM update AND the browser paints it before the
        // compute below blocks. The spinner is pure CSS, so it keeps animating
        // through the freeze (compositor-driven, off the main thread).
        wasm_bindgen_futures::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(32).await;

            let pipeline = Pipeline { operations: ops };
            // apply_stages gives the image after each op; prepend the original
            // so the Stages bar reads [original, after op0, after op1, ...].
            match pixelizer_core::apply_stages(&pipeline, img.clone()) {
                Ok(stages) => {
                    let mut urls = vec![encode_to_data_url(&img)];

                    // Final output: encode with size for the stats bar.
                    let (final_url, file_size, dims) = match stages.last() {
                        Some(last) => {
                            let (url, size) = viewport::encode_to_data_url_sized(last);
                            (url, Some(size), Some((last.width(), last.height())))
                        }
                        // No ops: output is the original.
                        None => {
                            let (url, size) = viewport::encode_to_data_url_sized(&img);
                            (url, Some(size), Some((img.width(), img.height())))
                        }
                    };

                    urls.extend(stages.iter().map(encode_to_data_url));
                    // Replace the final URL with the sized-encoded one so we
                    // don't encode it twice.
                    if let Some(slot) = urls.last_mut() {
                        *slot = final_url.clone();
                    }
                    output_url.set(Some(final_url));
                    stage_urls.set(urls);

                    let upscaled = dims;
                    let native = match (dims, trailing_upscale) {
                        (Some((w, h)), Some(f)) if f > 0 => Some((w / f, h / f)),
                        _ => dims,
                    };
                    stats.set(viewport::ViewportStats {
                        native,
                        upscaled,
                        file_size,
                    });
                }
                Err(e) => leptos::logging::error!("pipeline failed: {e:?}"),
            }
            is_running.set(false);
        });
    });

    // Invalidate stage previews on any pipeline change. Every edit, add, remove,
    // and reorder goes through set_rows, so tracking `rows` catches them all.
    // Clears the stages (greys the button), hides the bar, and deselects.
    Effect::new(move |_| {
        rows.track();
        stage_urls.set(Vec::new());
        show_stages.set(false);
        active_stage.set(None);
        stats.set(viewport::ViewportStats::default());
    });

    // Whether a run is currently possible (no image = can't run).
    let can_run = Signal::derive(move || source.get().is_some());

    // Labels for the Stages bar: "Original" then each op's display name, in
    // pipeline order. Matches the stage_urls layout (original prepended).
    // Derived from rows; safe because any edit clears stage_urls (hiding the
    // bar), so labels and stages are only ever shown together in sync.
    let stage_labels = Signal::derive(move || {
        let mut v = vec!["Original".to_string()];
        v.extend(
            rows.get()
                .iter()
                .map(|r| pixelizer_core::op_schema::label_for_tag(&r.inst.tag).to_string()),
        );
        v
    });

    view! {
        // App shell: fills the window, never scrolls itself.
        <div class="h-screen overflow-hidden flex gap-6 p-6">
            // Pipeline pane: fixed width, full height. Scrolls internally
            // (below its pinned header), managed inside PipelineList.
            <div class="shrink-0 h-full">
                <PipelineList
                    rows=rows
                    set_rows=set_rows
                    on_run=on_run
                    can_run=can_run
                />
            </div>
            // Viewport pane: fills the rest, fixed. Manages its own internal
            // stack (toolbar / stages bar / image).
            <div class="flex-1 min-w-0 overflow-hidden">
                <Viewport
                    source=source
                    output_url=output_url
                    stage_urls=stage_urls
                    stage_labels=stage_labels
                    show_stages=show_stages
                    active_stage=active_stage
                    is_running=is_running
                    stats=stats
                />
            </div>
        </div>
    }
}
fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| {
        provide_context(RwSignal::new(Palettes::load()));
        view! { <App/> }
    });
}

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

        let pipeline = Pipeline { operations: ops };
        // synchronous — UI freezes here for a few seconds. Known.
        // apply_stages gives the image after each op; prepend the original so
        // the Stages bar reads [original, after op0, after op1, ...].
        match pixelizer_core::apply_stages(&pipeline, img.clone()) {
            Ok(stages) => {
                let mut urls = vec![encode_to_data_url(&img)];
                urls.extend(stages.iter().map(encode_to_data_url));
                if let Some(last) = urls.last() {
                    output_url.set(Some(last.clone()));
                }
                stage_urls.set(urls);
            }
            Err(e) => leptos::logging::error!("pipeline failed: {e:?}"),
        }
    });

    // Invalidate stage previews on any pipeline change. Every edit, add, remove,
    // and reorder goes through set_rows, so tracking `rows` catches them all.
    // Clears the stages (greys the button), hides the bar, and deselects.
    Effect::new(move |_| {
        rows.track();
        stage_urls.set(Vec::new());
        show_stages.set(false);
        active_stage.set(None);
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
            // Pipeline pane: fixed width, scrolls internally when ops overflow.
            <div class="shrink-0 overflow-y-auto">
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

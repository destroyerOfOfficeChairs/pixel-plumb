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
        match pixelizer_core::apply(&pipeline, img) {
            Ok(result) => output_url.set(Some(encode_to_data_url(&result))),
            Err(e) => leptos::logging::error!("pipeline failed: {e:?}"),
        }
    });

    // Whether a run is currently possible (no image = can't run).
    let can_run = Signal::derive(move || source.get().is_some());

    view! {
        // App shell: fills the window, never scrolls itself. The two panes
        // manage their own overflow.
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
            // Viewport pane: fills the rest, fixed (doesn't scroll with the
            // pipeline). Stacked as [toolbar] / [image] / [status]; the bars
            // are placeholders here until their components land.
            <div class="flex-1 flex flex-col overflow-hidden">
                // <ViewportToolbar/> goes here
                <div class="flex-1 overflow-hidden">
                    <Viewport source=source output_url=output_url/>
                </div>
                // <ViewportStatus/> goes here
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

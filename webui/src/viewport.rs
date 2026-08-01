use base64::{Engine, engine::general_purpose::STANDARD};
use leptos::prelude::*;
use pixelizer_core::image::ImageFormat;
use pixelizer_core::image::{self};
use std::io::Cursor;

mod bottom_bar;
mod stages_bar;
mod toolbar;
pub use bottom_bar::ViewportStats;
use bottom_bar::ViewportStatus;
use stages_bar::StagesBar;
use toolbar::ViewportToolbar;

pub fn encode_to_data_url(img: &pixelizer_core::Image) -> String {
    encode_to_data_url_sized(img).0
}

/// Like `encode_to_data_url` but also returns the raw PNG byte length (before
/// base64), for the file-size stat in the bottom bar.
pub fn encode_to_data_url_sized(img: &pixelizer_core::Image) -> (String, usize) {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("PNG encode");
    let size = buf.len();
    (
        format!("data:image/png;base64,{}", STANDARD.encode(&buf)),
        size,
    )
}

fn decode(bytes: &[u8]) -> Option<pixelizer_core::Image> {
    match image::load_from_memory(bytes) {
        Ok(img) => Some(img.to_rgba8()),
        Err(e) => {
            leptos::logging::error!("decode failed: {e}");
            None
        }
    }
}

#[component]
pub fn Viewport(
    source: RwSignal<Option<pixelizer_core::Image>>,
    output_url: RwSignal<Option<String>>,
    /// Encoded per-stage images: [original, after op0, after op1, ...].
    /// Empty = stale (greys the Stages button).
    stage_urls: RwSignal<Vec<String>>,
    /// Labels matching stage_urls 1:1 (["Original", op name, ...]).
    stage_labels: Signal<Vec<String>>,
    /// Whether the Stages bar is visible.
    show_stages: RwSignal<bool>,
    /// Selected stage segment. None = show final output.
    active_stage: RwSignal<Option<usize>>,
    /// Stats about the last run, shown in the bottom bar.
    stats: RwSignal<ViewportStats>,
) -> impl IntoView {
    // Instant-display object URL of the uploaded file (a pointer to the blob,
    // no re-encode). Shown until a run produces output. Revoked on replacement.
    let source_url = RwSignal::new(None::<String>);
    let filename = RwSignal::new(None::<String>);

    let on_file = Callback::new(move |file: web_sys::File| {
        if let Ok(url) = web_sys::Url::create_object_url_with_blob(&file) {
            if let Some(old) = source_url.get_untracked() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
            source_url.set(Some(url));
        }
        // A new image invalidates the previous run: clear the output so the new
        // source shows, and drop the stale stages (greys the Stages button).
        output_url.set(None);
        stage_urls.set(Vec::new());
        show_stages.set(false);
        active_stage.set(None);
        let gloo_file = gloo_file::File::from(file);
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(bytes) = gloo_file::futures::read_as_bytes(&gloo_file).await {
                if let Some(img) = decode(&bytes) {
                    source.set(Some(img));
                }
            }
        });
    });

    // Whether a fresh run's stages exist (drives the Stages button's enabled
    // state).
    let stages_available = Signal::derive(move || !stage_urls.with(Vec::is_empty));

    // What to display: a selected stage takes precedence, then the final
    // output, then the instant source object URL.
    let display_url = move || {
        if let Some(i) = active_stage.get() {
            if let Some(u) = stage_urls.with(|s| s.get(i).cloned()) {
                return Some(u);
            }
        }
        output_url.get().or_else(|| source_url.get())
    };

    // When the stages bar is hidden, drop any selection so the viewport
    // returns to the final output.
    Effect::new(move || {
        if !show_stages.get() {
            active_stage.set(None);
        }
    });

    view! {
        <div class="h-full flex flex-col overflow-hidden">
            <ViewportToolbar
                on_file=on_file
                filename=filename
                show_stages=show_stages
                stages_available=stages_available
            />

            {move || show_stages.get().then(|| view! {
                <StagesBar stage_urls=stage_urls labels=stage_labels active_stage=active_stage/>
            })}

            <div class="flex-1 min-h-0 flex items-center justify-center overflow-hidden p-3">
                {move || display_url().map(|url| view! {
                    <img
                        src=url
                        class="w-full h-full object-contain [image-rendering:pixelated]"
                    />
                })}
            </div>

            <ViewportStatus stats=stats.into()/>
        </div>
    }
}

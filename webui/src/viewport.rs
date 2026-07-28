use base64::{Engine, engine::general_purpose::STANDARD};
use leptos::prelude::*;
use pixelizer_core::image::ImageFormat;
use pixelizer_core::image::{self};
use std::io::Cursor;

pub fn encode_to_data_url(img: &pixelizer_core::Image) -> String {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .expect("PNG encode");
    format!("data:image/png;base64,{}", STANDARD.encode(&buf))
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
) -> impl IntoView {
    let on_file = move |ev: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = event_target(&ev);
        if let Some(file_list) = input.files() {
            if let Some(file) = file_list.get(0) {
                let gloo_file = gloo_file::File::from(file);
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(bytes) = gloo_file::futures::read_as_bytes(&gloo_file).await {
                        if let Some(img) = decode(&bytes) {
                            source.set(Some(img));
                        }
                    }
                });
            }
        }
    };

    // What to display: prefer the processed output; fall back to the decoded
    // source so the image shows immediately on upload, before any run.
    // Returns an Option<String> data URL. Re-runs only when output_url or
    // source changes — so the source re-encode happens once per new image,
    // not on every render.
    let display_url = move || {
        output_url
            .get()
            .or_else(|| source.get().as_ref().map(encode_to_data_url))
    };

    view! {
        <div class="h-full flex flex-col gap-3 overflow-hidden">
            <input type="file" accept="image/*" on:change=on_file
                class="text-sm text-slate-300 shrink-0"/>
            <div class="flex-1 flex items-center justify-center overflow-hidden">
                {move || display_url().map(|url| view! {
                    <img
                        src=url
                        class="w-full h-full object-contain [image-rendering:pixelated]"
                    />
                })}
            </div>
        </div>
    }
}

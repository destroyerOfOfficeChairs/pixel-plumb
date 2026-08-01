use leptos::prelude::*;

/// The viewport's top toolbar. Left-justified controls so more can be added
/// (zoom, etc.) without reshuffling. Currently: a styled "Add image" button
/// with the loaded filename beside it, a divider, and a toggle for the Stages
/// bar (per-op pipeline previews).
///
/// The toolbar owns the file `<input>` and reports the chosen file upward via
/// `on_file`; it doesn't decode anything itself. It sets `filename` for display.
#[component]
pub fn ViewportToolbar(
    /// Called with the chosen file. The parent decodes / displays it.
    on_file: Callback<web_sys::File>,
    /// The loaded file's name, shown beside the button. Set here on selection.
    filename: RwSignal<Option<String>>,
    /// Whether the Stages bar is shown. Toggled by the button here.
    show_stages: RwSignal<bool>,
    /// Whether a fresh run's stages exist. When false, the Stages button is
    /// disabled (greyed) — there's nothing to show until the next run.
    stages_available: Signal<bool>,
) -> impl IntoView {
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();

    let on_change = move |ev: leptos::ev::Event| {
        let input: web_sys::HtmlInputElement = event_target(&ev);
        if let Some(files) = input.files() {
            if let Some(file) = files.get(0) {
                filename.set(Some(file.name()));
                on_file.run(file);
            }
        }
    };

    view! {
        <div class="shrink-0 flex items-center gap-3 px-3 py-2 border-b border-slate-800">
            // Hidden real input; the button below triggers it.
            <input
                node_ref=input_ref
                type="file"
                accept="image/*"
                class="hidden"
                on:change=on_change
            />

            // Styled "Add image" button
            <button
                class="flex items-center gap-2 bg-teal-600 hover:bg-teal-500 text-white \
                       text-sm font-medium rounded-md px-3 py-1.5"
                on:click=move |ev| {
                    ev.stop_propagation();
                    if let Some(input) = input_ref.get() {
                        input.click();
                    }
                }
            >
                // upload glyph
                <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4"
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round"
                        d="M12 16V4m0 0L8 8m4-4l4 4M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2" />
                </svg>
                "Add image"
            </button>

            // Filename (or a muted placeholder)
            <span class="text-sm text-slate-400 truncate max-w-[16rem]">
                {move || filename.get().unwrap_or_else(|| "No image loaded".to_string())}
            </span>

            // Divider
            <div class="w-px h-6 bg-slate-700"></div>

            // Stages toggle — disabled (greyed) until a fresh run exists.
            <button
                disabled=move || !stages_available.get()
                class=move || {
                    let base = "flex items-center gap-2 text-sm rounded-md px-3 py-1.5 border";
                    if !stages_available.get() {
                        format!("{base} border-slate-800 text-slate-600 cursor-not-allowed")
                    } else if show_stages.get() {
                        format!("{base} bg-slate-700 border-slate-600 text-slate-100")
                    } else {
                        format!("{base} border-slate-700 text-slate-400 hover:text-slate-200")
                    }
                }
                on:click=move |ev| {
                    ev.stop_propagation();
                    if stages_available.get_untracked() {
                        show_stages.update(|s| *s = !*s);
                    }
                }
            >
                // layers glyph
                <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4"
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round"
                        d="M12 4l8 4-8 4-8-4 8-4zM4 12l8 4 8-4M4 16l8 4 8-4" />
                </svg>
                "Stages"
            </button>
        </div>
    }
}

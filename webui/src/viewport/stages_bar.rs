use leptos::prelude::*;

/// The Stages bar: a horizontal filmstrip of pipeline stages. Each segment is a
/// thumbnail of that stage's image with a label underneath (truncated with an
/// ellipsis if long). Clicking a segment selects it for viewing; the active one
/// gets a teal border. Segments stop click propagation so clicking elsewhere in
/// the viewport deselects (handled by the parent).
///
/// `stage_urls` is [original, after op0, after op1, ...] and `labels` matches
/// it 1:1 (["Original", "downsample", ...]). If labels is shorter, missing ones
/// fall back to the index.
#[component]
pub fn StagesBar(
    stage_urls: RwSignal<Vec<String>>,
    labels: Signal<Vec<String>>,
    active_stage: RwSignal<Option<usize>>,
) -> impl IntoView {
    view! {
        <div class="shrink-0 border-b border-slate-800 overflow-x-auto">
            <div class="flex gap-2 p-2">
                {move || {
                    stage_urls.get().into_iter().enumerate().map(|(i, url)| {
                        let label = labels.with(|l| l.get(i).cloned())
                            .unwrap_or_else(|| format!("Stage {i}"));
                        let is_active = move || active_stage.get() == Some(i);
                        view! {
                            <button
                                class=move || {
                                    let base = "shrink-0 flex flex-col items-center gap-1 \
                                                rounded-md p-1 border-2 w-20";
                                    if is_active() {
                                        format!("{base} border-teal-400")
                                    } else {
                                        format!("{base} border-transparent hover:border-slate-600")
                                    }
                                }
                                on:click=move |_| {
                                    active_stage.update(|a| {
                                        *a = if *a == Some(i) { None } else { Some(i) };
                                    });
                                }
                            >
                                <img
                                    src=url
                                    class="w-16 h-16 object-contain bg-slate-900 rounded \
                                           [image-rendering:pixelated]"
                                />
                                <span class="w-full text-center text-[10px] text-slate-400 truncate">
                                    {label}
                                </span>
                            </button>
                        }
                    }).collect_view()
                }}
            </div>
        </div>
    }
}

use leptos::prelude::*;

/// Stats about the last pipeline run, shown in the viewport's bottom bar.
/// `None` fields render as a dash — before a run there's nothing to show.
#[derive(Clone, Copy, Default)]
pub struct ViewportStats {
    /// Output resolution *before* any final upscale — the "real" pixel-art
    /// resolution (e.g. 64×64 for an image upscaled to 512×512 for display).
    pub native: Option<(u32, u32)>,
    /// Full output resolution after upscaling. Only meaningfully different from
    /// `native` when the pipeline upscales; when equal, the bar omits it.
    pub upscaled: Option<(u32, u32)>,
    /// Encoded output size in bytes.
    pub file_size: Option<usize>,
}

/// Human-readable byte size: 1234 -> "1.2 KB".
fn format_bytes(n: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let n = n as f64;
    if n >= MB {
        format!("{:.1} MB", n / MB)
    } else if n >= KB {
        format!("{:.1} KB", n / KB)
    } else {
        format!("{n} B")
    }
}

/// The viewport's bottom status bar. Right-justified stats; left side is free
/// for future additions (zoom level, cursor position, etc.).
#[component]
pub fn ViewportStatus(stats: Signal<ViewportStats>) -> impl IntoView {
    view! {
        <div class="shrink-0 flex items-center justify-end gap-4 px-3 py-1.5 \
                    border-t border-slate-800 text-xs text-slate-400">
            {move || {
                let s = stats.get();
                // Show the upscaled resolution only when it differs from native.
                let show_upscaled = match (s.native, s.upscaled) {
                    (Some(n), Some(u)) => n != u,
                    _ => false,
                };
                view! {
                    <span>
                        "Resolution: "
                        {s.native
                            .map(|(w, h)| format!("{w}×{h}"))
                            .unwrap_or_else(|| "—".to_string())}
                    </span>
                    {show_upscaled.then(|| {
                        let (w, h) = s.upscaled.unwrap();
                        view! { <span>{format!("Upscaled: {w}×{h}")}</span> }
                    })}
                    <span>
                        "Size: "
                        {s.file_size
                            .map(format_bytes)
                            .unwrap_or_else(|| "—".to_string())}
                    </span>
                }
            }}
        </div>
    }
}

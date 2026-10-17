use leptos::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

pub const MAX_TOASTS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    pub detail: Option<String>,
}

static TOASTS: OnceLock<RwSignal<Vec<Toast>>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn init() {
    let _ = TOASTS.set(create_rw_signal(Vec::<Toast>::new()));
}

#[must_use]
pub fn stack() -> Option<RwSignal<Vec<Toast>>> {
    TOASTS.get().copied()
}

fn apply_push(stack: &mut Vec<Toast>, toast: Toast) {
    stack.push(toast);
    if stack.len() > MAX_TOASTS {
        let drop_n = stack.len() - MAX_TOASTS;
        stack.drain(..drop_n);
    }
}

fn apply_dismiss(stack: &mut Vec<Toast>, id: u64) {
    stack.retain(|t| t.id != id);
}

#[must_use]
pub fn auto_dismiss_ms(kind: ToastKind) -> u64 {
    if kind == ToastKind::Error {
        10_000
    } else {
        5_000
    }
}

pub fn push(kind: ToastKind, message: impl Into<String>, detail: Option<String>) {
    let Some(stack) = stack() else { return };
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let toast = Toast {
        id,
        kind,
        message: message.into(),
        detail,
    };
    stack.update(|v| apply_push(v, toast));
    let delay = auto_dismiss_ms(kind);
    set_timeout(move || dismiss(id), std::time::Duration::from_millis(delay));
}

pub fn success(message: impl Into<String>) {
    push(ToastKind::Success, message, None);
}

pub fn error(message: impl Into<String>) {
    push(ToastKind::Error, message, None);
}

pub fn warning(message: impl Into<String>) {
    push(ToastKind::Warning, message, None);
}

pub fn info(message: impl Into<String>) {
    push(ToastKind::Info, message, None);
}

pub fn dismiss(id: u64) {
    if let Some(stack) = stack() {
        stack.update(|v| apply_dismiss(v, id));
    }
}

#[component]
pub fn Toasts() -> impl IntoView {
    let stack = TOASTS
        .get()
        .copied()
        .expect("toasts::init() must run in app_view before <Toasts/> mounts");
    view! {
        <div class="spp-toast-container" aria-live="polite">
            <For
                each=move || stack.get()
                key=|t| t.id
                children=move |t: Toast| {
                    let id = t.id;
                    let class = format!("spp-toast spp-toast--{}", t.kind.as_str());
                    let detail_view = t.detail.as_deref().map(|d| {
                        view! {
                            <details class="spp-toast__details">
                                <summary class="spp-muted spp-text-xs">"Technical details"</summary>
                                <div class="spp-toast__detail-body">{d.to_string()}</div>
                            </details>
                        }
                        .into_view()
                    });
                    view! {
                        <div class=class role="status">
                            <div class="spp-toast__row">
                                <div class="spp-toast__body">
                                    {t.message.clone()}
                                    {detail_view}
                                </div>
                                <button
                                    class="spp-button spp-button--ghost spp-button--tiny"
                                    aria-label="Dismiss"
                                    on:click=move |_| dismiss(id)
                                >
                                    "\u{d7}"
                                </button>
                            </div>
                        </div>
                    }
                }
            />
        </div>
    }
}

use std::sync::Arc;

use leptos::ev;
use leptos::prelude::*;

#[derive(Clone)]
pub enum ViewState {
    Loading,
    Empty {
        message: String,
    },
    Error {
        message: String,
        retry: Option<RetryAction>,
    },
    Loaded,
}

pub type RetryAction = Arc<dyn Fn() + Send + Sync>;

impl std::fmt::Debug for ViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading => write!(f, "ViewState::Loading"),
            Self::Empty { message } => f
                .debug_struct("ViewState::Empty")
                .field("message", message)
                .finish(),
            Self::Error { message, retry } => f
                .debug_struct("ViewState::Error")
                .field("message", message)
                .field("retry", &retry.is_some())
                .finish(),
            Self::Loaded => write!(f, "ViewState::Loaded"),
        }
    }
}

impl ViewState {
    #[must_use]
    pub fn loading() -> Self {
        Self::Loading
    }

    #[must_use]
    pub fn empty(message: impl Into<String>) -> Self {
        Self::Empty {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            retry: None,
        }
    }

    #[must_use]
    pub fn error_with_retry(
        message: impl Into<String>,
        retry: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::Error {
            message: message.into(),
            retry: Some(Arc::new(retry)),
        }
    }
}

#[component]
pub fn StateView(#[prop(into)] state: Signal<ViewState>, children: ChildrenFn) -> impl IntoView {
    move || match state.get() {
        ViewState::Loading => view! { <LoadingState /> }.into_any(),
        ViewState::Empty { message } => view! {
            <EmptyState message=message.clone() />
        }
        .into_any(),
        ViewState::Error { message, retry } => view! {
            <ErrorState message=message.clone() retry=retry.clone() />
        }
        .into_any(),
        ViewState::Loaded => children().into_any(),
    }
}

#[component]
pub fn LoadingState() -> impl IntoView {
    view! {
        <div class="spp-state">
            <span class="spp-spinner" aria-label="Loading"></span>
            <p class="spp-state__body">"Loading…"</p>
        </div>
    }
}

#[component]
pub fn EmptyState(#[prop(into)] message: String) -> impl IntoView {
    view! {
        <div class="spp-state">
            <span class="spp-state__icon" aria-hidden="true">"∅"</span>
            <p class="spp-state__body">{message}</p>
        </div>
    }
}

#[component]
pub fn ErrorState(#[prop(into)] message: String, retry: Option<RetryAction>) -> impl IntoView {
    let retry_signal = RwSignal::new(retry);
    let on_retry = move |_| {
        if let Some(cb) = retry_signal.get() {
            cb();
        }
    };
    view! {
        <div class="spp-state spp-state--error">
            <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
            <h3 class="spp-state__title">"Something went wrong"</h3>
            <p class="spp-state__body">{message}</p>
            <Show when=move || retry_signal.get().is_some() fallback=|| ()>
                <button class="spp-button spp-button--ghost" on:click=on_retry>
                    "Try again"
                </button>
            </Show>
        </div>
    }
}

#[component]
pub fn RelativeTime(#[prop(into)] iso: String) -> impl IntoView {
    let label = format_relative_time(&iso);
    let dt = iso.clone();
    view! {
        <time datetime=dt class="spp-relative-time" title=iso>
            {label}
        </time>
    }
}

#[must_use]
pub fn format_relative_time(iso: &str) -> String {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let then_secs = parse_iso_to_epoch(iso).unwrap_or(now_secs);

    if now_secs <= then_secs {
        return "just now".to_string();
    }
    let diff = now_secs - then_secs;
    if diff < 60 {
        format!("{diff} second{} ago", if diff == 1 { "" } else { "s" })
    } else if diff < 3600 {
        let m = diff / 60;
        format!("{m} minute{} ago", if m == 1 { "" } else { "s" })
    } else if diff < 86_400 {
        let h = diff / 3600;
        format!("{h} hour{} ago", if h == 1 { "" } else { "s" })
    } else if diff < 604_800 {
        let d = diff / 86_400;
        format!("{d} day{} ago", if d == 1 { "" } else { "s" })
    } else if diff < 2_592_000 {
        let w = diff / 604_800;
        format!("{w} week{} ago", if w == 1 { "" } else { "s" })
    } else if diff < 31_536_000 {
        let mo = diff / 2_592_000;
        format!("{mo} month{} ago", if mo == 1 { "" } else { "s" })
    } else {
        let y = diff / 31_536_000;
        format!("{y} year{} ago", if y == 1 { "" } else { "s" })
    }
}

fn parse_iso_to_epoch(iso: &str) -> Option<u64> {
    let s = iso.trim_end_matches('Z');
    let (date_part, time_part) = s.split_once(['T', ' ']).unwrap_or((s, "00:00:00"));
    let (year, month, day) = {
        let mut parts = date_part.split('-');
        let y: u64 = parts.next()?.parse().ok()?;
        let m: u64 = parts.next()?.parse().ok()?;
        let d: u64 = parts.next()?.parse().ok()?;
        (y, m, d)
    };
    let (hour, min, sec) = {
        let mut parts = time_part.split(':');
        let h: u64 = parts.next()?.parse().ok()?;
        let mi: u64 = parts.next()?.parse().ok()?;
        let se: u64 = parts
            .next()
            .unwrap_or("0")
            .split('.')
            .next()?
            .parse()
            .ok()?;
        (h, mi, se)
    };
    let y = if month <= 2 { year - 1 } else { year };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era as i64 * 146097 + doe as i64 - 719468;
    let secs = days as u64 * 86_400 + hour * 3600 + min * 60 + sec;
    Some(secs)
}

#[component]
pub fn AppErrorBoundary(children: Children) -> impl IntoView {
    let fallback = move |_: ArcRwSignal<Errors>| {
        view! {
            <div class="spp-app-error">
                <h2>"Something went wrong"</h2>
                <p>"An unexpected error occurred while rendering this section."</p>
                <p class="spp-app-error__hint">
                    "Try refreshing the page. If the problem persists, check the application logs."
                </p>
            </div>
        }
    };
    view! {
        <ErrorBoundary fallback=fallback>
            {children()}
        </ErrorBoundary>
    }
}

#[component]
pub fn KeyboardRow(
    on_activate: impl Fn() + 'static + Clone + Send + Sync,
    children: Children,
) -> impl IntoView {
    let on_enter = on_activate.clone();
    let on_space = on_activate;
    let handle_keydown = move |ev: ev::KeyboardEvent| {
        if ev.key() == "Enter" {
            ev.prevent_default();
            on_enter();
        }
    };
    let handle_keyup = move |ev: ev::KeyboardEvent| {
        if ev.key() == " " {
            ev.prevent_default();
            on_space();
        }
    };
    view! {
        <div
            class="spp-keyboard-row"
            tabindex="0"
            role="button"
            on:keydown=handle_keydown
            on:keyup=handle_keyup
        >
            {children()}
        </div>
    }
}

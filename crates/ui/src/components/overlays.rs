use std::sync::Arc;

use leptos::ev;
use leptos::prelude::*;

#[component]
pub fn ConfirmDialog(
    #[prop(into)] title: String,
    #[prop(into)] message: String,
    #[prop(default = "Confirm".to_string(), into)] confirm_label: String,
    #[prop(default = false)] danger: bool,
    on_confirm: Arc<dyn Fn() + Send + Sync>,
    on_cancel: Arc<dyn Fn() + Send + Sync>,
) -> impl IntoView {
    let confirm = Arc::clone(&on_confirm);
    let cancel_for_close = Arc::clone(&on_cancel);
    let cancel_for_btn = Arc::clone(&on_cancel);
    view! {
        <Modal title=title.clone() on_close=move || cancel_for_close()>
            <p class="spp-modal__message">{message}</p>
            <div class="spp-modal__actions">
                <button class="spp-button" on:click=move |_| cancel_for_btn()>
                    "Cancel"
                </button>
                <button
                    class=move || {
                        format!(
                            "spp-button spp-button--primary{}",
                            if danger { " spp-button--danger" } else { "" }
                        )
                    }
                    on:click=move |_| confirm()
                >
                    {confirm_label}
                </button>
            </div>
        </Modal>
    }
}

#[component]
pub fn Modal(
    #[prop(into)] title: String,
    children: Children,
    on_close: impl Fn() + 'static + Send + Sync + Clone,
) -> impl IntoView {
    let on_close_esc = on_close.clone();
    let on_close_back = on_close.clone();

    let on_keydown = move |ev: ev::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            on_close_esc();
        }
    };

    let on_backdrop_click = move |ev: ev::MouseEvent| {
        let ct = ev.current_target();
        let t = ev.target();
        if ct == t {
            on_close_back();
        }
    };

    view! {
        <div
            class="spp-overlay"
            role="dialog"
            aria-modal="true"
            aria-label=title.clone()
            tabindex="-1"
            on:keydown=on_keydown
            on:click=on_backdrop_click
        >
            <div class="spp-modal" tabindex="0">
                <h3 class="spp-modal__title">{title.clone()}</h3>
                {children()}
            </div>
        </div>
    }
}

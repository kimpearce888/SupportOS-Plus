use leptos::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ButtonStyle {
    #[default]
    Primary,
    Ghost,
}

impl ButtonStyle {
    fn class(self) -> &'static str {
        match self {
            Self::Primary => "spp-button",
            Self::Ghost => "spp-button spp-button--ghost",
        }
    }
}

#[component]
pub fn Button<F>(
    on_click: F,
    #[prop(default = ButtonStyle::default())] style: ButtonStyle,
    #[prop(default = false)] disabled: bool,
    #[prop(into, optional)] aria_label: Option<String>,
    children: Children,
) -> impl IntoView
where
    F: Fn() + 'static,
{
    view! {
        <button
            class=style.class()
            disabled=disabled
            aria-label=aria_label
            on:click=move |_| on_click()
        >
            {children()}
        </button>
    }
}

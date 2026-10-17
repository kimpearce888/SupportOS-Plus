use leptos::prelude::*;

#[component]
pub fn NotFoundPage<F>(path: F) -> impl IntoView
where
    F: Fn() -> String + Send + Sync + 'static,
{
    view! {
        <div class="spp-not-found">
            <p class="spp-not-found__code">"404"</p>
            <h2 class="spp-state__title">"Page not found"</h2>
            <p class="spp-state__body">
                "The route " <code>{move || path()}</code> " does not exist."
            </p>
        </div>
    }
}

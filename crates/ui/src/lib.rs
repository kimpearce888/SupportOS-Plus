#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

use leptos::*;
use leptos_router::*;

pub mod api;
pub mod components;
pub mod layout;
pub mod pages;
pub mod queries;
pub mod shortcuts;
pub mod sse;
pub mod sse_bridge;
pub mod state;
pub mod toasts;
pub mod url_state;

pub use spp_catalog as catalog;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    mount();
}

pub fn mount() {
    console_error_panic_hook::set_once();
    state::apply_theme_at_boot();
    mount_to_body(app_view);
}

fn app_view() -> impl IntoView {
    let ui = state::UiState::create();
    provide_context(ui);
    toasts::init();
    queries::init();

    view! {
        <Router>
            <AppEffects />
            <Routes>
                <Route path="/onboarding" view=pages::OnboardingPage />
                <Route path="/" view=layout::LayoutShell>
                    <Route path="/" view=pages::DashboardPage />
                    <Route path="/inbox" view=InboxListRoute />
                    <Route path="/inbox/conversation/:id" view=InboxRoute />
                    <Route path="/search" view=pages::SearchPage />
                    <Route path="/customers" view=pages::CustomerSearchPage />
                    <Route path="/customers/:id" view=CustomerProfileRoute />
                    <Route path="/organizations" view=pages::OrganizationsPage />
                    <Route path="/organizations/:id" view=OrganizationDetailRoute />
                    <Route path="/ai" view=pages::AiCenterPage />
                    <Route path="/issues" view=pages::IssueRadarPage />
                    <Route path="/incidents" view=pages::IncidentsPage />
                    <Route path="/incidents/:id" view=IncidentDetailRoute />
                    <Route path="/custom-objects" view=pages::CustomObjectsPage />
                    <Route path="/connectors" view=pages::ConnectorsPage />
                    <Route path="/graph" view=pages::SupportGraphPage />
                    <Route path="/knowledge" view=pages::KnowledgePage />
                    <Route path="/docs" view=pages::DocsPage />
                    <Route path="/reports" view=pages::ReportsPage />
                    <Route path="/outreach" view=pages::OutreachPage />
                    <Route path="/operations" view=pages::OperationsPage />
                    <Route path="/notifications" view=pages::NotificationsPage />
                    <Route path="/automation" view=pages::AutomationPage />
                    <Route path="/sync-health" view=pages::SyncHealthPage />
                    <Route path="/settings" view=pages::SettingsPage />
                    <Route path="/*any" view=not_found_from_params />
                </Route>
            </Routes>
            <toasts::Toasts />
            <components::CommandPalette
                open=ui.palette_open
                on_close=move || ui.palette_open.set(false)
            />
            <div class="spp-shortcut-bar" aria-hidden="true">
                <span><span class="spp-kbd">"⌘K"</span> " search"</span>
                <span><span class="spp-kbd">"/"</span> " quick search"</span>
                <span><span class="spp-kbd">"g"</span><span class="spp-kbd">"i"</span> " inbox"</span>
                <span><span class="spp-kbd">"g"</span><span class="spp-kbd">"d"</span> " dashboard"</span>
            </div>
        </Router>
    }
}

#[component]
fn AppEffects() -> impl IntoView {
    let ui = use_context::<state::UiState>()
        .expect("AppEffects requires the UiState context (provided by app_view)");
    let location = use_location();
    let navigate = use_navigate();

    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/onboarding").await {
                let completed = v.get("completed").and_then(|c| c.as_bool());
                ui.onboarding_completed.set(completed);
            }
        });
    }
    {
        let pathname = location.pathname;
        let navigate = navigate.clone();
        create_effect(move |_| {
            if ui.onboarding_completed.get() == Some(false) && pathname.get() != "/onboarding" {
                navigate(
                    "/onboarding",
                    NavigateOptions {
                        replace: true,
                        ..Default::default()
                    },
                );
            }
        });
    }

    ui.refresh_nav_counts();
    set_interval(
        move || ui.refresh_nav_counts(),
        std::time::Duration::from_secs(30),
    );

    {
        let _unused_unsubscribe = crate::sse::subscribe(Box::new(move |event| {
            crate::sse_bridge::handle_event(&ui, event);
        }));
    }

    let pending_g = std::rc::Rc::new(std::cell::Cell::new(false));
    view! {
        <div
            class="spp-app-effects"
            on:window:keydown=move |e: leptos::ev::KeyboardEvent| {
                let outcome = shortcuts::resolve(
                    pending_g.get(),
                    shortcuts::is_typing(e.target().as_ref()),
                    (e.meta_key() || e.ctrl_key()) && e.key().eq_ignore_ascii_case("k"),
                    &e.key(),
                );
                pending_g.set(outcome.pending_g);
                match outcome.action {
                    shortcuts::ShortcutAction::None => {}
                    shortcuts::ShortcutAction::OpenPalette => {
                        e.prevent_default();
                        ui.palette_open.set(true);
                    }
                    shortcuts::ShortcutAction::QuickSearch => {
                        e.prevent_default();
                        navigate("/search", Default::default());
                    }
                    shortcuts::ShortcutAction::Navigate(to) => {
                        navigate(to, Default::default());
                    }
                }
            }
        ></div>
    }
}

#[component]
fn InboxListRoute() -> impl IntoView {
    view! { <pages::InboxPage /> }
}

#[component]
fn InboxRoute() -> impl IntoView {
    let params = use_params_map();
    view! {
        {move || {
            let raw = params.with(|p| p.get("id").cloned().unwrap_or_default());
            match raw.parse::<i64>() {
                Ok(id) => {
                    view! { <pages::InboxPage conversation_id=id /> }
                }
                Err(_) => {
                    view! { <pages::InboxPage /> }
                }
            }
        }}
    }
}

#[component]
fn IncidentDetailRoute() -> impl IntoView {
    let params = use_params_map();
    view! {
        {move || {
            let raw = params.with(|p| p.get("id").cloned().unwrap_or_default());
            match raw.parse::<i64>() {
                Ok(id) => {
                    view! { <pages::IncidentDetailPage incident_id=id /> }
                }
                Err(_) => {
                    view! { <pages::IncidentsPage /> }
                }
            }
        }}
    }
}

#[component]
fn CustomerProfileRoute() -> impl IntoView {
    let params = use_params_map();
    view! {
        {move || {
            let raw = params.with(|p| p.get("id").cloned().unwrap_or_default());
            let id = raw.parse::<i64>().unwrap_or(0);
            view! { <pages::CustomerProfilePage customer_id=id /> }
        }}
    }
}

#[component]
fn OrganizationDetailRoute() -> impl IntoView {
    let params = use_params_map();
    view! {
        {move || {
            let raw = params.with(|p| p.get("id").cloned().unwrap_or_default());
            let id = raw.parse::<i64>().unwrap_or(0);
            view! { <pages::OrganizationDetailPage organization_id=id /> }
        }}
    }
}

fn not_found_from_params() -> impl IntoView {
    let params = use_params_map();
    let path = move || {
        params.with(|p| {
            let any = p.get("any").cloned().unwrap_or_default();
            if any.is_empty() {
                "/".to_string()
            } else {
                format!("/{any}")
            }
        })
    };
    view! { <pages::NotFoundPage path=path /> }
}

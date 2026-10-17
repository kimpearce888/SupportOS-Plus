use leptos::prelude::*;
use leptos_router::components::{A, Outlet};

use crate::state::{Theme, UiState};

#[component]
pub fn LayoutShell() -> impl IntoView {
    let ui = use_context::<UiState>()
        .expect("LayoutShell requires the UiState context (provided by app_view)");

    let onboarding_active = move || ui.onboarding_completed.get() == Some(false);

    view! {
        <div class="spp-app-shell">
            <Show when=move || !onboarding_active() fallback=|| ()>
                <nav
                    class=move || {
                        if ui.sidebar_collapsed.get() {
                            "spp-sidebar spp-sidebar--collapsed"
                        } else {
                            "spp-sidebar"
                        }
                    }
                    aria-label="Main navigation"
                >
                    <div class="spp-sidebar__brand">
                        <span class="spp-sidebar__logo" aria-hidden="true">"S"</span>
                        <span class="spp-sidebar__brand-name">"SupportOS"</span>
                        <button
                            class="spp-button spp-button--ghost spp-button--small spp-sidebar__collapse"
                            aria-label="Toggle sidebar"
                            title="Toggle sidebar"
                            on:click=move |_| ui.toggle_sidebar()
                        >
                            <span aria-hidden="true">"◧"</span>
                        </button>
                    </div>

                    <NavItem to="/" icon="▦" label="Dashboard" />
                    <NavItem to="/inbox" icon="✉" label="Inbox" count=ui.inbox_count />
                    <NavItem
                        to="/notifications"
                        icon="🔔"
                        label="Notifications"
                        count=ui.unread_count
                    />
                    <NavItem to="/search" icon="⌕" label="Search" />

                    <p class="spp-nav__section">"Directory"</p>
                    <NavItem to="/customers" icon="👥" label="Customers" />
                    <NavItem to="/organizations" icon="⌂" label="Organizations" />

                    <p class="spp-nav__section">"Intelligence"</p>
                    <NavItem to="/ai" icon="✦" label="AI Center" />
                    <NavItem to="/issues" icon="⚠" label="Issues" />
                    <NavItem to="/incidents" icon="🔥" label="Incidents" />
                    <NavItem to="/knowledge" icon="📖" label="Knowledge" />
                    <NavItem to="/docs" icon="📑" label="Docs" />
                    <NavItem to="/custom-objects" icon="▣" label="Objects" />
                    <NavItem to="/connectors" icon="⇄" label="Connectors" />
                    <NavItem to="/graph" icon="⬡" label="Graph" />

                    <p class="spp-nav__section">"Operations"</p>
                    <NavItem to="/operations" icon="◫" label="Operations" />
                    <NavItem to="/reports" icon="📊" label="Reports" />
                    <NavItem to="/outreach" icon="📣" label="Outreach" />
                    <NavItem to="/automation" icon="⤻" label="Automation" />
                    <NavItem to="/sync-health" icon="♥" label="Sync Health" />
                    <NavItem to="/settings" icon="⚙" label="Settings" />

                    <div class="spp-sidebar__footer">
                        <button
                            class="spp-button spp-button--ghost spp-button--small spp-sidebar__footer-button"
                            aria-label="Open command palette"
                            on:click=move |_| ui.palette_open.set(true)
                        >
                            <span aria-hidden="true">"⌘"</span>
                            <span class="spp-nav__label">"Command"</span>
                            <span class="spp-kbd spp-nav__label">"⌘K"</span>
                        </button>
                        <button
                            class="spp-button spp-button--ghost spp-button--small spp-sidebar__footer-button"
                            aria-label="Toggle color theme"
                            on:click=move |_| ui.toggle_theme()
                        >
                            {move || match ui.theme.get() {
                                Theme::Light => view! {
                                    <span aria-hidden="true">"☾"</span>
                                }
                                .into_any(),
                                Theme::Dark => view! {
                                    <span aria-hidden="true">"☀"</span>
                                }
                                .into_any(),
                            }}
                            <span class="spp-nav__label">
                                {move || match ui.theme.get() {
                                    Theme::Light => "Dark theme",
                                    Theme::Dark => "Light theme",
                                }}
                            </span>
                        </button>
                    </div>
                </nav>
            </Show>

            <div class="spp-main-area">
                <main class="spp-main">
                    <Outlet />
                </main>
                <footer class="spp-footer">
                    <p>
                        "Help Scout is a trademark of Help Scout, Inc. "
                        "SupportOS++ is an independent, open-source integration and is not affiliated with or endorsed by Help Scout."
                    </p>
                </footer>
            </div>
        </div>
    }
}

#[component]
fn NavItem(
    to: &'static str,
    icon: &'static str,
    label: &'static str,
    #[prop(optional, into)] count: Option<Signal<Option<u32>>>,
) -> impl IntoView {
    view! {
        <A href=to attr:class="spp-nav__link">
            <span class="spp-nav__icon" aria-hidden="true">{icon}</span>
            <span class="spp-nav__label">{label}</span>
            {move || match count.as_ref().and_then(|c| c.get().and_then(crate::state::badge_text)) {
                Some(text) => view! {
                    <span class="spp-nav__count">{text}</span>
                }
                .into_any(),
                None => ().into_any(),
            }}
        </A>
    }
}

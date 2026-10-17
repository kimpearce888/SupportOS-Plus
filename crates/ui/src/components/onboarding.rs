use std::sync::Arc;

use leptos::*;

use crate::components::Button;

pub struct OnboardingProps {
    pub first_run_done: bool,
    pub on_accept_demo: Arc<dyn Fn() + Send + Sync>,
    pub on_dismiss: Arc<dyn Fn() + Send + Sync>,
}

impl Clone for OnboardingProps {
    fn clone(&self) -> Self {
        Self {
            first_run_done: self.first_run_done,
            on_accept_demo: Arc::clone(&self.on_accept_demo),
            on_dismiss: Arc::clone(&self.on_dismiss),
        }
    }
}

impl std::fmt::Debug for OnboardingProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OnboardingProps")
            .field("first_run_done", &self.first_run_done)
            .field("on_accept_demo", &"<closure>")
            .field("on_dismiss", &"<closure>")
            .finish()
    }
}

impl OnboardingProps {
    #[must_use]
    pub fn new<F1, F2>(first_run_done: bool, on_accept_demo: F1, on_dismiss: F2) -> Self
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        Self {
            first_run_done,
            on_accept_demo: Arc::new(on_accept_demo),
            on_dismiss: Arc::new(on_dismiss),
        }
    }
}

#[component]
pub fn OnboardingOverlay(props: OnboardingProps) -> impl IntoView {
    let visible = !props.first_run_done;
    let on_accept = StoredValue::new(Arc::clone(&props.on_accept_demo));
    let on_dismiss = StoredValue::new(Arc::clone(&props.on_dismiss));

    view! {
        <Show when=move || visible fallback=|| ()>
            <div class="spp-onboarding-overlay" role="dialog" aria_labelledby="spp-onboarding-title">
                <div class="spp-onboarding-card">
                    <h2 id="spp-onboarding-title" class="spp-onboarding-card__title">
                        "Welcome to SupportOS++"
                    </h2>
                    <p class="spp-onboarding-card__body">
                        "Try the 2-minute demo mode — no Help Scout credentials needed. "
                        "You can connect a real account later from Settings."
                    </p>
                    <div class="spp-onboarding-card__actions">
                        <Button
                            on_click=move || on_accept.with_value(|f| f())
                            style=crate::components::button::ButtonStyle::Primary
                        >
                            "Try the 2-minute demo mode"
                        </Button>
                        <Button
                            on_click=move || on_dismiss.with_value(|f| f())
                            style=crate::components::button::ButtonStyle::Ghost
                        >
                            "I'll set up later"
                        </Button>
                    </div>
                    <p class="spp-onboarding-card__legal">
                        "Help Scout is a trademark of Help Scout, Inc. "
                        "SupportOS++ is an independent, open-source integration and is not affiliated with or endorsed by Help Scout."
                    </p>
                </div>
            </div>
        </Show>
    }
}

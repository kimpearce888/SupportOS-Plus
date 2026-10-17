use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::use_navigate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    Welcome,
    HelpScout,
    LmStudio,
    Qdrant,
    FirstSync,
    Finish,
}

impl OnboardingStep {
    pub const ALL: [OnboardingStep; 6] = [
        OnboardingStep::Welcome,
        OnboardingStep::HelpScout,
        OnboardingStep::LmStudio,
        OnboardingStep::Qdrant,
        OnboardingStep::FirstSync,
        OnboardingStep::Finish,
    ];

    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::HelpScout => "helpscout",
            Self::LmStudio => "lmstudio",
            Self::Qdrant => "qdrant",
            Self::FirstSync => "first-sync",
            Self::Finish => "finish",
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::HelpScout => "Connect Help Scout",
            Self::LmStudio => "Local AI (LM Studio)",
            Self::Qdrant => "Vector store (Qdrant)",
            Self::FirstSync => "First sync",
            Self::Finish => "Finish",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    fn prev(self) -> Option<Self> {
        if self.index() == 0 {
            None
        } else {
            Some(Self::ALL[self.index() - 1])
        }
    }

    #[must_use]
    pub fn parse(step: &str) -> Option<Self> {
        Self::ALL.iter().find(|s| s.key() == step).copied()
    }
}

#[component]
pub fn OnboardingPage() -> impl IntoView {
    let first_run_done = RwSignal::new(None::<bool>);
    let step = RwSignal::new(OnboardingStep::Welcome);
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);
    let action_msg = RwSignal::new(None::<String>);
    let status = RwSignal::new(serde_json::Value::Null);
    let lm_check = RwSignal::new(None::<serde_json::Value>);
    let lm_checking = RwSignal::new(false);
    let qdrant_check = RwSignal::new(None::<serde_json::Value>);
    let qdrant_checking = RwSignal::new(false);
    let syncing = RwSignal::new(false);

    let ui = use_context::<crate::state::UiState>();
    let navigate = use_navigate();

    Effect::new(move |_| {
        let first_run_done = first_run_done;
        let step = step;
        let status = status;
        let loading = loading;
        let error_msg = error_msg;
        wasm_bindgen_futures::spawn_local(async move {
            match crate::api::get_json::<serde_json::Value>("/api/onboarding").await {
                Ok(data) => {
                    let done = data
                        .get("completed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if !done
                        && let Some(resume) = data
                            .get("step")
                            .and_then(|v| v.as_str())
                            .and_then(OnboardingStep::parse)
                    {
                        step.set(resume);
                    }
                    first_run_done.set(Some(done));
                    if let Some(ui) = ui {
                        ui.onboarding_completed.set(Some(done));
                    }
                    status.set(data);
                    loading.set(false);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    loading.set(false);
                }
            }
        });
    });

    let go_to = move |next: OnboardingStep| {
        step.set(next);
        let key = next.key().to_string();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = crate::api::post_json::<serde_json::Value>(
                "/api/onboarding/step",
                Some(&serde_json::json!({ "step": key })),
            )
            .await;
        });
    };

    let check_lm = move |_| {
        let lm_check = lm_check;
        let lm_checking = lm_checking;
        wasm_bindgen_futures::spawn_local(async move {
            lm_checking.set(true);
            let result =
                crate::api::post_json::<serde_json::Value>("/api/settings/lmstudio/test", None)
                    .await;
            match result {
                Ok(v) => lm_check.set(Some(v)),
                Err(e) => lm_check.set(Some(
                    serde_json::json!({ "ok": false, "connected": false, "message": e }),
                )),
            }
            lm_checking.set(false);
        });
    };

    let check_qdrant = move |_| {
        let qdrant_check = qdrant_check;
        let qdrant_checking = qdrant_checking;
        wasm_bindgen_futures::spawn_local(async move {
            qdrant_checking.set(true);
            let result =
                crate::api::post_json::<serde_json::Value>("/api/settings/qdrant/test", None).await;
            match result {
                Ok(v) => qdrant_check.set(Some(v)),
                Err(e) => qdrant_check.set(Some(
                    serde_json::json!({ "ok": false, "connected": false, "message": e }),
                )),
            }
            qdrant_checking.set(false);
        });
    };

    let run_first_sync = move |_| {
        let syncing = syncing;
        let action_msg = action_msg;
        let status = status;
        wasm_bindgen_futures::spawn_local(async move {
            syncing.set(true);
            let result = crate::api::post_json::<serde_json::Value>(
                "/api/sync/initial",
                Some(&serde_json::json!({ "wait": true })),
            )
            .await;
            match result {
                Ok(body) => {
                    let message = body
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("First sync completed.")
                        .to_string();
                    action_msg.set(Some(message));
                }
                Err(e) => action_msg.set(Some(e)),
            }
            if let Ok(data) = crate::api::get_json::<serde_json::Value>("/api/onboarding").await {
                status.set(data);
            }
            syncing.set(false);
        });
    };

    let mark_done = move |demo: bool| {
        let action_msg = action_msg;
        let error_msg = error_msg;
        let first_run_done = first_run_done;
        wasm_bindgen_futures::spawn_local(async move {
            let result = if demo {
                crate::api::post_json::<serde_json::Value>("/api/demo/enable", None).await
            } else {
                crate::api::post_json::<serde_json::Value>("/api/onboarding/complete", None).await
            };
            match result {
                Ok(_) => {
                    first_run_done.set(Some(true));
                    if let Some(ui) = ui {
                        ui.onboarding_completed.set(Some(true));
                    }
                    if demo {
                        action_msg.set(Some(
                            "Demo mode enabled. First-run marked done.".to_string(),
                        ));
                    } else {
                        action_msg.set(Some(
                            "First-run marked done. You can set up later.".to_string(),
                        ));
                    }
                    error_msg.set(None);
                }
                Err(e) => {
                    error_msg.set(Some(e));
                    action_msg.set(None);
                }
            }
        });
    };

    Effect::new(move |prev: Option<Option<bool>>| {
        let done = first_run_done.get();
        if prev.is_some() && done == Some(true) {
            navigate("/", NavigateOptions::default());
        }
        done
    });

    view! {
        <div class="spp-page spp-page--onboarding">
            <h2 class="spp-page__title">"Onboarding"</h2>

            <p class="spp-page__intro">
                "Welcome to SupportOS++. This wizard walks you through connecting your data — every step is optional and re-runnable from Settings."
            </p>

            <Show when=move || loading.get() fallback=|| ()>
                <p>"Loading…"</p>
            </Show>

            <Show when=move || error_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--error">
                    <span class="spp-state__icon" aria-hidden="true">"⚠"</span>
                    <p class="spp-state__body">{move || error_msg.get().unwrap_or_default()}</p>
                </div>
            </Show>

            <Show when=move || action_msg.get().is_some() fallback=|| ()>
                <div class="spp-state spp-state--success">
                    {move || action_msg.get().unwrap_or_default()}
                </div>
            </Show>

            <Show
                when=move || !loading.get() && error_msg.get().is_none()
                fallback=|| ()
            >
                {move || {
                    let done = first_run_done.get().unwrap_or(false);
                    if done {
                        view! {
                            <div class="spp-onboarding__done">
                                <h3>"✅ Setup complete"</h3>
                                <p>"You've already completed the first-run onboarding."</p>
                                <p>"To connect Help Scout, go to Settings → Help Scout credentials."</p>
                                <p>"To configure AI, go to the AI Center."</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="spp-onboarding-wizard">
                                <ol class="spp-onboarding-steps" aria-label="Onboarding progress">
                                    {OnboardingStep::ALL.iter().enumerate().map(|(i, s)| {
                                        let current = step.get();
                                        let class = if *s == current {
                                            "spp-onboarding-steps__step spp-onboarding-steps__step--current"
                                        } else if i < current.index() {
                                            "spp-onboarding-steps__step spp-onboarding-steps__step--done"
                                        } else {
                                            "spp-onboarding-steps__step"
                                        };
                                        view! {
                                            <li class=class>
                                                <span class="spp-onboarding-steps__badge">{(i + 1).to_string()}</span>
                                                {s.title()}
                                            </li>
                                        }
                                    }).collect_view()}
                                </ol>

                                <div class="spp-onboarding-panel">
                                    <Show
                                        when=move || step.get() == OnboardingStep::Welcome
                                        fallback=|| ().into_any()
                                    >
                                        <div class="spp-onboarding__choices">
                                            <div class="spp-onboarding__choice">
                                                <h3>"Guided setup (recommended)"</h3>
                                                <p>"Walk through Help Scout, your local AI (LM Studio) and the vector store with live connection checks, then run your first sync."</p>
                                                <button class="spp-button" on:click=move |_| go_to(OnboardingStep::HelpScout)>
                                                    "Start guided setup"
                                                </button>
                                            </div>
                                            <div class="spp-onboarding__choice">
                                                <h3>"Try the 2-minute demo mode"</h3>
                                                <p>"No credentials needed. The app loads sample data so you can explore the UI immediately."</p>
                                                <button class="spp-button" on:click=move |_| mark_done(true)>
                                                    "Start demo mode"
                                                </button>
                                            </div>
                                            <div class="spp-onboarding__choice">
                                                <h3>"I'll set up later"</h3>
                                                <p>"Mark the first-run as done. You can configure Help Scout credentials and AI providers at any time from Settings."</p>
                                                <button class="spp-button spp-button--ghost" on:click=move |_| mark_done(false)>
                                                    "Skip for now"
                                                </button>
                                            </div>
                                        </div>
                                        <div class="spp-onboarding__help">
                                            <h3>"What you'll need"</h3>
                                            <ul>
                                                <li>"Help Scout OAuth client ID + secret (for real data sync)"</li>
                                                <li>"A local AI provider: LM Studio (127.0.0.1:1234) or Ollama (127.0.0.1:11434) — both optional"</li>
                                                <li>"No cloud accounts, no telemetry, no data egress — everything stays local."</li>
                                            </ul>
                                        </div>
                                    </Show>

                                    <Show
                                        when=move || step.get() == OnboardingStep::HelpScout
                                        fallback=|| ().into_any()
                                    >
                                        <h3>"Step 2 — Connect Help Scout"</h3>
                                        <p>
                                            "SupportOS++ reads your support mailbox through the Help Scout API. "
                                            "Credentials are configured in Settings → Help Scout credentials (OAuth client ID + secret); "
                                            "this check reads the connection state live."
                                        </p>
                                        <div class="spp-onboarding-check">
                                            {move || {
                                                let s = status.get();
                                                let configured = s.get("hs_configured").and_then(|v| v.as_bool()).unwrap_or(false);
                                                let conversations = s.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                                                view! {
                                                    <span class=format!("spp-badge {}", if configured { "spp-badge--ok" } else { "spp-badge--warn" })>
                                                        {if configured { "Connected".to_string() } else { "Not connected".to_string() }}
                                                    </span>
                                                    <span class="spp-muted spp-text-xs">
                                                        {format!("{conversations} conversation(s) already mirrored.")}
                                                    </span>
                                                }
                                            }}
                                        </div>
                                        <p class="spp-muted spp-text-xs">
                                            "Demo mode works without Help Scout — the wizard continues either way."
                                        </p>
                                    </Show>

                                    <Show
                                        when=move || step.get() == OnboardingStep::LmStudio
                                        fallback=|| ().into_any()
                                    >
                                        <h3>"Step 3 — Local AI (LM Studio)"</h3>
                                        <p>
                                            "Drafts, summaries and the analysis pipeline run on a local OpenAI-compatible endpoint "
                                            "(LM Studio by default, at 127.0.0.1:1234). This is optional — everything else works without it."
                                        </p>
                                        <button class="spp-button" on:click=check_lm>
                                            {move || if lm_checking.get() { "Checking…" } else { "Run live check" }.to_string()}
                                        </button>
                                        <Show when=move || lm_check.get().is_some() fallback=|| ().into_any()>
                                            <div class="spp-onboarding-check spp-onboarding-check--detail">
                                                {move || {
                                                    let v = lm_check.get().unwrap_or_default();
                                                    let connected = v.get("connected").and_then(|x| x.as_bool()).unwrap_or(false);
                                                    let message = v.get("message").and_then(|x| x.as_str()).unwrap_or_default().to_string();
                                                    let models: Vec<String> = v.get("models")
                                                        .and_then(|x| x.as_array())
                                                        .map(|a| a.iter().filter_map(|m| m.as_str().map(str::to_string)).collect())
                                                        .unwrap_or_default();
                                                    let has_models = !models.is_empty();
                                                    let models_display = if models.is_empty() {
                                                        String::new()
                                                    } else {
                                                        format!("Models: {}", models.join(", "))
                                                    };
                                                    let badge = if connected { "spp-badge spp-badge--ok" } else { "spp-badge spp-badge--err" }.to_string();
                                                    let state_label = if connected { "Reachable" } else { "Unreachable" }.to_string();
                                                    view! {
                                                        <span class=badge.clone()>
                                                            {state_label.clone()}
                                                        </span>
                                                        <p class="spp-onboarding-check__message">{message.clone()}</p>
                                                        <Show when=move || has_models fallback=|| ().into_any()>
                                                            <p class="spp-muted spp-text-xs">
                                                                {models_display.clone()}
                                                            </p>
                                                        </Show>
                                                    }
                                                }}
                                            </div>
                                        </Show>
                                    </Show>

                                    <Show
                                        when=move || step.get() == OnboardingStep::Qdrant
                                        fallback=|| ().into_any()
                                    >
                                        <h3>"Step 4 — Vector store (Qdrant)"</h3>
                                        <p>
                                            "Semantic search stores its vectors in Qdrant. When it is not reachable, "
                                            "keyword search (FTS) remains fully functional — this is an optional enhancement."
                                        </p>
                                        <button class="spp-button" on:click=check_qdrant>
                                            {move || if qdrant_checking.get() { "Checking…" } else { "Run live check" }.to_string()}
                                        </button>
                                        <Show when=move || qdrant_check.get().is_some() fallback=|| ().into_any()>
                                            <div class="spp-onboarding-check spp-onboarding-check--detail">
                                                {move || {
                                                    let v = qdrant_check.get().unwrap_or_default();
                                                    let connected = v.get("connected").and_then(|x| x.as_bool()).unwrap_or(false);
                                                    let message = v.get("message").and_then(|x| x.as_str()).unwrap_or_default().to_string();
                                                    view! {
                                                        <span class=format!("spp-badge {}", if connected { "spp-badge--ok" } else { "spp-badge--err" })>
                                                            {if connected { "Reachable".to_string() } else { "Not reachable".to_string() }}
                                                        </span>
                                                        <p class="spp-onboarding-check__message">{message}</p>
                                                    }
                                                }}
                                            </div>
                                        </Show>
                                    </Show>

                                    <Show
                                        when=move || step.get() == OnboardingStep::FirstSync
                                        fallback=|| ().into_any()
                                    >
                                        <h3>"Step 5 — First sync"</h3>
                                        <p>
                                            "The initial sync pulls your Help Scout mailbox into the local mirror. "
                                            "In demo mode this loads the seeded sample data instead."
                                        </p>
                                        <button class="spp-button" on:click=run_first_sync disabled=move || syncing.get()>
                                            {move || if syncing.get() { "Syncing…" } else { "Run first sync" }.to_string()}
                                        </button>
                                        {move || {
                                            let s = status.get();
                                            let conversations = s.get("conversations").and_then(|v| v.as_i64()).unwrap_or(0);
                                            let sync_state = s.get("sync_state").and_then(|v| v.as_str()).unwrap_or("new").to_string();
                                            view! {
                                                <p class="spp-muted spp-text-xs">
                                                    {format!("Sync state: {sync_state} — {conversations} conversation(s) mirrored.")}
                                                </p>
                                            }
                                        }}
                                    </Show>

                                    <Show
                                        when=move || step.get() == OnboardingStep::Finish
                                        fallback=|| ().into_any()
                                    >
                                        <h3>"Step 6 — Finish"</h3>
                                        <p>
                                            "That's it. Everything can be reconfigured later from Settings and the AI Center. "
                                            "Mark the first run done to enter the app."
                                        </p>
                                        <div class="spp-onboarding__choices">
                                            <div class="spp-onboarding__choice">
                                                <h3>"Enter the app"</h3>
                                                <p>"Marks the first run done and opens the dashboard."</p>
                                                <button class="spp-button" on:click=move |_| mark_done(false)>
                                                    "Finish setup"
                                                </button>
                                            </div>
                                            <div class="spp-onboarding__choice">
                                                <h3>"Use demo data instead"</h3>
                                                <p>"Loads the sample mailbox for exploring without credentials."</p>
                                                <button class="spp-button spp-button--ghost" on:click=move |_| mark_done(true)>
                                                    "Enable demo mode"
                                                </button>
                                            </div>
                                        </div>
                                    </Show>
                                </div>

                                <div class="spp-onboarding-nav">
                                    <Show
                                        when=move || step.get().prev().is_some()
                                        fallback=|| ().into_any()
                                    >
                                        <button
                                            class="spp-button spp-button--ghost"
                                            on:click=move |_| {
                                                if let Some(p) = step.get().prev() {
                                                    go_to(p);
                                                }
                                            }
                                        >
                                            "Back"
                                        </button>
                                    </Show>
                                    <Show
                                        when=move || step.get().next().is_some()
                                        fallback=|| ().into_any()
                                    >
                                        <button
                                            class="spp-button spp-button--primary"
                                            on:click=move |_| {
                                                if let Some(n) = step.get().next() {
                                                    go_to(n);
                                                }
                                            }
                                        >
                                            "Next"
                                        </button>
                                    </Show>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
            </Show>
        </div>
    }
}

//! P4 (D21): the webhook relay guard.
//!
//! Help Scout's delivery model has a silent failure mode: a webhook whose
//! events keep failing (ten discards, e.g. after a relay outage) is
//! **automatically deactivated** — and the platform never tells the app.
//! The health check closes that hole: it lists the account's hooks,
//! verifies ours is registered, enabled and subscribed to every event the
//! app processes, and re-registers it (with the stored secret, capped at
//! the platform's 40 characters) when the platform dropped, disabled or
//! pruned it — then removes the stale twin.
//!
//! It runs every ~6 hours (piggybacked on the maintenance tick) and on
//! demand (Settings → "check now", through the `webhook_health_check`
//! job), and its outcome surfaces in Settings and Sync Health. The
//! user-run tunnel guide lives next to it in Settings: the listener stays
//! loopback-only by design, cloudflared/ngrok/ssh relays bridge the NAT,
//! and a POST to the webhook endpoint must answer 401 to prove the chain.
//! One honest limit: the API never reveals a hook's stored secret, so a
//! rotated local secret needs a re-register.

use rusqlite::Connection;

use crate::error::Result;
use crate::helpscout::{HelpScoutProvider, HsWebhookConfig};

/// Every event the app actually processes (the match arms of
/// `webhook_handler::process_event`) — the health check re-registers a
/// hook that is subscribed to anything less.
pub const REQUIRED_EVENTS: &[&str] = &[
    "convo.created",
    "convo.updated",
    "convo.assigned",
    "convo.status",
    "convo.tags",
    "convo.custom-fields",
    "convo.moved",
    "convo.customer.reply.created",
    "convo.agent.reply.created",
    "convo.note.created",
    "convo.ai-answers.created",
    "convo.merged",
    "convo.deleted",
    "customer.created",
    "customer.updated",
    "customer.deleted",
    "organization.created",
    "organization.updated",
    "organization.deleted",
    "satisfaction.ratings",
    "tag.created",
    "tag.updated",
    "tag.deleted",
    "user.status.changed",
    "beacon.chat.created",
    "beacon.chat.customer.replied",
    "beacon.chat.agent.replied",
    "beacon.chat.assigned",
    "beacon.chat.ended",
];

/// The label the registered hook carries on the platform.
const HOOK_LABEL: &str = "SupportOS-Plus";

/// `application_settings` keys backing the cadence and the last outcome.
const KEY_LAST_RUN: &str = "webhook_health_last_run";
const KEY_OUTCOME: &str = "webhook_health_last_outcome";

/// The ~6-hour cadence (it rides the maintenance tick, which itself fires
/// every 6 hours — the two agree on purpose).
pub const CADENCE_SECS: i64 = 6 * 3600;

/// The verdict of one health check, as Settings and Sync Health render
/// it. `verdict` is one of `healthy`, `reregistered`, `failed`,
/// `not_configured`; `detail` is the human sentence.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WebhookHealthOutcome {
    pub checked_at: String,
    pub verdict: String,
    pub detail: String,
    #[serde(default)]
    pub created_hook_id: Option<i64>,
    #[serde(default)]
    pub removed_hook_ids: Vec<i64>,
}

impl WebhookHealthOutcome {
    /// A fresh outcome stamped now.
    fn new(verdict: &str, detail: String) -> Self {
        Self {
            checked_at: chrono::Utc::now().to_rfc3339(),
            verdict: verdict.to_string(),
            detail,
            created_hook_id: None,
            removed_hook_ids: Vec::new(),
        }
    }

    /// Whether the last check found the hook in good standing.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.verdict == "healthy"
    }

    /// Whether the last check healed a hook the platform had dropped,
    /// disabled or pruned.
    #[must_use]
    pub fn is_healed(&self) -> bool {
        self.verdict == "reregistered"
    }

    /// One-line summary for the status surfaces.
    #[must_use]
    pub fn summary(&self) -> String {
        format!("{} — {}", self.verdict, self.detail)
    }
}

/// The honest verdict when there is nothing to guard: no connection, or
/// no relay URL configured (the listener stays loopback-only until the
/// user runs a tunnel).
#[must_use]
pub fn not_configured_outcome(detail: &str) -> WebhookHealthOutcome {
    WebhookHealthOutcome::new("not_configured", detail.to_string())
}

/// A failed check (provider error) — recorded so the surfaces show the
/// real reason instead of a stale healthy verdict from a previous run.
#[must_use]
pub fn failed_outcome(detail: &str) -> WebhookHealthOutcome {
    WebhookHealthOutcome::new("failed", detail.to_string())
}

/// Store the outcome and stamp the cadence clock.
///
/// # Errors
/// Propagates the sqlite write failure.
pub fn store_outcome(conn: &Connection, outcome: &WebhookHealthOutcome) -> Result<()> {
    crate::settings::set_json(conn, KEY_OUTCOME, outcome)?;
    crate::settings::set_i64(conn, KEY_LAST_RUN, chrono::Utc::now().timestamp())?;
    Ok(())
}

/// The last stored outcome, if a check ever ran.
///
/// # Errors
/// Propagates the sqlite read failure (a missing row is `Ok(None)`).
pub fn load_outcome(conn: &Connection) -> Result<Option<WebhookHealthOutcome>> {
    crate::settings::get_json(conn, KEY_OUTCOME)
}

/// Whether the ~6-hour cadence is due (the maintenance tick asks before
/// enqueueing the job; the job itself stamps the clock when it runs, so
/// a dequeued-but-not-yet-run check does not silently reset the timer).
///
/// # Errors
/// Propagates the sqlite read failure.
pub fn cadence_due(conn: &Connection) -> Result<bool> {
    let last = crate::settings::get_i64(conn, KEY_LAST_RUN, 0)?;
    Ok(chrono::Utc::now().timestamp() - last >= CADENCE_SECS)
}

/// Cap the stored secret at the platform's 40 characters (registration
/// answers 400 for anything longer — the platform's own rule). Audit
/// item 32: the same cap now also applies at signature verification
/// ([`crate::webhook::effective_secret`] is the one canonical
/// definition), so a secret longer than the cap verifies against the
/// same truncation the platform holds instead of failing forever.
#[must_use]
fn capped_secret(secret: &str) -> String {
    crate::webhook::effective_secret(secret)
}

/// The pure classification of the account's hooks against ours:
/// healthy, or the reason a re-register is due plus the stale ids to
/// remove afterwards.
#[derive(Debug, Clone, PartialEq)]
enum Classification {
    Healthy { hook_id: i64, extra_twins: usize },
    ReRegister { reason: String, stale_ids: Vec<i64> },
}

#[must_use]
fn classify(our_url: &str, hooks: &[HsWebhookConfig]) -> Classification {
    let ours: Vec<&HsWebhookConfig> = hooks.iter().filter(|h| h.url == our_url).collect();
    if ours.is_empty() {
        return Classification::ReRegister {
            reason: "the platform no longer lists our webhook (dropped or never registered)"
                .to_string(),
            stale_ids: Vec::new(),
        };
    }
    let enabled: Vec<&HsWebhookConfig> = ours
        .iter()
        .copied()
        .filter(|h| h.status != "disabled" && !h.status.is_empty())
        .collect();
    let Some(primary) = enabled.first() else {
        return Classification::ReRegister {
            reason: "our webhook is listed but deactivated (ten discarded events \
                     will do that silently)"
                .to_string(),
            stale_ids: ours.iter().map(|h| h.remote_id).collect(),
        };
    };
    let missing: Vec<&str> = REQUIRED_EVENTS
        .iter()
        .copied()
        .filter(|ev| !primary.events.iter().any(|e| e == ev))
        .collect();
    if !missing.is_empty() {
        return Classification::ReRegister {
            reason: format!(
                "our webhook is pruned to a smaller event set (missing {})",
                missing.join(", ")
            ),
            stale_ids: ours.iter().map(|h| h.remote_id).collect(),
        };
    }
    Classification::Healthy {
        hook_id: primary.remote_id,
        extra_twins: ours.len().saturating_sub(1),
    }
}

/// Run the check against the live provider: list the account's hooks,
/// classify, re-register when the platform dropped, disabled or pruned
/// ours, then remove the stale twin(s). Never invents a hook state — a
/// provider error propagates and the caller records a `failed` outcome.
///
/// # Errors
/// Propagates provider failures (the caller stores a failed outcome).
pub async fn run_check(
    provider: &dyn HelpScoutProvider,
    our_url: &str,
    secret: &str,
) -> Result<WebhookHealthOutcome> {
    let hooks = provider.list_webhooks().await?;
    match classify(our_url, &hooks) {
        Classification::Healthy {
            hook_id,
            extra_twins,
        } => {
            let mut detail = format!(
                "hook #{hook_id} is registered, enabled and subscribed to all {} events we process",
                REQUIRED_EVENTS.len()
            );
            if extra_twins > 0 {
                detail.push_str(&format!(
                    " (note: {extra_twins} older hook(s) share this URL)"
                ));
            }
            Ok(WebhookHealthOutcome::new("healthy", detail))
        }
        Classification::ReRegister { reason, stale_ids } => {
            let events: Vec<String> = REQUIRED_EVENTS.iter().map(|e| (*e).to_string()).collect();
            let created_id = provider
                .create_webhook(our_url, &events, &capped_secret(secret), HOOK_LABEL)
                .await?;
            let mut removed: Vec<i64> = Vec::new();
            for id in &stale_ids {
                if provider.delete_webhook(*id).await.unwrap_or(false) {
                    removed.push(*id);
                }
            }
            let mut detail = format!(
                "{reason} — re-registered as hook #{created_id} ({} events)",
                REQUIRED_EVENTS.len()
            );
            if !removed.is_empty() {
                detail.push_str(&format!(
                    "; removed stale twin(s) {}",
                    removed
                        .iter()
                        .map(|id| format!("#{id}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Ok(WebhookHealthOutcome {
                created_hook_id: Some(created_id),
                removed_hook_ids: removed,
                ..WebhookHealthOutcome::new("reregistered", detail)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("in-memory connection");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn hook(id: i64, url: &str, status: &str, events: &[&str]) -> HsWebhookConfig {
        HsWebhookConfig {
            remote_id: id,
            url: url.to_string(),
            events: events.iter().map(|e| e.to_string()).collect(),
            status: status.to_string(),
        }
    }

    fn all_events() -> Vec<&'static str> {
        REQUIRED_EVENTS.to_vec()
    }

    #[test]
    fn healthy_when_registered_enabled_and_complete() {
        let c = classify(
            "https://relay.example.com/hook",
            &[hook(
                5,
                "https://relay.example.com/hook",
                "enabled",
                &all_events(),
            )],
        );
        assert!(matches!(c, Classification::Healthy { hook_id: 5, .. }));
    }

    #[test]
    fn classify_flags_dropped_disabled_and_pruned_hooks() {
        let url = "https://relay.example.com/hook";

        // Dropped: the platform no longer lists ours.
        let dropped = classify(
            url,
            &[hook(9, "https://other.example.com/x", "enabled", &[])],
        );
        assert!(
            matches!(dropped, Classification::ReRegister { ref reason, ref stale_ids }
            if reason.contains("no longer lists") && stale_ids.is_empty())
        );

        // Disabled: ten discarded events deactivate silently.
        let disabled = classify(url, &[hook(5, url, "disabled", &all_events())]);
        assert!(
            matches!(disabled, Classification::ReRegister { ref stale_ids, .. }
            if *stale_ids == vec![5])
        );

        // Pruned: subscribed to fewer events than the app processes.
        let pruned = classify(url, &[hook(5, url, "enabled", &["convo.created"])]);
        assert!(
            matches!(pruned, Classification::ReRegister { ref reason, ref stale_ids }
            if reason.contains("pruned") && *stale_ids == vec![5])
        );
    }

    #[test]
    fn store_load_and_cadence_round_trip() {
        let conn = fresh_conn();
        assert!(load_outcome(&conn).expect("load").is_none());
        assert!(cadence_due(&conn).expect("cadence"), "never ran -> due");

        let outcome = not_configured_outcome("no relay URL configured");
        store_outcome(&conn, &outcome).expect("store");
        let back = load_outcome(&conn).expect("load").expect("stored");
        assert_eq!(back, outcome);
        assert!(!cadence_due(&conn).expect("cadence"), "just ran -> not due");
    }

    /// A recording provider: enough of the trait to drive `run_check`.
    struct FakeHooks {
        hooks: std::sync::Mutex<Vec<HsWebhookConfig>>,
        created: std::sync::Mutex<Vec<(String, String)>>,
        deleted: std::sync::Mutex<Vec<i64>>,
    }

    #[async_trait::async_trait]
    impl HelpScoutProvider for FakeHooks {
        fn kind(&self) -> &'static str {
            "fake"
        }

        async fn get_me(&self) -> Result<crate::helpscout::HsUser> {
            Err(crate::error::Error::Other("not needed".into()))
        }

        async fn list_mailboxes(&self) -> Result<Vec<crate::helpscout::HsMailbox>> {
            Ok(Vec::new())
        }

        async fn list_users(&self) -> Result<Vec<crate::helpscout::HsUser>> {
            Ok(Vec::new())
        }

        async fn list_teams(&self) -> Result<Vec<crate::helpscout::HsTeam>> {
            Ok(Vec::new())
        }

        async fn list_tags(&self) -> Result<Vec<crate::helpscout::HsTag>> {
            Ok(Vec::new())
        }

        async fn list_conversations(
            &self,
            _query: &crate::helpscout::ConversationQuery,
        ) -> Result<crate::helpscout::Page<crate::helpscout::HsConversation>> {
            Ok(crate::helpscout::Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }

        async fn list_customers(
            &self,
            _query: &crate::helpscout::CustomerQuery,
        ) -> Result<crate::helpscout::Page<crate::helpscout::HsCustomer>> {
            Ok(crate::helpscout::Page {
                items: Vec::new(),
                next_cursor: None,
            })
        }

        async fn list_beacon_chats(&self) -> Result<Vec<crate::helpscout::HsBeaconChat>> {
            Ok(Vec::new())
        }

        async fn list_docs(&self) -> Result<Vec<crate::helpscout::HsDocArticle>> {
            Ok(Vec::new())
        }

        async fn list_ratings(&self) -> Result<Vec<crate::helpscout::HsRating>> {
            Ok(Vec::new())
        }

        async fn create_reply_thread(
            &self,
            _input: crate::helpscout::CreateThreadInput,
        ) -> Result<crate::helpscout::ThreadCreated> {
            Err(crate::error::Error::Other("not needed".into()))
        }

        async fn create_note_thread(
            &self,
            _input: crate::helpscout::CreateThreadInput,
        ) -> Result<crate::helpscout::ThreadCreated> {
            Err(crate::error::Error::Other("not needed".into()))
        }

        async fn update_conversation(
            &self,
            _conversation_id: i64,
            _patch: crate::helpscout::ConversationPatch,
        ) -> Result<bool> {
            Ok(false)
        }

        async fn list_webhooks(&self) -> Result<Vec<HsWebhookConfig>> {
            Ok(self.hooks.lock().unwrap_or_else(|p| p.into_inner()).clone())
        }

        async fn create_webhook(
            &self,
            url: &str,
            events: &[String],
            secret: &str,
            _label: &str,
        ) -> Result<i64> {
            self.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((url.to_string(), secret.to_string()));
            let event_strs: Vec<&str> = events.iter().map(String::as_str).collect();
            self.hooks
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(hook(99, url, "enabled", &event_strs));
            Ok(99)
        }

        async fn delete_webhook(&self, remote_id: i64) -> Result<bool> {
            self.deleted
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(remote_id);
            Ok(true)
        }
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    #[test]
    fn run_check_heals_a_dropped_hook_and_caps_the_secret() {
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(vec![hook(
                5,
                "https://relay.example.com/hook",
                "disabled",
                &all_events(),
            )]),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
        };
        let outcome = rt()
            .block_on(async {
                run_check(
                    &fake,
                    "https://relay.example.com/hook",
                    "a-secret-longer-than-the-platforms-forty-character-cap!!!!!!!!",
                )
                .await
            })
            .expect("run_check");

        assert!(outcome.is_healed(), "{:?}", outcome.verdict);
        assert_eq!(outcome.created_hook_id, Some(99));
        assert_eq!(outcome.removed_hook_ids, vec![5]);

        // The platform caps webhook secrets at 40 characters — the
        // re-registration must respect it or answer 400.
        let created = fake.created.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].1.chars().count(), 40, "secret must be capped");
        assert_eq!(created[0].1, "a-secret-longer-than-the-platforms-forty");
    }

    #[test]
    fn run_check_leaves_a_healthy_hook_alone() {
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(vec![hook(
                5,
                "https://relay.example.com/hook",
                "enabled",
                &all_events(),
            )]),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
        };
        let outcome = rt()
            .block_on(async { run_check(&fake, "https://relay.example.com/hook", "secret").await })
            .expect("run_check");
        assert!(outcome.is_healthy());
        assert!(
            fake.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty()
        );
        assert!(
            fake.deleted
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty()
        );
    }

    #[test]
    fn a_healthy_hook_with_an_older_twin_is_reported_not_healed() {
        // The platform keeps the old deactivated registration around
        // (ten discards deactivates; it does not delete). Ours is the
        // healthy one — nothing to heal, but the twin is said out loud.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(vec![
                hook(
                    5,
                    "https://relay.example.com/hook",
                    "enabled",
                    &all_events(),
                ),
                hook(
                    4,
                    "https://relay.example.com/hook",
                    "disabled",
                    &all_events(),
                ),
            ]),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
        };
        let outcome = rt()
            .block_on(async { run_check(&fake, "https://relay.example.com/hook", "secret").await })
            .expect("run_check");
        assert!(outcome.is_healthy(), "the enabled hook is ours");
        assert!(outcome.detail.contains("older hook"), "{}", outcome.detail);
        assert!(
            fake.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty()
        );
        assert!(
            fake.deleted
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty()
        );
    }
}

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

/// Events registered on the platform hook. Help Scout validates the whole
/// array on registration — a single retired name rejects the POST with a
/// 400 that lists every value the platform still accepts — and the enum
/// has churned under this app twice now:
///
/// - the earlier audit (M7) removed six names the catalogue did not list
///   back then; `convo.tags` and `convo.custom-fields` are documented
///   valid again today, so they are back in;
/// - 2026-10-27, live account: `convo.updated`, `beacon.chat.agent.replied`,
///   `beacon.chat.assigned` and `beacon.chat.ended` were retired — the
///   registration 400'd ("Expected array of values from this list:
///   'beacon.chat.created', …") and health was stuck "failed".
///
/// The list below matches the platform's 2026-10 catalogue, and
/// [`run_check`] self-heals on top of it: if the platform ever rejects
/// the array again, the accepted values are parsed straight out of the
/// 400 body and the registration retries with the intersection — no
/// release needed. The handler keeps tolerating every name ever seen
/// should the user subscribe to one manually.
pub const REQUIRED_EVENTS: &[&str] = &[
    "convo.created",
    "convo.assigned",
    "convo.status",
    "convo.tags",
    "convo.custom-fields",
    "convo.moved",
    "convo.customer.reply.created",
    "convo.agent.reply.created",
    "convo.note.created",
    "convo.merged",
    "convo.deleted",
    "customer.created",
    "customer.updated",
    "customer.deleted",
    "organization.created",
    "organization.updated",
    "organization.deleted",
    "satisfaction.ratings",
    "beacon.chat.created",
    "beacon.chat.customer.replied",
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

/// The relay URL exactly as it will be registered on the platform.
///
/// Help Scout's url validator refuses anything it cannot deliver to with
/// a bare `path: "url" — must be a valid URL.` 400 (the live account hit
/// exactly that on 2026-10-27, after the relay field held a value the
/// platform's validator would not take). Instead of echoing that opaque
/// body, the check normalizes what it can and refuses what it cannot:
///
/// - surrounding whitespace is trimmed and a missing scheme becomes
///   https — the classic "funnel address pasted without the prefix";
/// - plain http is upgraded to https: TLS-terminating relays (Tailscale
///   Funnel, cloudflared, ngrok) redirect http on every delivery and the
///   platform disables a hook after ten discards;
/// - loopback, private, link-local and CGNAT hosts (127.0.0.1, 192.168.x,
///   10.x, 100.64/10 …) are refused outright — the platform cannot
///   deliver there and its validator answers "must be a valid URL."
///
/// The `Err` text is written to be read as-is in the outcome surfaces:
/// it names the stored value and the exact fix.
fn normalize_relay_url(raw: &str) -> std::result::Result<String, String> {
    let trimmed = raw.trim();
    let lowered = trimmed.to_ascii_lowercase();
    let with_scheme = if lowered.starts_with("http://") || lowered.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    let mut parsed = url::Url::parse(&with_scheme).map_err(|_| {
        format!(
            "the relay URL '{trimmed}' is not a valid URL — open Settings and set \
             it to the public https address of your relay, for example \
             https://your-host.ts.net/webhook"
        )
    })?;
    let host = parsed.host().ok_or_else(|| {
        "the relay URL has no host — open Settings and set it to the public \
         https address of your relay, for example https://your-host.ts.net/webhook"
            .to_string()
    })?;
    if is_unroutable_host(host) {
        return Err(format!(
            "the relay URL points at {} — Help Scout cannot deliver webhooks to a \
             loopback or private address. Run a tunnel (Tailscale Funnel, cloudflared, \
             ngrok) and set its public https URL in Settings, for example \
             https://your-host.ts.net/webhook",
            parsed.host_str().unwrap_or("a local address")
        ));
    }
    if parsed.scheme() == "http" {
        parsed
            .set_scheme("https")
            .map_err(|_| "the relay URL scheme could not be upgraded to https".to_string())?;
    }
    Ok(parsed.to_string())
}

/// Whether Help Scout's servers can never route to this host: loopback,
/// RFC1918/private, link-local, unspecified, or the CGNAT range Tailscale
/// node IPs live in (100.64/10 — reachable only from inside the tailnet).
fn is_unroutable_host(host: url::Host<&str>) -> bool {
    match host {
        url::Host::Ipv4(ip) => {
            let o = ip.octets();
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                || (o[0] == 100 && (64..=127).contains(&o[1]))
        }
        url::Host::Ipv6(ip) => ip.is_loopback() || ip.is_unspecified(),
        url::Host::Domain(d) => {
            let d = d.to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
    }
}

/// Registration failures must stay diagnosable years from now: append the
/// exact URL the request carried to both the friendly surface text and
/// the raw body, so a platform rejection like `path: "url" — must be a
/// valid URL.` can be answered by reading the outcome alone.
fn with_relay_context(e: crate::error::Error, relay: &str) -> crate::error::Error {
    let note = format!(" (the url we sent: {relay})");
    if let crate::error::Error::Other(boxed) = &e
        && let Some(api) = boxed.downcast_ref::<crate::helpscout_real::HsApiError>()
    {
        let mut api = api.clone();
        api.friendly.push_str(&note);
        api.message.push_str(&note);
        return crate::error::Error::Other(Box::new(api));
    }
    crate::error::Error::Other(format!("{e}{note}").into())
}

/// One registration attempt with the platform's own error as the escape
/// hatch: when the POST answers 400 "Expected array of values from this
/// list", the accepted values are parsed out of that body and the
/// registration retries once with the intersection. The event enum has
/// churned under this app twice; a static list must never wedge the
/// health check in "failed" until the next release again.
///
/// Returns the created hook id, the event set actually registered, and
/// the names the platform retired (empty on the happy path).
async fn register_hook(
    provider: &dyn HelpScoutProvider,
    our_url: &str,
    events: &[String],
    secret: &str,
) -> Result<(i64, Vec<String>, Vec<String>)> {
    let secret = capped_secret(secret);
    match provider
        .create_webhook(our_url, events, &secret, HOOK_LABEL)
        .await
    {
        Ok(id) => Ok((id, events.to_vec(), Vec::new())),
        Err(first) => {
            let Some(accepted) = accepted_events_from_error(&transport_message(&first)) else {
                return Err(first);
            };
            let usable: Vec<String> = events
                .iter()
                .filter(|e| accepted.iter().any(|a| a == *e))
                .cloned()
                .collect();
            if usable.is_empty() || usable.len() == events.len() {
                return Err(first);
            }
            let dropped: Vec<String> = events
                .iter()
                .filter(|e| !usable.contains(e))
                .cloned()
                .collect();
            tracing::warn!(
                dropped = %dropped.join(", "),
                "Help Scout retired some webhook events — registering the accepted intersection"
            );
            let id = provider
                .create_webhook(our_url, &usable, &secret, HOOK_LABEL)
                .await?;
            Ok((id, usable, dropped))
        }
    }
}

/// The transport layer keeps the raw response text (up to 4,000 chars) in
/// [`crate::helpscout_real::HsApiError::message`]; `Display` only carries
/// the 300-character friendly sentence, so downcast to reach the body.
fn transport_message(e: &crate::error::Error) -> String {
    if let crate::error::Error::Other(boxed) = e
        && let Some(api) = boxed.downcast_ref::<crate::helpscout_real::HsApiError>()
    {
        return api.message.clone();
    }
    e.to_string()
}

/// Pull every Help Scout event name out of a 400 body carrying the
/// platform's "Expected array of values from this list: 'a', 'b', …"
/// validation error. Single-quoted dotted lowercase tokens only —
/// anything else in the body is ignored. `None` when the marker or any
/// name is missing (the caller then keeps the original error).
fn accepted_events_from_error(text: &str) -> Option<Vec<String>> {
    const MARKER: &str = "Expected array of values from this list";
    let tail = &text[text.find(MARKER)? + MARKER.len()..];
    let mut out: Vec<String> = Vec::new();
    let mut rest = tail;
    while let Some(start) = rest.find('\'') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('\'') else {
            break;
        };
        let token = &after[..end];
        if is_event_name(token) && !out.iter().any(|e| e == token) {
            out.push(token.to_string());
        }
        rest = &after[end + 1..];
    }
    if out.is_empty() { None } else { Some(out) }
}

#[must_use]
fn is_event_name(s: &str) -> bool {
    s.len() > 3
        && s.contains('.')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

/// Run the check against the live provider: list the account's hooks,
/// classify, re-register when the platform dropped, disabled or pruned
/// ours, then remove the stale twin(s). Never invents a hook state — a
/// provider error propagates and the caller records a `failed` outcome.
/// A relay URL that cannot be sent at all (schemeless, mistyped, loopback
/// or private) is an honest `failed` outcome naming the fix — no API call
/// is made.
///
/// # Errors
/// Propagates provider failures (the caller stores a failed outcome).
pub async fn run_check(
    provider: &dyn HelpScoutProvider,
    our_url: &str,
    secret: &str,
) -> Result<WebhookHealthOutcome> {
    // A URL the platform's validator would refuse (schemeless, mistyped,
    // loopback/private) is diagnosed here — before any API call — so the
    // outcome names the fix instead of echoing a bare "must be a valid
    // URL." 400 (the live account's exact failure on 2026-10-27).
    let relay = match normalize_relay_url(our_url) {
        Ok(relay) => relay,
        Err(msg) => return Ok(failed_outcome(&msg)),
    };
    let upgrade_note = (relay != our_url).then(|| {
        " — normalized to https:// (a plain-http or schemeless hook is \
         redirected or refused on every delivery and disabled after ten discards)"
            .to_string()
    });
    let hooks = provider.list_webhooks().await?;
    // Classify against the upgraded relay: registration always uses it,
    // so an http://-saved URL must not keep hunting for a hook that will
    // never exist under that scheme (that would re-register every run).
    match classify(&relay, &hooks) {
        Classification::Healthy {
            hook_id,
            extra_twins,
        } => {
            let mut detail = format!(
                "hook #{hook_id} is registered, enabled and subscribed to all {} events we process",
                REQUIRED_EVENTS.len()
            );
            if let Some(note) = &upgrade_note {
                detail.push_str(note);
            }
            if extra_twins > 0 {
                detail.push_str(&format!(
                    " (note: {extra_twins} older hook(s) share this URL)"
                ));
            }
            Ok(WebhookHealthOutcome::new("healthy", detail))
        }
        Classification::ReRegister { reason, stale_ids } => {
            let events: Vec<String> = REQUIRED_EVENTS.iter().map(|e| (*e).to_string()).collect();
            let (created_id, registered, dropped) =
                register_hook(provider, &relay, &events, secret)
                    .await
                    .map_err(|e| with_relay_context(e, &relay))?;
            let mut removed: Vec<i64> = Vec::new();
            for id in &stale_ids {
                if provider.delete_webhook(*id).await.unwrap_or(false) {
                    removed.push(*id);
                }
            }
            let mut detail = format!(
                "{reason} — re-registered as hook #{created_id} ({} events)",
                registered.len()
            );
            if !dropped.is_empty() {
                detail.push_str(&format!(
                    "; the platform no longer accepts {} — dropped from the subscription",
                    dropped.join(", ")
                ));
            }
            if let Some(note) = &upgrade_note {
                detail.push_str(note);
            }
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
    /// `reject` lists event names the platform has "retired" — a create
    /// requesting any of them answers the real 400 rejection body.
    struct FakeHooks {
        hooks: std::sync::Mutex<Vec<HsWebhookConfig>>,
        created: std::sync::Mutex<Vec<(String, String)>>,
        deleted: std::sync::Mutex<Vec<i64>>,
        reject: Vec<String>,
    }

    /// The platform's real rejection shape: the whole array is refused and
    /// the body lists every value it still accepts.
    fn platform_rejection(keep: &[&str]) -> crate::error::Error {
        let list = keep
            .iter()
            .map(|e| format!("'{e}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let body = format!(
            "{{\"message\":\"Bad request\",\"_embedded\":{{\"errors\":[{{\"path\":\"events\",\"message\":\" Expected array of values from this list: {list}\"}}]}}}}"
        );
        crate::error::Error::Other(Box::new(crate::helpscout_real::HsApiError {
            status_code: 400,
            message: format!("Help Scout POST /v2/webhooks -> 400: {body}"),
            friendly: "Help Scout rejected the request as invalid".to_string(),
            retryable: false,
        }))
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
            if self.reject.iter().any(|r| r == "*") {
                // A 400 that is not about the events catalogue at all.
                return Err(crate::error::Error::Other(Box::new(
                    crate::helpscout_real::HsApiError {
                        status_code: 400,
                        message:
                            "Help Scout POST /v2/webhooks -> 400: {\"message\":\"Bad request\"}"
                                .to_string(),
                        friendly: "Help Scout rejected the request as invalid".to_string(),
                        retryable: false,
                    },
                )));
            }
            if events.iter().any(|e| self.reject.iter().any(|r| r == e)) {
                let keep: Vec<&str> = REQUIRED_EVENTS
                    .iter()
                    .copied()
                    .filter(|e| !self.reject.iter().any(|r| r == e))
                    .collect();
                return Err(platform_rejection(&keep));
            }
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
            reject: Vec::new(),
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
            reject: Vec::new(),
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
            reject: Vec::new(),
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

    #[test]
    fn accepted_events_parser_reads_the_real_rejection_body() {
        // The body Help Scout sent the live account on 2026-10-27 (the
        // catalogue is the platform's, verbatim).
        let body = r#"{"_links":{"about":{"href":"http://developer.helpscout.net/mailbox-api/overview/errors"}},"logRef":"71418079-cc3c-4adc-aff0-0e160463f4ad#269758","message":"Bad request","_embedded":{"errors":[{"path":"events","message":" Expected array of values from this list: 'beacon.chat.created', 'beacon.chat.customer.replied', 'convo.agent.reply.created', 'convo.ai-answers-email.reply.created', 'convo.ai-answers-email.resolution', 'convo.assigned', 'convo.created', 'convo.custom-fields', 'convo.customer.reply.created', 'convo.deleted', 'convo.merged', 'convo.moved', 'convo.note.created', 'convo.status', 'convo.tags', 'customer.created', 'customer.deleted', 'customer.updated', 'message.survey.response.received', 'organization.created', 'organization.deleted', 'organization.updated', 'satisfaction.ratings', 'tag.created', 'tag.deleted', 'tag.updated', 'user.status.changed'"}]}}"#;
        let accepted = accepted_events_from_error(body).expect("the list parses");
        assert_eq!(accepted.len(), 27, "every catalogue value, deduped");
        assert!(accepted.contains(&"beacon.chat.customer.replied".to_string()));
        assert!(!accepted.contains(&"beacon.chat.ended".to_string()));
        // No marker / no names -> None (the caller keeps the first error).
        assert!(accepted_events_from_error("{\"message\":\"Bad request\"}").is_none());
        assert!(accepted_events_from_error("").is_none());
    }

    #[test]
    fn run_check_self_heals_when_the_platform_retires_events() {
        // The app's static list has drifted ahead of the platform's
        // catalogue again (the exact failure the live account hit on
        // 2026-10-27): the first registration 400s with the accepted
        // list, the retry subscribes to the intersection, and the check
        // still lands "reregistered" — naming what was dropped.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: vec![
                "satisfaction.ratings".to_string(),
                "beacon.chat.customer.replied".to_string(),
            ],
        };
        let outcome = rt()
            .block_on(async { run_check(&fake, "https://relay.example.com/hook", "secret").await })
            .expect("run_check must self-heal instead of staying failed");

        assert!(
            outcome.is_healed(),
            "{:?} — {}",
            outcome.verdict,
            outcome.detail
        );
        assert_eq!(outcome.created_hook_id, Some(99));
        assert!(
            outcome.detail.contains("satisfaction.ratings"),
            "{}",
            outcome.detail
        );
        assert!(
            outcome.detail.contains("beacon.chat.customer.replied"),
            "{}",
            outcome.detail
        );
        let created = fake.created.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(created.len(), 1, "exactly one successful registration");
        let hooks = fake.hooks.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(hooks[0].events.len(), REQUIRED_EVENTS.len() - 2);
        assert!(
            !hooks[0]
                .events
                .iter()
                .any(|e| e == "beacon.chat.customer.replied")
        );
    }

    #[test]
    fn run_check_stays_failed_when_the_rejection_is_not_about_events() {
        // A 400 without the catalogue marker must surface, not retry.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: vec!["*".to_string()],
        };
        let err = rt()
            .block_on(async { run_check(&fake, "https://relay.example.com/hook", "secret").await })
            .expect_err("must stay failed");
        assert!(err.to_string().contains("rejected"), "{err}");
    }

    #[test]
    fn relay_urls_are_normalized_before_registration() {
        // Schemeless (the classic funnel-address-without-the-prefix paste)
        // and whitespace-padded values are cleaned up…
        assert_eq!(
            normalize_relay_url("  pop-os.tail1234.ts.net/webhook ").unwrap_or_default(),
            "https://pop-os.tail1234.ts.net/webhook"
        );
        // …plain http is upgraded (redirect-bounce protection)…
        assert_eq!(
            normalize_relay_url("http://pop-os.tail1234.ts.net/webhook").unwrap_or_default(),
            "https://pop-os.tail1234.ts.net/webhook"
        );
        assert_eq!(
            normalize_relay_url("https://relay.example.com/hook").unwrap_or_default(),
            "https://relay.example.com/hook"
        );
        // …but a URL the platform could never deliver to is refused with
        // the fix spelled out, not passed through to a bare "must be a
        // valid URL." 400.
        for bad in [
            "http://127.0.0.1:3000/webhook",
            "http://localhost:3000/webhook",
            "http://192.168.1.10:3000/webhook",
            "http://10.0.0.5/webhook",
            "http://100.101.102.103/webhook", // CGNAT — a tailscale node IP
            "not a url at all",
        ] {
            let msg = normalize_relay_url(bad).expect_err(bad);
            assert!(msg.contains("Settings"), "{bad} -> {msg}");
        }
    }

    #[test]
    fn run_check_refuses_an_undeliverable_relay_without_touching_the_api() {
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: Vec::new(),
        };
        let outcome = rt()
            .block_on(async { run_check(&fake, "http://127.0.0.1:3000/webhook", "secret").await })
            .expect("a misconfigured relay is an honest failed outcome, not an error");
        assert_eq!(outcome.verdict, "failed");
        assert!(
            outcome.detail.contains("loopback or private"),
            "{}",
            outcome.detail
        );
        assert!(
            fake.created
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty(),
            "no registration may be attempted"
        );
    }

    #[test]
    fn a_schemeless_relay_is_registered_over_https_on_the_first_try() {
        // The user pasted the funnel address without the scheme — exactly
        // the shape Help Scout answers with 400 "must be a valid URL.".
        // The check registers the normalized https form straight away.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: Vec::new(),
        };
        let outcome = rt()
            .block_on(async { run_check(&fake, "pop-os.tail1234.ts.net/webhook", "secret").await })
            .expect("run_check");
        assert!(outcome.is_healed(), "{}", outcome.detail);
        let created = fake.created.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(created[0].0, "https://pop-os.tail1234.ts.net/webhook");
    }

    #[test]
    fn a_registration_failure_names_the_url_we_sent() {
        // When the platform still rejects the registration, the error must
        // carry the exact URL, so "must be a valid URL." can be answered
        // by reading the outcome alone.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: vec!["*".to_string()],
        };
        let err = rt()
            .block_on(async { run_check(&fake, "https://relay.example.com/hook", "secret").await })
            .expect_err("must stay failed");
        let text = err.to_string();
        assert!(text.contains("rejected"), "{text}");
        assert!(text.contains("the url we sent"), "{text}");
        assert!(text.contains("https://relay.example.com/hook"), "{text}");
    }

    #[test]
    fn a_rejected_http_relay_still_classifies_and_registers_over_https() {
        // The user saved an http:// ts.net relay URL: the check must
        // classify and register under https:// so the same check is not
        // re-run (and a duplicate hook created) every six hours.
        let fake = FakeHooks {
            hooks: std::sync::Mutex::new(Vec::new()),
            created: std::sync::Mutex::new(Vec::new()),
            deleted: std::sync::Mutex::new(Vec::new()),
            reject: Vec::new(),
        };
        let outcome = rt()
            .block_on(async {
                run_check(&fake, "http://pop-os.tail1234.ts.net/webhook", "secret").await
            })
            .expect("run_check");
        assert!(outcome.is_healed());
        assert!(
            outcome.detail.contains("normalized to https://"),
            "{}",
            outcome.detail
        );
        let created = fake.created.lock().unwrap_or_else(|p| p.into_inner());
        assert_eq!(created[0].0, "https://pop-os.tail1234.ts.net/webhook");
    }
}

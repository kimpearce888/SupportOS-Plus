//! Connection Doctor (v1.4.0 R1, defect D3).
//!
//! Five sequential checks with per-step results and remedies, run from
//! Settings and after every credential change. Each step names exactly
//! what failed in plain language, so an owner never has to guess which
//! link in the chain broke: the network, the credentials, the token
//! exchange, the API identity, the mailboxes, or the rate-limit budget.
//!
//! The doctor deliberately stops at the first hard failure: when
//! Help Scout is unreachable, "get token" would only re-report the same
//! network error with a less helpful label. Every step that *does* run
//! records what it saw, so the rendered report is complete evidence of
//! the chain up to the break.

use crate::helpscout::HelpScoutProvider as _;
use crate::helpscout_real::RealHelpScoutProvider;

/// Outcome of one doctor step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorStatus {
    /// The check succeeded; `detail` carries what it saw.
    Pass,
    /// The check failed; `detail` says why, `remedy` says what to do.
    Fail,
    /// The check succeeded with a caveat worth stating.
    Warn,
}

impl DoctorStatus {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "ok",
            Self::Fail => "failed",
            Self::Warn => "warning",
        }
    }
}

/// One Connection Doctor step: what was checked, what happened, and the
/// remedy when it failed.
#[derive(Debug, Clone)]
pub struct DoctorStep {
    /// Short step name, e.g. "reach api.helpscout.net".
    pub name: String,
    pub status: DoctorStatus,
    /// What the step observed, in plain language.
    pub detail: String,
    /// What to do about a failure (empty on pass).
    pub remedy: String,
}

impl DoctorStep {
    fn pass(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Pass,
            detail,
            remedy: String::new(),
        }
    }

    fn warn(name: &str, detail: String, remedy: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Warn,
            detail,
            remedy: remedy.to_string(),
        }
    }

    fn fail(name: &str, detail: String, remedy: &str) -> Self {
        Self {
            name: name.to_string(),
            status: DoctorStatus::Fail,
            detail,
            remedy: remedy.to_string(),
        }
    }
}

/// Run the five checks against a live provider.
///
/// 1. **Reach** — an unauthenticated `GET /v2/users/me` must produce any
///    HTTP response (a 401 proves the server is reachable).
/// 2. **Token** — the client-credentials grant must succeed (the
///    platform's exact rejection message is preserved).
/// 3. **Identity** — `GET /v2/users/me` with the token must return the
///    connected user.
/// 4. **Mailboxes** — the mailbox list must be readable.
/// 5. **Rate limit** — the headers Help Scout last reported (limit and
///    remaining) are shown, with a note that they populate only after
///    the first authenticated call of this session.
pub async fn run_doctor(provider: &RealHelpScoutProvider) -> Vec<DoctorStep> {
    let mut steps: Vec<DoctorStep> = Vec::with_capacity(5);
    let creds = &provider.credentials;
    let api_base = creds.api_base.trim_end_matches('/').to_string();

    // 1. Reach the API host.
    let client = provider.http_client();
    let reach = client
        .get(format!("{api_base}/v2/users/me"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    match reach {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if status == 401 {
                steps.push(DoctorStep::pass(
                    "reach the Help Scout API",
                    format!(
                        "{api_base} answered HTTP 401 without credentials, exactly as expected."
                    ),
                ));
            } else {
                // Any HTTP response still proves reachability; anything
                // other than 401 is worth showing verbatim.
                steps.push(DoctorStep::warn(
                    "reach the Help Scout API",
                    format!("{api_base} answered HTTP {status} (reachable; 401 was expected)."),
                    "Continue and watch the later steps — an unusual response from the API host may explain them.",
                ));
            }
        }
        Err(e) => {
            steps.push(DoctorStep::fail(
                "reach the Help Scout API",
                format!("cannot reach {api_base}: {e}"),
                "Check your internet connection, any VPN or proxy, and whether api.helpscout.net is blocked on this network. Everything else waits until the API host answers.",
            ));
            return steps;
        }
    }

    // 2. Obtain a token with the client-credentials grant.
    match provider.client_credentials_login().await {
        Ok(_) => {
            steps.push(DoctorStep::pass(
                "sign in with client credentials",
                "Help Scout accepted the client id/secret and issued an access token.".to_string(),
            ));
        }
        Err(e) => {
            steps.push(DoctorStep::fail(
                "sign in with client credentials",
                e.to_string(),
                "Open your Help Scout profile → Apps & Integrations → your custom app, and check that the client id and secret match exactly (no stray spaces) and the app is enabled. The Free plan has no API access; custom apps need a paid plan.",
            ));
            return steps;
        }
    }

    // 3. Read the connected identity.
    match provider.get_me().await {
        Ok(me) => {
            let name = format!(
                "{} {}",
                me.first_name.unwrap_or_default(),
                me.last_name.unwrap_or_default()
            )
            .trim()
            .to_string();
            let who = if name.is_empty() {
                format!("user id {}", me.remote_id)
            } else {
                name
            };
            steps.push(DoctorStep::pass(
                "read the connected user",
                format!(
                    "Connected as {who} ({}).",
                    me.email.as_deref().unwrap_or("no email on file")
                ),
            ));
        }
        Err(e) => {
            steps.push(DoctorStep::fail(
                "read the connected user",
                e.to_string(),
                "The token was issued but the account rejects /v2/users/me — the user may be a Light User (no API access) or the app's scopes may be wrong. Reconnect with a full user's credentials.",
            ));
            return steps;
        }
    }

    // 4. List mailboxes.
    match provider.list_mailboxes().await {
        Ok(mailboxes) if mailboxes.is_empty() => {
            steps.push(DoctorStep::warn(
                "list mailboxes",
                "The connection works but this Help Scout account has no mailboxes visible to it.".into(),
                "A mailbox assigned only to other users or a fresh account explains this. Sync will import an empty account.",
            ));
        }
        Ok(mailboxes) => {
            steps.push(DoctorStep::pass(
                "list mailboxes",
                format!("{} mailbox(s) visible.", mailboxes.len()),
            ));
        }
        Err(e) => {
            steps.push(DoctorStep::fail(
                "list mailboxes",
                e.to_string(),
                "The account authenticated but refused the mailbox list — check the user's role and mailbox assignments in Help Scout.",
            ));
        }
    }

    // 5. Show the rate-limit picture from the responses so far.
    let snap = provider.rate_limit_snapshot();
    let limit = snap.limit_per_minute;
    match snap.remaining {
        Some(remaining) => {
            let detail = format!("{remaining} of {limit} requests/minute remaining (as last reported by Help Scout).");
            if remaining > 0 {
                steps.push(DoctorStep::pass("rate-limit budget", detail));
            } else {
                steps.push(DoctorStep::warn(
                    "rate-limit budget",
                    detail,
                    "Requests are exhausted for this minute; Help Scout answers 429 until the window resets. The limiter retries automatically.",
                ));
            }
        }
        None => steps.push(DoctorStep::warn(
            "rate-limit budget",
            format!("Help Scout has not reported rate-limit headers this session yet (plan limit: {limit}/minute assumed)."),
            "The exact remaining budget appears after the first authenticated call of a sync; run a sync and check again.",
        )),
    }

    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpscout_real::HsCredentials;
    use crate::mockserver::{MockRequest, MockResponse, MockServer};

    fn provider_for(db: &crate::db::DbHandle, base: String) -> RealHelpScoutProvider {
        RealHelpScoutProvider::new(
            db.clone(),
            HsCredentials {
                client_id: "doctor-client".into(),
                client_secret: "doctor-secret".into(),
                redirect_uri: String::new(),
                api_base: base,
                webhook_secret: String::new(),
                docs_api_key: String::new(),
                docs_api_base: String::new(),
            },
        )
    }

    fn db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-doctor-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("doctor.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    #[test]
    fn unreachable_api_stops_the_doctor_at_step_one() {
        let db = db();
        // Nothing listens on this loopback port.
        let provider = provider_for(&db, "http://127.0.0.1:1".to_string());
        let steps = runtime().block_on(run_doctor(&provider));
        assert_eq!(steps.len(), 1, "the doctor stops after a hard failure");
        assert_eq!(steps[0].status, DoctorStatus::Fail);
        assert!(
            !steps[0].remedy.is_empty(),
            "every failure carries a remedy"
        );
    }

    #[test]
    fn full_chain_passes_against_a_mock_api() {
        let db = db();
        let me = serde_json::json!({
            "id": 7,
            "firstName": "Doctor",
            "lastName": "Who",
            "email": "doctor@example.test",
            "role": "owner",
        });
        let mailboxes = serde_json::json!({
            "_embedded": {
                "mailboxes": [
                    { "id": 11, "name": "Support", "slug": "support" }
                ]
            }
        });
        let server = MockServer::start(std::sync::Arc::new(move |req: &MockRequest| {
            // Step 1 probes without credentials (expects 401); step 3
            // arrives with the Bearer token the step-2 grant stored.
            if req.method == "GET" && req.path == "/v2/users/me" {
                return if req.header("authorization").is_some() {
                    MockResponse::json(200, me.clone())
                        .with_header("X-RateLimit-Limit-minute", "200")
                        .with_header("X-RateLimit-Remaining-minute", "187")
                } else {
                    MockResponse::json(401, serde_json::json!({ "code": 401 }))
                };
            }
            match (req.method.as_str(), req.path.as_str()) {
                ("POST", "/v2/oauth2/token") => MockResponse::json(
                    200,
                    serde_json::json!({ "access_token": "doctor-token", "expires_in": 3600 }),
                ),
                ("GET", "/v2/mailboxes") => MockResponse::json(200, mailboxes.clone()),
                _ => MockResponse::json(404, serde_json::json!({"code": 404})),
            }
        }))
        .expect("mock server");
        let provider = provider_for(&db, server.url());
        let steps = runtime().block_on(run_doctor(&provider));
        assert_eq!(steps.len(), 5, "all five steps run: {steps:#?}");
        for step in &steps {
            assert_eq!(
                step.status,
                DoctorStatus::Pass,
                "step {} reported {}",
                step.name,
                step.status.label()
            );
        }
        assert!(steps[2].detail.contains("Doctor Who"));
        assert!(steps[3].detail.contains("1 mailbox"));
    }

    #[test]
    fn bad_credentials_fail_at_the_token_step_with_a_remedy() {
        let db = db();
        let server = MockServer::start(std::sync::Arc::new(|req: &MockRequest| {
            if req.path.contains("/oauth2/token") {
                MockResponse::json(
                    400,
                    serde_json::json!({
                        "code": 400,
                        "message": "invalid client credentials",
                    }),
                )
            } else {
                MockResponse::json(401, serde_json::json!({ "code": 401 }))
            }
        }))
        .expect("mock server");
        let provider = provider_for(&db, server.url());
        let steps = runtime().block_on(run_doctor(&provider));
        assert_eq!(steps.len(), 2, "reachability passed, the token step failed");
        assert_eq!(steps[0].status, DoctorStatus::Pass);
        assert_eq!(steps[1].status, DoctorStatus::Fail);
        assert!(
            !steps[1].remedy.is_empty(),
            "the token failure explains the fix"
        );
    }
}

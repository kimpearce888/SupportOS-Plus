//! A6 golden-set runner: the seeded 9-case regression harness for the
//! analysis pipeline.
//!
//! The 9 cases (`golden_test_set`, seeded at boot by `ai_pipeline` since the
//! earliest builds) exercise the real ticket-analysis prompt end to end:
//! each case is rendered through the exact same `build_ticket_analysis_user`
//! and `TICKET_ANALYSIS_SYSTEM` path production uses, and passes only when
//! the model returns a parseable analysis with at least one non-empty core
//! field. Every case records an `ai_runs` row (kind `golden_eval`, with
//! token usage and endpoint) so the cost ledger prices eval runs too, and
//! the whole run lands in `golden_test_runs` for the AI usage dashboard.
//!
//! Triggers (A6): on demand from the AI Center, on model-role reassignment
//! (the role grid enqueues a job), and via [`golden_context_fingerprint`] —
//! when the analysis prompt version or role map changes after an upgrade,
//! the maintenance pass notices the fingerprint drift and re-runs the set.

use rusqlite::{Connection, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::ai_pipeline::{
    AiBackend, LmStudioError, complete_run_with_usage, fail_run, parse_ticket_analysis, start_run,
};
use crate::ai_prompts::{
    EvidenceContext, PROMPT_VERSIONS_GOLDEN_EVAL, PROMPT_VERSIONS_TICKET_ANALYSIS,
    TICKET_ANALYSIS_SYSTEM, ThreadEntry, build_ticket_analysis_user,
};
use crate::error::Result;

struct GoldenCase {
    id: i64,
    name: String,
    category: String,
    subject: String,
    body: String,
}

fn active_cases(conn: &Connection) -> Result<Vec<GoldenCase>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, category, subject, body
           FROM golden_test_set WHERE active = 1 ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(GoldenCase {
                id: r.get(0)?,
                name: r.get(1)?,
                category: r.get(2)?,
                subject: r.get(3)?,
                body: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Execute every active golden case against the routed backend and persist
/// the run. A disabled backend still writes an honest run row — passed=0,
/// failed=N, one error per case — instead of pretending nothing happened.
pub async fn run_golden_set(
    conn: &Connection,
    backend: &AiBackend,
) -> std::result::Result<Value, LmStudioError> {
    let cases = active_cases(conn).map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let model_name = match backend {
        AiBackend::LmStudio { model, .. } => model.clone().unwrap_or_else(|| "unset".into()),
        AiBackend::Disabled => "disabled".into(),
    };
    conn.execute(
        "INSERT INTO golden_test_runs (backend, passed, failed) VALUES (?1, 0, 0)",
        params![model_name],
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    let run_row = conn.last_insert_rowid();

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut results: Vec<Value> = Vec::new();
    for case in &cases {
        let outcome = run_case(conn, backend, case).await;
        let entry = match outcome {
            Ok((latency_ms, fields)) => {
                passed += 1;
                json!({
                    "name": case.name,
                    "category": case.category,
                    "passed": true,
                    "latency_ms": latency_ms,
                    "fields": fields,
                })
            }
            Err((message, latency_ms)) => {
                failed += 1;
                json!({
                    "name": case.name,
                    "category": case.category,
                    "passed": false,
                    "latency_ms": latency_ms,
                    "error": message,
                })
            }
        };
        results.push(entry);
    }
    conn.execute(
        "UPDATE golden_test_runs
            SET finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                passed = ?2, failed = ?3, results = ?4
          WHERE id = ?1",
        params![
            run_row,
            passed as i64,
            failed as i64,
            serde_json::to_string(&results).unwrap_or_default()
        ],
    )
    .map_err(|e| LmStudioError::new(e.to_string(), true))?;
    Ok(json!({
        "run_id": run_row,
        "backend": model_name,
        "passed": passed,
        "failed": failed,
        "results": results,
    }))
}

type CaseOk = (u64, Vec<&'static str>);
type CaseErr = (String, Option<u64>);

async fn run_case(
    conn: &Connection,
    backend: &AiBackend,
    case: &GoldenCase,
) -> std::result::Result<CaseOk, CaseErr> {
    if matches!(backend, AiBackend::Disabled) {
        return Err((
            "AI backend is disabled — the golden set needs a routed Classify role.".into(),
            None,
        ));
    }
    let model = match backend {
        AiBackend::LmStudio { model, .. } => model.as_deref(),
        AiBackend::Disabled => None,
    };
    let ctx = EvidenceContext {
        conversation_number: 0,
        subject: case.subject.clone(),
        customer_name: "Golden case".into(),
        threads: vec![ThreadEntry {
            author: "Customer".into(),
            kind: "customer".into(),
            date: String::new(),
            text: case.body.clone(),
        }],
        ..EvidenceContext::default()
    };
    let run_id = start_run(
        conn,
        "golden_eval",
        None,
        model,
        PROMPT_VERSIONS_GOLDEN_EVAL,
        None,
        &json!([case.id, case.name]),
    )
    .map_err(|e| (e.to_string(), None))?;
    let res = backend
        .chat_json(
            conn,
            TICKET_ANALYSIS_SYSTEM,
            &build_ticket_analysis_user(&ctx),
            true,
            None,
        )
        .await;
    match res {
        Ok(res) => {
            let analysis = parse_ticket_analysis(res.json.as_ref());
            let mut fields: Vec<&'static str> = Vec::new();
            let mut non_empty = 0;
            if let Some(a) = &analysis {
                for (label, value) in [
                    ("summary", a.summary.as_deref()),
                    ("intent", a.intent.as_deref()),
                    ("customer_goal", a.customer_goal.as_deref()),
                ] {
                    if value.is_some_and(|v| !v.trim().is_empty()) {
                        fields.push(label);
                        non_empty += 1;
                    }
                }
            }
            if analysis.is_some() && non_empty > 0 {
                let _ = complete_run_with_usage(
                    conn,
                    run_id,
                    &json!({ "case": case.name, "category": case.category }),
                    res.latency_ms,
                    res.usage.as_ref(),
                    backend.endpoint_id(),
                );
                Ok((res.latency_ms, fields))
            } else {
                let _ = fail_run(
                    conn,
                    run_id,
                    "The model did not return a parseable analysis with a non-empty core field.",
                );
                Err((
                    "The model did not return a parseable analysis with a non-empty core field."
                        .into(),
                    Some(res.latency_ms),
                ))
            }
        }
        Err(e) => {
            let _ = fail_run(conn, run_id, &e.to_string());
            Err((e.to_string(), None))
        }
    }
}

/// One persisted golden run, as the dashboard renders it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoldenRunRow {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub backend: String,
    pub passed: i64,
    pub failed: i64,
    pub results: Value,
}

/// The most recent golden runs (A6 usage dashboard's eval column).
pub fn latest_golden_runs(conn: &Connection, limit: usize) -> Vec<GoldenRunRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, started_at, finished_at, backend, passed, failed, results
           FROM golden_test_runs ORDER BY id DESC LIMIT ?1",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    stmt.query_map(params![limit as i64], |r| {
        Ok(GoldenRunRow {
            id: r.get(0)?,
            started_at: r.get(1)?,
            finished_at: r.get(2)?,
            backend: r.get(3)?,
            passed: r.get(4)?,
            failed: r.get(5)?,
            results: r
                .get::<_, Option<String>>(6)?
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null),
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// The change-detection fingerprint for "re-run the golden set on prompt or
/// role changes": the analysis + eval prompt versions and the current role
/// map. When maintenance sees a stored fingerprint that differs from this,
/// the set re-runs and the new fingerprint is stored.
pub fn golden_context_fingerprint(conn: &Connection) -> String {
    let roles = crate::ai_hub::load_roles(conn)
        .map(|r| serde_json::to_string(&r).unwrap_or_default())
        .unwrap_or_default();
    let material = format!(
        "{}|{}|{roles}",
        PROMPT_VERSIONS_TICKET_ANALYSIS, PROMPT_VERSIONS_GOLDEN_EVAL,
    );
    let digest = Sha256::digest(material.as_bytes());
    crate::crypto::hex(&digest)
}

/// Whether the stored fingerprint is stale (maintenance asks this on its
/// daily pass).
pub fn golden_fingerprint_stale(conn: &Connection) -> bool {
    let current = golden_context_fingerprint(conn);
    let stored = crate::settings::get_string(conn, "ai_golden_fingerprint")
        .ok()
        .flatten();
    stored.as_deref() != Some(current.as_str())
}

/// Persist the current fingerprint after a successful run.
pub fn store_golden_fingerprint(conn: &Connection) -> Result<()> {
    crate::settings::set_string(
        conn,
        "ai_golden_fingerprint",
        &golden_context_fingerprint(conn),
    )
}

#[cfg(test)]
mod batch13_a6_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    /// A6: a disabled backend still writes an honest run row — passed=0,
    /// failed=9, one error per case — instead of pretending nothing happened.
    #[test]
    fn golden_set_with_disabled_backend_writes_honest_failed_run() {
        let conn = db();
        let seeded: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM golden_test_set WHERE active = 1",
                [],
                |r| r.get(0),
            )
            .expect("seeded");
        assert_eq!(seeded, 9, "the 9 seeded cases exist");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let summary = rt
            .block_on(run_golden_set(
                &conn,
                &crate::ai_pipeline::AiBackend::Disabled,
            ))
            .expect("run");
        assert_eq!(
            summary.get("passed").and_then(|v| v.as_i64()),
            Some(0),
            "nothing passes without a backend"
        );
        assert_eq!(
            summary.get("failed").and_then(|v| v.as_i64()),
            Some(9),
            "all 9 cases reported honestly"
        );
        let runs = latest_golden_runs(&conn, 5);
        assert_eq!(runs.len(), 1, "one persisted run row");
        assert_eq!(runs[0].backend, "disabled");
        assert_eq!(runs[0].passed, 0);
        assert_eq!(runs[0].failed, 9);
        let results = runs[0].results.as_array().expect("results array");
        assert_eq!(results.len(), 9);
        assert!(
            results
                .iter()
                .all(|c| c.get("error").and_then(|e| e.as_str()).is_some()),
            "every failed case carries its error"
        );
    }

    /// A6: the fingerprint is stale after a role-map change and fresh right
    /// after storing — the "re-run on prompt/role changes" trigger.
    #[test]
    fn golden_fingerprint_tracks_role_changes() {
        let conn = db();
        assert!(
            golden_fingerprint_stale(&conn),
            "nothing stored yet means stale"
        );
        store_golden_fingerprint(&conn).expect("store");
        assert!(
            !golden_fingerprint_stale(&conn),
            "fresh right after storing"
        );
        let mut roles = crate::ai_hub::load_roles(&conn).expect("roles");
        roles.set(
            crate::ai_hub::AiRole::Classify,
            Some(crate::ai_hub::RoleAssignment {
                endpoint: "lmstudio".into(),
                model: Some("test-model".into()),
            }),
        );
        crate::ai_hub::save_roles(&conn, &roles).expect("save roles");
        assert!(
            golden_fingerprint_stale(&conn),
            "role reassignment makes the stored fingerprint stale"
        );
    }
}

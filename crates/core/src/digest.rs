use chrono::Utc;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::error::Result;

pub fn apply_m057_ai_digests(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ai_digests (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            period_start TEXT NOT NULL,
            period_end   TEXT NOT NULL,
            trigger_kind TEXT NOT NULL DEFAULT 'manual'
                     CHECK (trigger_kind IN ('manual','scheduled')),
            facts        TEXT NOT NULL,
            narrative    TEXT,
            ai_run_id    INTEGER REFERENCES ai_runs(id),
            status       TEXT NOT NULL DEFAULT 'complete'
                     CHECK (status IN ('complete','facts_only','failed')),
            created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
        );
        CREATE INDEX IF NOT EXISTS idx_ai_digests_created
            ON ai_digests (created_at DESC);",
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DigestRow {
    pub id: i64,
    pub period_start: String,
    pub period_end: String,
    pub trigger_kind: String,
    pub facts: Value,
    pub narrative: Option<String>,
    pub ai_run_id: Option<i64>,
    pub status: String,
    pub created_at: String,
}

fn digest_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<DigestRow> {
    let facts_raw: String = r.get(4)?;
    Ok(DigestRow {
        id: r.get(0)?,
        period_start: r.get(1)?,
        period_end: r.get(2)?,
        trigger_kind: r.get(3)?,
        facts: serde_json::from_str(&facts_raw).unwrap_or_else(|_| json!({})),
        narrative: r.get(5)?,
        ai_run_id: r.get(6)?,
        status: r.get(7)?,
        created_at: r.get(8)?,
    })
}

const DIGEST_COLUMNS: &str =
    "id, period_start, period_end, trigger_kind, facts, narrative, ai_run_id, status, created_at";

pub struct NewDraft<'a> {
    pub period_start: &'a str,
    pub period_end: &'a str,
    pub trigger_kind: &'a str,
    pub facts: &'a Value,
    pub narrative: Option<&'a str>,
    pub ai_run_id: Option<i64>,
    pub status: &'a str,
}

pub fn store_digest(conn: &Connection, draft: &NewDraft<'_>) -> Result<i64> {
    conn.execute(
        "INSERT INTO ai_digests
             (period_start, period_end, trigger_kind, facts, narrative, ai_run_id, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            draft.period_start,
            draft.period_end,
            draft.trigger_kind,
            serde_json::to_string(draft.facts).unwrap_or_else(|_| "{}".into()),
            draft.narrative,
            draft.ai_run_id,
            draft.status
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_digests(conn: &Connection, query: Option<&str>, limit: i64) -> Vec<DigestRow> {
    conn.prepare(&format!(
        "SELECT {DIGEST_COLUMNS} FROM ai_digests
          WHERE (?1 IS NULL
                 OR narrative LIKE '%' || ?1 || '%'
                 OR facts LIKE '%' || ?1 || '%')
          ORDER BY id DESC LIMIT ?2"
    ))
    .and_then(|mut stmt| {
        stmt.query_map(params![query, limit], digest_from_row)
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
    })
    .unwrap_or_default()
}

#[must_use]
pub fn digest_by_id(conn: &Connection, id: i64) -> Option<DigestRow> {
    conn.query_row(
        &format!("SELECT {DIGEST_COLUMNS} FROM ai_digests WHERE id = ?1"),
        params![id],
        |r| digest_from_row(r).map(Some),
    )
    .unwrap_or(None)
}

#[must_use]
pub fn latest_digest_created_at(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT created_at FROM ai_digests ORDER BY id DESC LIMIT 1",
        [],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

fn day_bounds(days: i64) -> (String, String) {
    let end = Utc::now();
    let start = end - chrono::Duration::try_days(days).unwrap_or_default();
    (
        start.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        end.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
    )
}

fn volume_facts(conn: &Connection, start: &str, end: &str, prev_start: &str) -> Value {
    let new_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE remote_created_at >= ?1 AND remote_created_at < ?2 AND deleted_at IS NULL",
            params![start, end],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let closed_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE closed_at >= ?1 AND closed_at < ?2 AND deleted_at IS NULL",
            params![start, end],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let prev_new: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE remote_created_at >= ?1 AND remote_created_at < ?2 AND deleted_at IS NULL",
            params![prev_start, start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let per_day: Vec<(String, i64)> = conn
        .prepare(
            "SELECT substr(remote_created_at, 1, 10) AS d, COUNT(*)
               FROM conversations
              WHERE remote_created_at >= ?1 AND remote_created_at < ?2 AND deleted_at IS NULL
              GROUP BY d ORDER BY d",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![start, end], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let backlog: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE status IN ('active','pending') AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    json!({
        "new_conversations": new_count,
        "closed_conversations": closed_count,
        "previous_period_new_conversations": prev_new,
        "new_conversations_delta": new_count - prev_new,
        "new_per_day": per_day.iter().map(|(d, n)| json!({"day": d, "count": n})).collect::<Vec<_>>(),
        "backlog_now": backlog,
    })
}

fn anomaly_facts(conn: &Connection, start: &str, end: &str) -> Value {
    let per_day: Vec<(String, i64)> = conn
        .prepare(
            "SELECT substr(remote_created_at, 1, 10) AS d, COUNT(*)
               FROM conversations
              WHERE remote_created_at >= ?1 AND remote_created_at < ?2 AND deleted_at IS NULL
              GROUP BY d ORDER BY d",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![start, end], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let mut anomalies: Vec<Value> = Vec::new();
    if per_day.len() >= 4 {
        let values: Vec<f64> = per_day.iter().map(|(_, n)| *n as f64).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance =
            values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / values.len() as f64;
        let sigma = variance.sqrt();
        for (day, count) in &per_day {
            if *count >= 3 && sigma > 0.0 && (*count as f64 - mean) > 2.0 * sigma {
                anomalies.push(json!({
                    "kind": "volume_spike",
                    "day": day,
                    "count": count,
                    "period_mean": (mean * 10.0).round() / 10.0,
                    "detail": format!(
                        "{count} new conversations on {day} versus a period mean of {:.1} — more than two standard deviations above.",
                        (mean * 10.0).round() / 10.0
                    ),
                }));
            }
        }
    }
    let backlog_now: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE status IN ('active','pending') AND deleted_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let backlog_week_ago: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations
              WHERE COALESCE(remote_created_at, created_at) < ?1
                AND deleted_at IS NULL
                AND ((closed_at IS NULL AND status IN ('active','pending'))
                     OR (closed_at IS NOT NULL AND closed_at >= ?1))",
            params![start],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if backlog_now >= 10
        && backlog_week_ago > 0
        && backlog_now > backlog_week_ago + backlog_week_ago / 2
    {
        anomalies.push(json!({
            "kind": "backlog_jump",
            "from": backlog_week_ago,
            "to": backlog_now,
            "detail": format!(
                "Open backlog grew from {backlog_week_ago} to {backlog_now} conversations — more than 50% up."
            ),
        }));
    }
    json!({ "anomalies": anomalies })
}

fn topic_facts(conn: &Connection) -> Value {
    let clusters: Vec<Value> = conn
        .prepare(
            "SELECT label, conversation_count FROM issue_clusters
              ORDER BY conversation_count DESC, id DESC LIMIT 5",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "label": r.get::<_, String>(0)?,
                    "conversations": r.get::<_, i64>(1)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let tags: Vec<Value> = conn
        .prepare(
            "SELECT t.tag, COUNT(*) AS uses
               FROM conversation_tags t
              GROUP BY t.tag ORDER BY uses DESC LIMIT 8",
        )
        .and_then(|mut stmt| {
            stmt.query_map([], |r| {
                Ok(json!({
                    "tag": r.get::<_, String>(0)?,
                    "conversations": r.get::<_, i64>(1)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    json!({ "top_issue_clusters": clusters, "top_tags": tags })
}

fn sentiment_facts(conn: &Connection, start: &str, end: &str) -> Value {
    let rows: Vec<String> = conn
        .prepare(
            "SELECT r.response_json FROM ai_runs r
              JOIN (SELECT conversation_id, MAX(id) AS max_id
                      FROM ai_runs
                     WHERE type = 'ticket_analysis' AND status = 'completed'
                       AND conversation_id IS NOT NULL
                     GROUP BY conversation_id) latest
                ON latest.max_id = r.id
              JOIN conversations c ON c.id = r.conversation_id
              WHERE c.remote_created_at >= ?1 AND c.remote_created_at < ?2
                AND c.deleted_at IS NULL",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![start, end], |r| r.get::<_, String>(0))
                .map(|rows| {
                    rows.filter_map(|r| r.ok())
                        .filter_map(|json_str| {
                            let v: Value = serde_json::from_str(&json_str).ok()?;
                            v.get("sentiment")
                                .and_then(|s| s.as_str())
                                .map(String::from)
                        })
                        .collect::<Vec<String>>()
                })
        })
        .unwrap_or_default();
    if rows.is_empty() {
        return json!({ "analyzed_conversations": 0, "note": "No ticket analyses in range — sentiment counts are absent, not zero." });
    }
    let count = |s: &str| rows.iter().filter(|r| r == &s).count();
    json!({
        "analyzed_conversations": rows.len(),
        "positive": count("positive"),
        "neutral": count("neutral"),
        "negative": count("negative"),
        "frustrated": count("frustrated"),
    })
}

fn workload_facts(conn: &Connection, start: &str, end: &str) -> Value {
    let replies_per_user: Vec<Value> = conn
        .prepare(
            "SELECT COALESCE(NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), ''), th.from_name, 'Unknown'),
                    COUNT(*) AS replies
               FROM conversation_threads th
               LEFT JOIN users u ON u.email = th.from_email AND th.from_email IS NOT NULL
              WHERE th.type IN ('message','chat','phone')
                AND th.from_type IN ('user','agent')
                AND th.state = 'published'
                AND th.deleted_at IS NULL
                AND th.remote_created_at >= ?1 AND th.remote_created_at < ?2
              GROUP BY u.id, th.from_name ORDER BY replies DESC LIMIT 8",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![start, end], |r| {
                Ok(json!({ "user": r.get::<_, String>(0)?, "replies": r.get::<_, i64>(1)? }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let busiest: Option<(String, i64)> = conn
        .prepare(
            "SELECT substr(remote_created_at, 1, 13) AS hour, COUNT(*)
               FROM conversations
              WHERE remote_created_at >= ?1 AND remote_created_at < ?2 AND deleted_at IS NULL
              GROUP BY hour ORDER BY COUNT(*) DESC LIMIT 1",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![start, end], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map(|mut rows| rows.next().map(|r| r.unwrap_or(("".into(), 0))))
        })
        .unwrap_or(None);
    json!({
        "replies_per_user": replies_per_user,
        "busiest_hour": busiest.as_ref().map(|(h, n)| json!({"hour": h, "new_conversations": n})),
    })
}

fn happiness_facts(conn: &Connection, start: &str, end: &str) -> Value {
    let count = |rating: &str| {
        conn.query_row(
            "SELECT COUNT(*) FROM ratings
              WHERE rating = ?1 AND remote_created_at >= ?2 AND remote_created_at < ?3",
            params![rating, start, end],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
    };
    let great = count("great");
    let okay = count("okay");
    let not_good = count("not-good");
    let total = great + okay + not_good;
    json!({
        "great": great,
        "okay": okay,
        "not_good": not_good,
        "total": total,
        "source": "satisfaction webhook ratings received in range",
    })
}

fn change_facts(conn: &Connection, start: &str) -> Value {
    let since = latest_digest_created_at(conn).unwrap_or_else(|| start.to_string());
    let new_gaps: Vec<Value> = conn
        .prepare(
            "SELECT kind, query_text, created_at FROM knowledge_gap_candidates
              WHERE datetime(created_at) >= datetime(?1) ORDER BY id DESC LIMIT 12",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![since], |r| {
                Ok(json!({
                    "kind": r.get::<_, Option<String>>(0)?,
                    "question": r.get::<_, String>(1)?,
                    "created_at": r.get::<_, String>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let new_articles: Vec<Value> = conn
        .prepare(
            "SELECT name, status, remote_updated_at FROM docs_articles
              WHERE datetime(remote_updated_at) >= datetime(?1)
              ORDER BY datetime(remote_updated_at) DESC LIMIT 12",
        )
        .and_then(|mut stmt| {
            stmt.query_map(params![since], |r| {
                Ok(json!({
                    "name": r.get::<_, String>(0)?,
                    "status": r.get::<_, Option<String>>(1)?,
                    "updated_at": r.get::<_, Option<String>>(2)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    let new_documents: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM knowledge_documents WHERE datetime(updated_at) >= datetime(?1)",
            params![since],
            |r| r.get(0),
        )
        .unwrap_or(0);
    json!({
        "measured_since": since,
        "new_gap_candidates": new_gaps,
        "changed_help_center_articles": new_articles,
        "new_local_documents": new_documents,
    })
}

pub fn collect_facts(conn: &Connection, days: i64) -> (Value, String, String) {
    let days = days.clamp(1, 90);
    let (start, end) = day_bounds(days);
    let prev_start = chrono::DateTime::parse_from_rfc3339(&start)
        .map(|d| d - chrono::Duration::try_days(days).unwrap_or_default())
        .map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .unwrap_or_else(|_| start.clone());
    let facts = json!({
        "period": { "start": start, "end": end, "days": days },
        "volume": volume_facts(conn, &start, &end, &prev_start),
        "anomalies": anomaly_facts(conn, &start, &end),
        "topics": topic_facts(conn),
        "sentiment": sentiment_facts(conn, &start, &end),
        "workload": workload_facts(conn, &start, &end),
        "happiness": happiness_facts(conn, &start, &end),
        "what_changed": change_facts(conn, &start),
        "generated_by": "SupportOS deterministic digest facts (SQL-computed)",
    });
    (facts, start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_conversation(conn: &Connection, remote_id: i64, created: &str, status: &str) -> i64 {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at, closed_at)
             VALUES (?1, ?2, 'Digest seed', 'p', ?3, ?4,
                CASE WHEN ?3 = 'closed' THEN ?4 ELSE NULL END)",
            params![remote_id, remote_id, status, created],
        )
        .expect("conversation");
        conn.last_insert_rowid()
    }

    #[test]
    fn digest_archive_roundtrips_and_searches() {
        let conn = db();
        let id = store_digest(
            &conn,
            &NewDraft {
                period_start: "2026-10-14T00:00:00.000Z",
                period_end: "2026-10-21T00:00:00.000Z",
                trigger_kind: "manual",
                facts: &json!({"volume": {"new_conversations": 12}}),
                narrative: Some("Twelve new conversations; export failures dominate."),
                ai_run_id: None,
                status: "complete",
            },
        )
        .expect("store");
        assert!(id > 0);
        let rows = list_digests(&conn, None, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "complete");
        assert!(
            rows[0]
                .narrative
                .as_deref()
                .unwrap_or_default()
                .contains("export")
        );

        assert_eq!(list_digests(&conn, Some("export fail"), 10).len(), 1);
        store_digest(
            &conn,
            &NewDraft {
                period_start: "2026-10-07T00:00:00.000Z",
                period_end: "2026-10-14T00:00:00.000Z",
                trigger_kind: "scheduled",
                facts: &json!({"topics": {"top_tags": [{"tag": "billing", "conversations": 5}]}}),
                narrative: None,
                ai_run_id: None,
                status: "facts_only",
            },
        )
        .expect("store 2");
        assert_eq!(list_digests(&conn, Some("billing"), 10).len(), 1);
        assert_eq!(list_digests(&conn, None, 10).len(), 2);
        assert_eq!(digest_by_id(&conn, id).expect("row").trigger_kind, "manual");
    }

    #[test]
    fn facts_collect_volume_anomaly_and_happiness() {
        let conn = db();
        let day_iso = |days_back: i64| {
            (chrono::Utc::now() - chrono::Duration::try_days(days_back).unwrap_or_default())
                .format("%Y-%m-%dT09:00:00.000Z")
                .to_string()
        };
        for days_back in [6, 5, 4, 3, 2, 1] {
            let _ = seed_conversation(&conn, days_back, &day_iso(days_back), "active");
        }
        let now_iso = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S.000Z")
            .to_string();
        let loud_day = now_iso.get(0..10).unwrap_or("").to_string();
        for n in 0..10 {
            let _ = seed_conversation(&conn, 100 + n, &now_iso, "active");
        }
        conn.execute(
            "INSERT INTO ratings (remote_id, conversation_id, rating, remote_created_at)
             VALUES (501, 1, 'great', ?1),
                    (502, 1, 'not-good', ?2)",
            params![day_iso(2), day_iso(1)],
        )
        .expect("ratings");

        let (facts, start, end) = collect_facts(&conn, 7);
        assert!(end > start);
        assert_eq!(facts["volume"]["new_conversations"], 16);
        let anomalies = facts["anomalies"]["anomalies"]
            .as_array()
            .expect("anomalies");
        assert!(
            anomalies
                .iter()
                .any(|a| a["kind"] == "volume_spike" && a["day"] == loud_day.as_str()),
            "the loud day {loud_day} must be flagged: {anomalies:?}"
        );
        assert_eq!(facts["happiness"]["great"], 1);
        assert_eq!(facts["happiness"]["not_good"], 1);
        let _ = seed_conversation(&conn, 999, "2020-09-01T09:00:00.000Z", "active");
        let (facts2, _, _) = collect_facts(&conn, 7);
        assert_eq!(facts2["volume"]["new_conversations"], 16);
    }

    #[test]
    fn sentiment_facts_absent_without_analysis_runs() {
        let conn = db();
        let (start, end) = day_bounds(7);
        let s = sentiment_facts(&conn, &start, &end);
        assert_eq!(s["analyzed_conversations"], 0);
        assert!(
            s["note"]
                .as_str()
                .unwrap_or_default()
                .contains("absent, not zero")
        );
    }
}

#[cfg(test)]
mod batch24_audit55_tests {
    use super::*;
    use chrono::Utc;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    struct Bounds {
        start: String,
        end: String,
        mid: String,
    }

    fn bounds() -> Bounds {
        let end = Utc::now();
        let start = end - chrono::Duration::try_days(7).unwrap_or_default();
        let mid = end - chrono::Duration::try_days(3).unwrap_or_default();
        let fmt = |d: chrono::DateTime<Utc>| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        Bounds {
            start: fmt(start),
            end: fmt(end),
            mid: fmt(mid),
        }
    }

    const OLD: &str = "2026-01-01T00:00:00.000Z";

    fn insert(
        conn: &Connection,
        remote: i64,
        created: &str,
        status: &str,
        closed_at: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                                        remote_created_at, closed_at)
             VALUES (?1, ?1, 's', 'p', ?2, ?3, ?4)",
            params![remote, status, created, closed_at],
        )
        .expect("conversation");
    }

    #[test]
    fn audit55_worked_down_backlog_is_not_a_jump() {
        let conn = db();
        let b = bounds();
        for i in 0..3 {
            insert(&conn, 100 + i, OLD, "active", None);
        }
        for i in 0..9 {
            insert(&conn, 200 + i, OLD, "closed", Some(&b.mid));
        }
        for i in 0..9 {
            insert(&conn, 300 + i, &b.mid, "active", None);
        }
        let facts = anomaly_facts(&conn, &b.start, &b.end);
        let anomalies = facts["anomalies"].as_array().expect("anomalies");
        assert!(
            anomalies.iter().all(|a| a["kind"] != json!("backlog_jump")),
            "a flat backlog must not be reported as a jump: {anomalies:?}"
        );
    }

    #[test]
    fn audit55_genuine_backlog_jump_still_fires() {
        let conn = db();
        let b = bounds();
        for i in 0..10 {
            insert(&conn, 400 + i, OLD, "active", None);
        }
        for i in 0..6 {
            insert(&conn, 500 + i, &b.mid, "active", None);
        }
        let facts = anomaly_facts(&conn, &b.start, &b.end);
        let anomalies = facts["anomalies"].as_array().expect("anomalies");
        let jump = anomalies
            .iter()
            .find(|a| a["kind"] == json!("backlog_jump"))
            .expect("the genuine jump must be reported");
        assert_eq!(jump["from"], json!(10));
        assert_eq!(jump["to"], json!(16));
    }

    #[test]
    fn audit55_previously_closed_tickets_stay_excluded() {
        let conn = db();
        let b = bounds();
        let older_close = "2025-12-01T00:00:00.000Z";
        for i in 0..10 {
            insert(&conn, 600 + i, OLD, "active", None);
        }
        for i in 0..4 {
            insert(&conn, 700 + i, OLD, "closed", Some(older_close));
        }
        for i in 0..6 {
            insert(&conn, 800 + i, &b.mid, "active", None);
        }
        let facts = anomaly_facts(&conn, &b.start, &b.end);
        let anomalies = facts["anomalies"].as_array().expect("anomalies");
        let jump = anomalies
            .iter()
            .find(|a| a["kind"] == json!("backlog_jump"))
            .expect("16 > 10 + 5 fires the jump");
        assert_eq!(jump["from"], json!(10), "only the 10 open-at-start count");
    }
}

#[cfg(test)]
mod batch30_tests {
    use super::*;

    #[test]
    fn change_facts_counts_space_format_docs_after_iso_t_since() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");

        conn.execute("INSERT INTO knowledge_sources (name) VALUES ('src')", [])
            .expect("source");
        conn.execute(
            "INSERT INTO knowledge_documents (source_id, title, updated_at)
             VALUES (1, 'doc updated later same day', '2026-10-20 11:30:00')",
            [],
        )
        .expect("doc later");
        conn.execute(
            "INSERT INTO knowledge_documents (source_id, title, updated_at)
             VALUES (1, 'doc updated earlier same day', '2026-10-20 09:00:00')",
            [],
        )
        .expect("doc earlier");

        conn.execute(
            "INSERT INTO docs_articles (remote_id, name, status, remote_updated_at)
             VALUES (11, 'Article later same day (space)', 'published', '2026-10-20 11:30:00')",
            [],
        )
        .expect("article later");
        conn.execute(
            "INSERT INTO docs_articles (remote_id, name, status, remote_updated_at)
             VALUES (12, 'Article before since (ISO-T)', 'published', '2026-10-19T09:00:00.000Z')",
            [],
        )
        .expect("article before");

        let facts = change_facts(&conn, "2026-10-20T10:00:00.000Z");
        assert_eq!(
            facts["new_local_documents"], 1,
            "the 11:30 space-format update is after the 10:00 ISO-T bound"
        );
        let articles = facts["changed_help_center_articles"]
            .as_array()
            .expect("articles");
        assert_eq!(articles.len(), 1, "only the later same-day article counts");
        assert_eq!(articles[0]["name"], "Article later same day (space)");
    }
}

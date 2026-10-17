use std::time::Instant;

use rusqlite::{params, Connection};

use crate::error::Result;

pub const DATASET_SIZE: u32 = 2000;

pub const MAX_QUERY_MS: u128 = 500;

pub fn populate_synthetic_dataset(conn: &Connection) -> Result<()> {
    crate::activity::apply_m003(conn)?;
    crate::ticket_states::apply_m004(conn)?;
    crate::search::apply_fts_migration(conn)?;
    let _ = conn.execute(
        "ALTER TABLE conversations RENAME COLUMN mailbox_id TO mailbox_local_id",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE conversations RENAME COLUMN assignee_id TO assignee_local_id",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE conversations RENAME COLUMN customer_id TO customer_local_id",
        [],
    );

    for i in 1..=DATASET_SIZE as i64 {
        let status = match i % 3 {
            0 => "closed",
            1 => "active",
            _ => "pending",
        };
        let mailbox_id = 101 + (i % 2);
        let customer_id = 2001 + (i % 100);
        let subject = format!("Conversation #{i} - Support request");
        let preview = format!("Customer reports issue #{i} with product feature...");

        conn.execute(
            "INSERT INTO conversations (remote_id, number, status, mailbox_local_id, customer_local_id, subject, preview)
             VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6)",
            params![i, status, mailbox_id, customer_id, subject, preview],
        )?;

        crate::search::index_conversation(conn, i, &subject, &preview)?;

        if i % 2 == 0 {
            crate::activity::record_event(
                conn,
                &crate::activity::ActivityEvent {
                    id: None,
                    conversation_id: i,
                    event_type: "message".into(),
                    actor_type: "customer".into(),
                    actor_id: None,
                    occurred_at: format!("2026-01-{:02}T10:00:00Z", (i % 28) + 1),
                    dedup_key: format!("synth_evt_{i}"),
                },
            )?;
            crate::activity::update_derived_columns(conn, i)?;
        }
    }

    for i in 1..=100 {
        let name = format!("Customer{i}");
        let email = format!("customer{i}@example.com");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name, email, organization)
             VALUES (?1, ?2, 'Last', ?3, ?4)",
            params![
                2000 + i,
                name,
                email,
                if i <= 50 { "Acme Corp" } else { "Globex" }
            ],
        )?;
        crate::search::index_customer(
            conn,
            2000 + i,
            &name,
            "Last",
            &email,
            if i <= 50 { "Acme Corp" } else { "Globex" },
        )?;
    }

    Ok(())
}

pub fn benchmark_query(conn: &Connection, label: &str, sql: &str) -> Result<(u128, usize)> {
    let start = Instant::now();
    let mut stmt = conn.prepare(sql)?;
    let count = stmt.query_map([], |r| r.get::<_, i64>(0))?.count();
    let elapsed = start.elapsed().as_millis();
    tracing::info!(label, elapsed_ms = elapsed, count, "benchmark query");
    Ok((elapsed, count))
}

pub fn run_performance_tests(conn: &Connection) -> Result<Vec<PerfResult>> {
    populate_synthetic_dataset(conn)?;
    let mut results = Vec::new();

    let (ms, count) = benchmark_query(
        conn,
        "count_by_response_state",
        "SELECT COUNT(*) FROM conversations GROUP BY response_state",
    )?;
    results.push(PerfResult {
        label: "count_by_response_state".into(),
        elapsed_ms: ms,
        result_count: count,
        within_bound: ms <= MAX_QUERY_MS,
    });

    let (ms, count) = benchmark_query(
        conn,
        "filter_by_status_active",
        "SELECT COUNT(*) FROM conversations WHERE status = 'active'",
    )?;
    results.push(PerfResult {
        label: "filter_by_status_active".into(),
        elapsed_ms: ms,
        result_count: count,
        within_bound: ms <= MAX_QUERY_MS,
    });

    let start = Instant::now();
    let search_results = crate::search::universal_search(conn, "Conversation")?;
    let elapsed = start.elapsed().as_millis();
    results.push(PerfResult {
        label: "universal_search_fts5".into(),
        elapsed_ms: elapsed,
        result_count: search_results.len(),
        within_bound: elapsed <= MAX_QUERY_MS,
    });

    Ok(results)
}

#[derive(Debug, Clone)]
pub struct PerfResult {
    pub label: String,
    pub elapsed_ms: u128,
    pub result_count: usize,
    pub within_bound: bool,
}

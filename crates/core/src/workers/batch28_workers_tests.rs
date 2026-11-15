use super::*;

fn temp_db() -> crate::db::DbHandle {
    let dir = std::env::temp_dir().join(format!(
        "spp-workers-b28-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let mut conn = crate::db::open(&dir.join("workers.db")).expect("open db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    crate::db::DbHandle::new(conn)
}

fn manager(db: &crate::db::DbHandle) -> Arc<WorkerManager> {
    Arc::new(WorkerManager::new(
        db.clone(),
        None,
        Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
        crate::event_bus::EventBus::default(),
        std::env::temp_dir(),
        None,
    ))
}

fn rating(remote_id: i64) -> crate::helpscout::HsRating {
    crate::helpscout::HsRating {
        remote_id,
        conversation_id: None,
        thread_id: None,
        rating: Some("great".into()),
        comment: Some("Fast, clear help".into()),
        customer_id: None,
        customer_name: Some("Rina Chowdhury".into()),
        user_id: None,
        created_at: Some("2026-10-22T01:02:03Z".into()),
    }
}

fn rating_links(
    db: &crate::db::DbHandle,
    remote_id: i64,
) -> (Option<i64>, Option<i64>, Option<i64>) {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT conversation_id, customer_local_id, user_local_id
               FROM ratings WHERE remote_id = ?1",
            rusqlite::params![remote_id],
            |r| {
                Ok((
                    r.get::<_, Option<i64>>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .expect("rating row")
    })
}

fn seed_conversations(conn: &Connection, n: i64) {
    for i in 1..=n {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (?1, ?1, 'Backfill subject', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
            rusqlite::params![9000 + i],
        )
        .expect("conversation");
    }
}

fn jobs_of_kind(db: &crate::db::DbHandle, kind: &str) -> i64 {
    db.with_read(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE type = ?1 AND status = 'queued'",
            rusqlite::params![kind],
            |r| r.get(0),
        )
        .unwrap_or(0)
    })
}

#[test]
fn audit87_rating_upsert_relinks_conversation_customer_on_conflict() {
    let db = temp_db();
    db.with_write(|conn| {
        let inserted = upsert_rating_row(conn, &rating(501), None, None, None).expect("insert");
        assert!(inserted, "the first upsert is a fresh insert");
    });
    assert_eq!(
        rating_links(&db, 501),
        (None, None, None),
        "stored unlinked"
    );

    db.with_write(|conn| {
        conn.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status, remote_created_at)
             VALUES (900, 900, 'Rated conversation', 'p', 'closed', '2026-10-20T08:00:00.000Z')",
            [],
        )
        .expect("conversation");
        conn.execute(
            "INSERT INTO customers (remote_id, first_name, last_name) VALUES (77, 'Rina', 'Chowdhury')",
            [],
        )
        .expect("customer");
    });
    let (conv_local, customer_local): (i64, i64) = db.with_read(|conn| {
        (
            conn.query_row(
                "SELECT id FROM conversations WHERE remote_id = 900",
                [],
                |r| r.get(0),
            )
            .expect("conv local"),
            conn.query_row("SELECT id FROM customers WHERE remote_id = 77", [], |r| {
                r.get(0)
            })
            .expect("customer local"),
        )
    });
    db.with_write(|conn| {
        let inserted = upsert_rating_row(
            conn,
            &rating(501),
            Some(conv_local),
            Some(customer_local),
            None,
        )
        .expect("conflict upsert");
        assert!(!inserted, "the second upsert is a conflict, not an insert");
    });
    assert_eq!(
        rating_links(&db, 501),
        (Some(conv_local), Some(customer_local), None),
        "the conflict path re-links the freshly-resolved ids"
    );

    db.with_write(|conn| {
        upsert_rating_row(conn, &rating(501), None, None, None).expect("null upsert");
    });
    assert_eq!(
        rating_links(&db, 501),
        (Some(conv_local), Some(customer_local), None),
        "an unresolved re-fetch never clears an existing link"
    );
}

#[test]
fn audit90_post_sync_ai_backfill_covers_every_conversation() {
    let db = temp_db();
    db.with_write(|conn| {
        crate::settings::set_string(conn, "lmstudio_base_url", "http://127.0.0.1:1234/v1")
            .expect("backend");
        seed_conversations(conn, 60);
    });
    let manager = manager(&db);
    manager.on_after_initial_sync();

    assert_eq!(
        jobs_of_kind(&db, "analyze_ticket"),
        60,
        "every unanalyzed conversation is enqueued — the 50 cap is gone"
    );
    assert_eq!(
        jobs_of_kind(&db, "extract_attributes"),
        60,
        "the attributes backfill covers every conversation too"
    );
}

#[test]
fn audit90_post_sync_ai_backfill_skips_deleted_conversations() {
    let db = temp_db();
    db.with_write(|conn| {
        crate::settings::set_string(conn, "lmstudio_base_url", "http://127.0.0.1:1234/v1")
            .expect("backend");
        seed_conversations(conn, 3);
        conn.execute(
            "UPDATE conversations SET deleted_at = datetime('now') WHERE remote_id = 9002",
            [],
        )
        .expect("soft delete");
    });
    let manager = manager(&db);
    manager.on_after_initial_sync();
    assert_eq!(jobs_of_kind(&db, "analyze_ticket"), 2, "only the live two");
}

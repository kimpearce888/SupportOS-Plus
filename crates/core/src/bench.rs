//! Performance measurement harness (TODO 46).
//!
//! These are `#[ignore]`d measurement tests, not correctness tests: they
//! seed a realistically large local dataset through the real sync write
//! path (`SyncEngine::run_resource` against an in-process provider —
//! upserts, FTS, tags, all of it), then time the operations that bound
//! startup, sync and UI responsiveness. Run them explicitly and print
//! the numbers:
//!
//! ```text
//! cargo test -p supportos-plusplus-core bench -- --ignored --nocapture
//! ```
//!
//! Assertions use deliberately generous bounds (order-of-magnitude
//! regression guards, not machine-jitter guards). The measurements
//! recorded in `completed.md` (final batch) come from this exact
//! procedure on the development machine. Process startup time and max
//! RSS of the real binary are measured separately with the release
//! binary + AppImage smoke path — that procedure and its numbers are
//! documented alongside these.
//!
//! Scale (a solo-support-desk "realistically large" dataset):
//! 3 000 conversations × 3 threads, 5 000 customers, 100 tags, 50 users,
//! 3 mailboxes, 3 000 × 384-dim vector points.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::db::DbHandle;
use crate::helpscout::{
    ConversationQuery, CustomerQuery, HsConversation, HsCustomer, HsMailbox, HsThread, HsUser, Page,
};
use crate::sync_engine::SyncEngine;

const N_CONVERSATIONS: usize = 3_000;
const THREADS_PER_CONVERSATION: usize = 3;
const N_CUSTOMERS: usize = 5_000;
const N_TAGS: usize = 100;
const N_USERS: usize = 50;
const VECTOR_DIM: usize = 384;
const N_VECTOR_POINTS: usize = 3_000;
const PAGE_SIZE: usize = 50;

fn temp_bench_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "spp-bench-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("bench dir");
    dir
}

struct TempDirGuard(PathBuf);
impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic pseudo-random vector in [-1, 1] — reproducible across
/// runs and machines.
fn pseudo_vec(seed: u64, dim: usize) -> Vec<f32> {
    let mut state = splitmix(seed);
    (0..dim)
        .map(|_| {
            state = splitmix(state);
            ((state >> 33) as f64 / 2_147_483_648.0 - 1.0) as f32
        })
        .collect()
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn sample_conversation(i: usize) -> HsConversation {
    let remote: i64 = 100_000 + i as i64;
    HsConversation {
        remote_id: remote,
        number: 1000 + i as i64,
        kind: Some("email".into()),
        source_type: Some("email".into()),
        source_via: Some("customer".into()),
        subject: Some(format!("Subject line {i} — login trouble report")),
        preview: Some(format!(
            "Customer {} cannot access their account after the password \
             reset email failed to arrive; preview snippet {i}.",
            1 + (i % N_CUSTOMERS)
        )),
        status: match i % 5 {
            0 => "closed".into(),
            1 => "pending".into(),
            2 => "active".into(),
            3 => "active".into(),
            _ => "spam".into(),
        },
        state: Some("published".into()),
        mailbox_id: 1 + (i % 3) as i64,
        assignee_id: Some(1 + (i % N_USERS) as i64),
        assignee_type: Some("user".into()),
        assigned_team_id: Some(1),
        customer_id: 1 + (i % N_CUSTOMERS) as i64,
        priority: Some(match i % 3 {
            0 => "high".into(),
            1 => "normal".into(),
            _ => "low".into(),
        }),
        created_at: Some(format!("2026-09-{:02}T09:00:00Z", 1 + (i % 28))),
        updated_at: Some(format!("2026-10-{:02}T09:00:00Z", 1 + (i % 28))),
        closed_at: i
            .is_multiple_of(5)
            .then(|| format!("2026-10-{:02}T09:00:00Z", 1 + (i % 28))),
        snoozed_until: None,
        thread_count: THREADS_PER_CONVERSATION as i64,
        merged_into: None,
        tags: vec![
            format!("tag{}", i % N_TAGS),
            format!("tag{}", (i + 7) % N_TAGS),
        ],
        custom_fields: Vec::new(),
    }
}

fn sample_thread(conv_remote: i64, j: usize) -> HsThread {
    HsThread {
        remote_id: conv_remote * 10 + j as i64,
        conversation_id: conv_remote,
        kind: if j == 0 {
            "customer".into()
        } else {
            "message".into()
        },
        status: Some("active".into()),
        state: Some("published".into()),
        body: Some(format!(
            "<p>Thread body {j} for conversation {conv_remote}: the login flow \
             fails at the verification step with an unhelpful error.</p>"
        )),
        created_by_customer_id: Some(1 + (conv_remote as usize % N_CUSTOMERS) as i64),
        created_by_user_id: Some(1 + (conv_remote as usize % N_USERS) as i64),
        assigned_to_id: None,
        created_at: Some(format!("2026-09-{:02}T10:{:02}:00Z", 1 + (j % 28), j * 7)),
        to: vec![crate::helpscout::HsThreadRecipient {
            id: Some(conv_remote),
            email: Some(format!("customer{conv_remote}@example.com")),
        }],
        cc: Vec::new(),
        attachments: Vec::new(),
        scheduled_for: None,
    }
}

fn sample_customer(i: usize) -> HsCustomer {
    HsCustomer {
        remote_id: 1 + i as i64,
        first_name: Some(format!("First{i}")),
        last_name: Some("Customer".into()),
        email: Some(format!("customer{i}@example.com")),
        organization: Some("Analytical Engines".into()),
        job_title: Some("Engineer".into()),
        phone: Some("+15550000".into()),
        created_at: Some("2026-08-01T00:00:00Z".into()),
        updated_at: Some("2026-10-01T00:00:00Z".into()),
        photo_url: None,
        organization_id: Some(5),
        background: Some(format!(
            "Background notes for customer {i} — long-time account."
        )),
        age: None,
        gender: None,
        location: Some("London".into()),
        emails: vec![crate::helpscout::HsCustomerEmail {
            value: Some(format!("customer{i}@example.com")),
            kind: Some("home".into()),
        }],
        phones: Vec::new(),
        websites: Vec::new(),
        social_profiles: Vec::new(),
        address: None,
        properties: Vec::new(),
    }
}

/// In-process provider serving the deterministic dataset — the bench
/// measures the engine's *write path* (upserts, FTS, tags), never the
/// network.
struct LocalBenchProvider;

#[async_trait::async_trait]
impl crate::helpscout::HelpScoutProvider for LocalBenchProvider {
    fn kind(&self) -> &'static str {
        "local-bench"
    }

    async fn get_me(&self) -> crate::error::Result<HsUser> {
        Ok(HsUser {
            remote_id: 1,
            first_name: Some("Bench".into()),
            last_name: Some("Agent".into()),
            email: None,
            role: None,
            user_type: "user".into(),
            timezone: None,
            photo_url: None,
            initials: None,
            mention: None,
            job_title: None,
            phone: None,
            alternate_emails: Vec::new(),
            created_at: None,
            updated_at: None,
        })
    }

    async fn list_mailboxes(&self) -> crate::error::Result<Vec<HsMailbox>> {
        Ok((1..=3)
            .map(|m| HsMailbox {
                remote_id: m,
                name: format!("Mailbox {m}"),
                slug: Some(format!("mb{m}")),
                email: Some(format!("mb{m}@example.com")),
                created_at: None,
                updated_at: None,
            })
            .collect())
    }

    async fn list_users(&self) -> crate::error::Result<Vec<HsUser>> {
        Ok((1..=N_USERS)
            .map(|u| HsUser {
                remote_id: u as i64,
                first_name: Some(format!("Agent{u}")),
                last_name: Some("Support".into()),
                email: Some(format!("agent{u}@example.com")),
                role: Some("agent".into()),
                user_type: "user".into(),
                timezone: None,
                photo_url: None,
                initials: None,
                mention: Some(format!("agent{u}")),
                job_title: None,
                phone: None,
                alternate_emails: Vec::new(),
                created_at: None,
                updated_at: None,
            })
            .collect())
    }

    async fn list_teams(&self) -> crate::error::Result<Vec<crate::helpscout::HsTeam>> {
        Ok(vec![crate::helpscout::HsTeam {
            remote_id: 1,
            name: "Tier 1".into(),
            member_user_ids: (1..=N_USERS as i64).collect(),
        }])
    }

    async fn list_tags(&self) -> crate::error::Result<Vec<crate::helpscout::HsTag>> {
        Ok((0..N_TAGS)
            .map(|t| crate::helpscout::HsTag {
                remote_id: t as i64 + 1,
                name: format!("tag{t}"),
                slug: Some(format!("tag-{t}")),
                color: None,
                ticket_count: None,
                created_at: None,
                updated_at: None,
            })
            .collect())
    }

    async fn list_conversations(
        &self,
        query: &ConversationQuery,
    ) -> crate::error::Result<Page<HsConversation>> {
        let page_no: usize = query
            .cursor
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let offset = page_no * PAGE_SIZE;
        let items: Vec<HsConversation> = (offset..(offset + PAGE_SIZE).min(N_CONVERSATIONS))
            .map(sample_conversation)
            .collect();
        let next_cursor = if offset + PAGE_SIZE < N_CONVERSATIONS {
            Some((page_no + 1).to_string())
        } else {
            None
        };
        Ok(Page {
            items,
            next_cursor: next_cursor.map(|c| format!("/{c}")),
        })
    }

    async fn list_customers(
        &self,
        query: &CustomerQuery,
    ) -> crate::error::Result<Page<HsCustomer>> {
        let page_no: usize = query
            .cursor
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let offset = page_no * PAGE_SIZE;
        let items: Vec<HsCustomer> = (offset..(offset + PAGE_SIZE).min(N_CUSTOMERS))
            .map(sample_customer)
            .collect();
        let next_cursor = if offset + PAGE_SIZE < N_CUSTOMERS {
            Some((page_no + 1).to_string())
        } else {
            None
        };
        Ok(Page {
            items,
            next_cursor: next_cursor.map(|c| format!("/{c}")),
        })
    }

    async fn list_beacon_chats(&self) -> crate::error::Result<Vec<crate::helpscout::HsBeaconChat>> {
        Ok(Vec::new())
    }

    async fn list_docs(&self) -> crate::error::Result<Vec<crate::helpscout::HsDocArticle>> {
        Ok(Vec::new())
    }

    async fn list_ratings(&self) -> crate::error::Result<Vec<crate::helpscout::HsRating>> {
        Ok(Vec::new())
    }

    async fn list_threads(&self, conversation_id: i64) -> crate::error::Result<Vec<HsThread>> {
        Ok((0..THREADS_PER_CONVERSATION)
            .map(|j| sample_thread(conversation_id, j))
            .collect())
    }

    async fn create_reply_thread(
        &self,
        input: crate::helpscout::CreateThreadInput,
    ) -> crate::error::Result<crate::helpscout::ThreadCreated> {
        Ok(crate::helpscout::ThreadCreated {
            thread_id: 0,
            conversation_id: input.conversation_id,
        })
    }

    async fn create_note_thread(
        &self,
        input: crate::helpscout::CreateThreadInput,
    ) -> crate::error::Result<crate::helpscout::ThreadCreated> {
        Ok(crate::helpscout::ThreadCreated {
            thread_id: 0,
            conversation_id: input.conversation_id,
        })
    }

    async fn update_conversation(
        &self,
        _conversation_id: i64,
        _patch: crate::helpscout::ConversationPatch,
    ) -> crate::error::Result<bool> {
        Ok(true)
    }
}

/// Print a labeled timing and return elapsed + output.
fn timeit<T>(label: &str, f: impl FnOnce() -> T) -> (std::time::Duration, T) {
    let started = Instant::now();
    let out = f();
    let elapsed = started.elapsed();
    println!("[bench] {label}: {:.1?}", elapsed);
    (elapsed, out)
}

/// Seed the reference data + customers + conversations through the real
/// sync-engine write path and return the handle.
fn seeded_bench_db(dir: &std::path::Path) -> DbHandle {
    let db_path = dir.join("bench.db");
    let mut conn = crate::db::open(&db_path).expect("open bench db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    let db = DbHandle::new(conn);

    let engine = SyncEngine::new(db.clone(), Arc::new(LocalBenchProvider));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    // Reference data first (same order as the real initial sync).
    for resource in ["users", "teams", "mailboxes", "tags"] {
        let r = rt
            .block_on(async { engine.run_resource(resource, true).await })
            .expect("reference resource");
        assert!(
            r.error.is_none(),
            "reference resource {resource}: {:?}",
            r.error
        );
    }

    // Customers through the real ingest path.
    let (t_customers, r) = timeit("seed customers via sync write path (5000, paged)", || {
        rt.block_on(async { engine.run_resource("customers", true).await })
    });
    let r = r.expect("customers resource");
    assert!(r.error.is_none(), "customers: {:?}", r.error);
    assert_eq!(r.processed, N_CUSTOMERS as i64);
    println!(
        "[bench]   -> {:.0} customers/s through upsert_customer",
        N_CUSTOMERS as f64 / t_customers.as_secs_f64()
    );

    // Conversations + threads through the real ingest path.
    let (t_convs, r) = timeit(
        "seed conversations+threads via sync write path (3000 x 3)",
        || rt.block_on(async { engine.run_resource("conversations", true).await }),
    );
    let r = r.expect("conversations resource");
    assert!(r.error.is_none(), "conversations: {:?}", r.error);
    assert_eq!(r.processed, N_CONVERSATIONS as i64);
    println!(
        "[bench]   -> {:.0} conversations/s (incl. thread fetch + upsert + FTS + tags)",
        N_CONVERSATIONS as f64 / t_convs.as_secs_f64()
    );

    db
}

/// The full measurement suite (single `#[ignore]`d test).
#[test]
#[ignore = "performance measurement; run with --ignored --nocapture"]
fn bench_db_operations_on_large_dataset() {
    let dir = temp_bench_dir("db");
    let _guard = TempDirGuard(dir.clone());
    let db = seeded_bench_db(&dir);

    // Scale sanity before timing anything.
    let threads: i64 = db
        .with_read(|conn| {
            conn.query_row("SELECT COUNT(*) FROM conversation_threads", [], |r| {
                r.get(0)
            })
        })
        .unwrap_or(-1);
    assert_eq!(threads, (N_CONVERSATIONS * THREADS_PER_CONVERSATION) as i64);

    // --- Inbox queries (the UI's hot path) -------------------------------
    use crate::inbox::{InboxFilters, list_conversations};
    let (t, (items, total)) = timeit("inbox list page (50, unfiltered)", || {
        list_conversations(
            &db.write_guard(),
            &InboxFilters {
                limit: Some(50),
                ..Default::default()
            },
        )
        .expect("inbox list")
    });
    println!(
        "[bench]   -> {total} total conversations, page returned {} items",
        items.len()
    );
    assert_eq!(items.len(), 50);
    assert_eq!(total as usize, N_CONVERSATIONS);
    assert!(
        t.as_millis() < 5_000,
        "inbox page query must stay interactive: {t:?}"
    );

    let (t_filtered, _) = timeit("inbox list page (status=active filter)", || {
        list_conversations(
            &db.write_guard(),
            &InboxFilters {
                status: Some("active".into()),
                ..Default::default()
            },
        )
        .expect("filtered inbox list")
    });
    assert!(
        t_filtered.as_millis() < 5_000,
        "filtered page: {t_filtered:?}"
    );

    let (t_tag, (tag_items, _)) = timeit("inbox list page (tag filter)", || {
        list_conversations(
            &db.write_guard(),
            &InboxFilters {
                tag: Some("tag3".into()),
                ..Default::default()
            },
        )
        .expect("tag-filtered inbox list")
    });
    assert!(!tag_items.is_empty(), "tag filter must match rows");
    assert!(t_tag.as_millis() < 8_000, "tag filter is a join: {t_tag:?}");

    let (t_offset, _) = timeit("inbox list deep page (offset 2000)", || {
        list_conversations(
            &db.write_guard(),
            &InboxFilters {
                limit: Some(50),
                offset: Some(2000),
                ..Default::default()
            },
        )
        .expect("deep page")
    });
    assert!(t_offset.as_millis() < 5_000, "deep page: {t_offset:?}");

    // --- Conversation detail (thread + customer join) ---------------------
    let detail_local_id: i64 = db
        .with_read(|conn| {
            conn.query_row(
                "SELECT id FROM conversations WHERE remote_id = 100000",
                [],
                |r| r.get(0),
            )
        })
        .expect("local id for a seeded conversation");
    let (t_detail, detail) = timeit("inbox get_conversation (detail)", || {
        crate::inbox::get_conversation(&db.write_guard(), detail_local_id)
            .expect("conversation detail")
            .expect("seeded conversation present")
    });
    assert_eq!(detail.thread.len(), THREADS_PER_CONVERSATION);
    assert!(t_detail.as_millis() < 5_000, "detail query: {t_detail:?}");

    // --- FTS search -------------------------------------------------------
    let (t_fts, fts_hits) = timeit("FTS search 'login' (all scopes)", || {
        crate::search::search(
            &db.write_guard(),
            "login",
            "all",
            &crate::search::SearchFilters::default(),
        )
        .expect("fts search")
    });
    println!("[bench]   -> {} hits", fts_hits.total);
    assert!(fts_hits.total > 0, "seeded data must be searchable");
    assert!(t_fts.as_millis() < 5_000, "FTS search: {t_fts:?}");

    // --- Post-sync full-table pass ---------------------------------------
    let (t_backfill, n) = timeit("interaction_current::backfill_all", || {
        crate::interaction_current::backfill_all(&db.write_guard()).expect("backfill")
    });
    println!("[bench]   -> {n} conversation snapshots");
    assert!(
        t_backfill.as_secs() < 120,
        "backfill must not run away: {t_backfill:?}"
    );

    // --- Idempotent re-walk (incremental sync cost when nothing changed) --
    let engine = SyncEngine::new(db.clone(), Arc::new(LocalBenchProvider));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (t_rewalk, r) = timeit(
        "conversations re-walk, no changes (incremental path)",
        || rt.block_on(async { engine.run_resource("conversations", false).await }),
    );
    let r = r.expect("re-walk");
    assert!(r.error.is_none(), "re-walk: {:?}", r.error);
    assert_eq!(r.processed, N_CONVERSATIONS as i64);
    println!(
        "[bench]   -> re-walk at {:.0} conversations/s (idempotent upserts)",
        N_CONVERSATIONS as f64 / t_rewalk.as_secs_f64()
    );
    assert!(
        t_rewalk.as_secs() < 300,
        "re-walk must terminate: {t_rewalk:?}"
    );

    // --- Vector store -----------------------------------------------------
    let vec_dir = dir.join("vector");
    std::fs::create_dir_all(&vec_dir).expect("vec dir");
    let index = crate::vectorstore_sqlitevec::VectorIndex::new(vec_dir, true);
    assert!(index.ensure_collection(VECTOR_DIM), "collection created");

    let (t_upsert, ok) = timeit("vector upsert (3000 x 384, batches of 250)", || {
        let mut ok_all = true;
        for batch_start in (0..N_VECTOR_POINTS).step_by(250) {
            let points: Vec<crate::vectorstore_sqlitevec::VectorPoint> = (batch_start
                ..(batch_start + 250).min(N_VECTOR_POINTS))
                .map(|i| crate::vectorstore_sqlitevec::VectorPoint {
                    id: i as i64,
                    vector: pseudo_vec(i as u64, VECTOR_DIM),
                    payload: serde_json::json!({
                        "entity_type": "thread",
                        "entity_id": i,
                        "text": format!("chunk text {i} about login failures"),
                    }),
                })
                .collect();
            ok_all &= index.upsert(&points);
        }
        ok_all
    });
    assert!(ok, "vector upserts must succeed");
    println!(
        "[bench]   -> {:.0} points/s",
        N_VECTOR_POINTS as f64 / t_upsert.as_secs_f64()
    );
    assert!(t_upsert.as_secs() < 120, "vector upsert: {t_upsert:?}");

    let (t_knn, hits) = timeit("vector KNN search (top 20)", || {
        index.search(&pseudo_vec(42, VECTOR_DIM), 20)
    });
    assert_eq!(hits.len(), 20, "KNN must return the top-k");
    assert!(t_knn.as_millis() < 5_000, "vector KNN: {t_knn:?}");

    let (t_count, count) = timeit("vector count_by_entity", || index.count_by_entity("thread"));
    assert_eq!(count, N_VECTOR_POINTS);
    assert!(t_count.as_millis() < 5_000, "count: {t_count:?}");

    // --- Data scale on disk ------------------------------------------------
    // (Process max RSS is measured for the real binary separately with
    // /usr/bin/time -v — see completed.md for that procedure + numbers.)
    let db_bytes = std::fs::metadata(dir.join("bench.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    let wal_bytes = std::fs::metadata(dir.join("bench.db-wal"))
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "[bench] bench.db = {:.1} MiB (+{:.1} MiB WAL) for {N_CONVERSATIONS} conversations / \
         {} threads / {N_CUSTOMERS} customers",
        db_bytes as f64 / 1_048_576.0,
        wal_bytes as f64 / 1_048_576.0,
        N_CONVERSATIONS * THREADS_PER_CONVERSATION
    );
}

/// Boot-migration cost on a fresh file DB (the core side of startup time;
/// full process startup is measured against the real binary).
#[test]
#[ignore = "performance measurement; run with --ignored --nocapture"]
fn bench_boot_migrations_on_fresh_db() {
    let dir = temp_bench_dir("migrate");
    let _guard = TempDirGuard(dir.clone());
    let db_path = dir.join("fresh.db");
    let (t, _) = timeit("db::open + apply_all (fresh file DB)", || {
        let mut conn = crate::db::open(&db_path).expect("open");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
    });
    assert!(t.as_millis() < 2_000, "boot migrations must be fast: {t:?}");

    // Re-open an already-migrated DB (the normal every-boot path).
    let (t_reopen, _) = timeit("db::open on migrated DB (idempotent re-run)", || {
        let mut conn = crate::db::open(&db_path).expect("reopen");
        crate::bootstrap::apply_all(&mut conn).expect("idempotent migrations")
    });
    assert!(
        t_reopen <= t,
        "re-opening must not be slower than first boot: {t_reopen:?} vs {t:?}"
    );
}

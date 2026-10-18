//! Test support: an in-memory bootstrapped database plus deterministic seed
//! helpers used by the page model tests.
//!
//! The seeds write directly through the same tables the sync pipeline
//! fills (`conversations`, `conversation_threads`, `ratings`, …), so the
//! page models exercise their real queries.

#![allow(dead_code)] // test helpers are used inconsistently across modules

use spp_core::db::DbHandle;

/// Open an in-memory database with all boot migrations applied.
pub fn empty_db() -> DbHandle {
    let mut conn = rusqlite::Connection::open_in_memory().expect("in-memory db");
    spp_core::bootstrap::apply_all(&mut conn).expect("bootstrap");
    DbHandle::new(conn)
}

/// A conversation seed row:
/// `(marker, local_id, subject, status, channel, mailbox_id, assignee_id, customer_id)`.
pub type ConvSeed = (
    &'static str,
    i64,
    &'static str,
    &'static str,
    &'static str,
    i64,
    i64,
    i64,
);

/// Fixture database with users, mailboxes, customers and the given
/// conversations (all created "now", assigned, not deleted).
pub fn fixture(convs: Vec<ConvSeed>) -> DbHandle {
    let db = empty_db();
    db.with_write(|conn| {
        seed_users(conn);
        seed_mailboxes(conn);
        seed_customers(conn);
        for (_, id, subject, status, channel, mailbox, assignee, customer) in convs {
            seed_conversation(
                conn, id, subject, status, channel, mailbox, assignee, customer,
            );
        }
    });
    db
}

/// Two users: Ada (id 5, mention `ada`) and Grace (id 6).
pub fn seed_users(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO users (id, remote_id, first_name, last_name, email, mention)
         VALUES (5, 505, 'Ada', 'Lovelace', 'ada@example.com', 'ada'),
                (6, 506, 'Grace', 'Hopper', 'grace@example.com', NULL)",
        [],
    )
    .expect("users");
}

/// Two mailboxes: Support (1) and Billing (2).
pub fn seed_mailboxes(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO mailboxes (id, remote_id, name, email)
         VALUES (1, 101, 'Support', 'support@example.com'),
                (2, 102, 'Billing', 'billing@example.com')",
        [],
    )
    .expect("mailboxes");
}

/// Two customers: Kim (id 9) and Ada C. (id 10), each with a primary
/// `customer_emails` row (the store the segment engine and outreach
/// validation read).
pub fn seed_customers(conn: &rusqlite::Connection) {
    conn.execute(
        "INSERT INTO customers (id, remote_id, first_name, last_name, email)
         VALUES (9, 909, 'Kim', 'Pearce', 'kim@example.com'),
                (10, 910, 'Ada', 'Customer', 'adac@example.com')",
        [],
    )
    .expect("customers");
    conn.execute(
        "INSERT INTO customer_emails (customer_id, value, type)
         VALUES (9, 'kim@example.com', 'work'),
                (10, 'adac@example.com', 'work')",
        [],
    )
    .expect("customer emails");
}

/// Insert one conversation with now-relative timestamps (so day-range
/// queries always include the seeded rows). An `assignee_id` of 0 seeds
/// an unassigned conversation (NULL assignee, matching the operations
/// tiles' `assignee_local_id IS NULL` predicate).
#[allow(clippy::too_many_arguments)]
pub fn seed_conversation(
    conn: &rusqlite::Connection,
    id: i64,
    subject: &str,
    status: &str,
    channel: &str,
    mailbox_id: i64,
    assignee_id: i64,
    customer_id: i64,
) {
    conn.execute(
        "INSERT INTO conversations (id, remote_id, number, subject, preview, status, type,
             mailbox_local_id, assignee_local_id, customer_local_id, priority,
             created_at, updated_at, closed_at, first_customer_message_at, local_created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 CASE WHEN ?9 = 0 THEN NULL ELSE ?9 END,
                 ?10, 'none',
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 CASE WHEN ?11 = 'closed'
                      THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        rusqlite::params![
            id,
            1000 + id,
            500 + id,
            subject,
            format!("{subject} preview"),
            status,
            channel,
            mailbox_id,
            assignee_id,
            customer_id,
            status,
        ],
    )
    .expect("conversation");
}

/// Insert a thread entry for a conversation (now-relative timestamp).
pub fn seed_thread(conn: &rusqlite::Connection, conversation_id: i64, kind: &str, body: &str) {
    conn.execute(
        "INSERT INTO conversation_threads (conversation_id, type, body_text, from_type, state,
             created_by_customer_id, created_at)
         VALUES (?1, ?2, ?3, 'customer', 'published', 9,
                 strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        rusqlite::params![conversation_id, kind, body],
    )
    .expect("thread");
}

/// Insert a rating for a conversation (now-relative timestamp).
pub fn seed_rating(conn: &rusqlite::Connection, conversation_id: i64, rating: &str) {
    conn.execute(
        "INSERT INTO ratings (conversation_id, rating, remote_created_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        rusqlite::params![conversation_id, rating],
    )
    .expect("rating");
}

/// Insert a tag and attach it to a conversation.
pub fn seed_tag(conn: &rusqlite::Connection, conversation_id: i64, name: &str) {
    let tag_id: i64 = conn
        .query_row(
            "INSERT INTO tags (remote_id, name, slug) VALUES ((SELECT COALESCE(MAX(remote_id), 0) + 1 FROM tags), ?1, ?1) RETURNING id",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .expect("tag insert");
    conn.execute(
        "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
        rusqlite::params![conversation_id, tag_id],
    )
    .expect("conversation tag");
}

/// Insert a help-center collection + article; returns the article id.
pub fn seed_doc_article(
    conn: &rusqlite::Connection,
    collection_id: i64,
    name: &str,
    status: &str,
    text: &str,
) -> i64 {
    conn.execute(
        "INSERT OR IGNORE INTO docs_collections (id, remote_id, name, slug)
         VALUES (?1, ?1, 'Guides', 'guides')",
        rusqlite::params![collection_id],
    )
    .expect("collection");
    let id: i64 = conn
        .query_row(
            "INSERT INTO docs_articles (collection_local_id, number, name, status, text, preview,
                 views, words, remote_created_at, remote_updated_at)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, 0, 0,
                     '2026-01-10T00:00:00.000Z', '2026-01-10T00:00:00.000Z')
             RETURNING id",
            rusqlite::params![
                collection_id,
                name,
                status,
                text,
                &text[..text.len().min(80)]
            ],
            |r| r.get(0),
        )
        .expect("article insert");
    let _ = spp_core::search::rebuild_indexes(conn);
    id
}

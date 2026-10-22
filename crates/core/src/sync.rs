use rusqlite::{Connection, params};

use crate::error::Result;
use crate::helpscout::{HsConversation, HsCustomer, HsMailbox, HsTag, HsTeam, HsUser};

pub fn upsert_mailbox(conn: &Connection, m: &HsMailbox) -> Result<()> {
    conn.execute(
        "INSERT INTO mailboxes (remote_id, name, slug, email, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(remote_id) DO UPDATE SET
            name = excluded.name,
            slug = excluded.slug,
            email = excluded.email,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            m.remote_id,
            m.name,
            m.slug,
            m.email,
            m.created_at,
            m.updated_at
        ],
    )?;
    Ok(())
}

pub fn upsert_user(conn: &Connection, u: &HsUser) -> Result<()> {
    conn.execute(
        "INSERT INTO users (remote_id, first_name, last_name, email, role, user_type, timezone,
            photo_url, initials, mention, job_title, phone, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(remote_id) DO UPDATE SET
            first_name = excluded.first_name,
            last_name = excluded.last_name,
            email = excluded.email,
            role = excluded.role,
            user_type = excluded.user_type,
            timezone = excluded.timezone,
            photo_url = excluded.photo_url,
            initials = excluded.initials,
            mention = excluded.mention,
            job_title = excluded.job_title,
            phone = excluded.phone,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            u.remote_id,
            u.first_name,
            u.last_name,
            u.email,
            u.role,
            u.user_type,
            u.timezone,
            u.photo_url,
            u.initials,
            u.mention,
            u.job_title,
            u.phone,
            u.created_at,
            u.updated_at,
        ],
    )?;
    Ok(())
}

pub fn upsert_team(conn: &Connection, t: &HsTeam) -> Result<()> {
    conn.execute(
        "INSERT INTO teams (remote_id, name)
         VALUES (?1, ?2)
         ON CONFLICT(remote_id) DO UPDATE SET name = excluded.name",
        params![t.remote_id, t.name],
    )?;
    let team_local: Option<i64> = conn
        .query_row(
            "SELECT id FROM teams WHERE remote_id = ?1",
            params![t.remote_id],
            |r| r.get(0),
        )
        .ok();
    if let Some(team_local) = team_local {
        if !crate::sync_schema::table_exists(conn, "team_members").unwrap_or(false) {
            return Ok(());
        }
        conn.execute(
            "DELETE FROM team_members WHERE team_id = ?1",
            params![team_local],
        )?;
        for member_remote in &t.member_user_ids {
            if let Some(user_local) = crate::sync_engine::local_id(conn, "users", *member_remote) {
                conn.execute(
                    "INSERT OR IGNORE INTO team_members (team_id, user_id) VALUES (?1, ?2)",
                    params![team_local, user_local],
                )?;
            }
        }
    }
    Ok(())
}

pub fn upsert_tag(conn: &Connection, t: &HsTag) -> Result<()> {
    conn.execute(
        "INSERT INTO tags (remote_id, name, slug, color, ticket_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(remote_id) DO UPDATE SET
            name = excluded.name,
            slug = excluded.slug,
            color = excluded.color,
            ticket_count = excluded.ticket_count,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            t.remote_id,
            t.name,
            t.slug,
            t.color,
            t.ticket_count,
            t.created_at,
            t.updated_at,
        ],
    )?;
    Ok(())
}

pub fn upsert_conversation(conn: &Connection, c: &HsConversation) -> Result<()> {
    let mailbox_local = crate::sync_engine::local_id(conn, "mailboxes", c.mailbox_id);
    let customer_local = if c.customer_id > 0 {
        crate::sync_engine::local_id(conn, "customers", c.customer_id)
    } else {
        None
    };
    let assignee_local = c
        .assignee_id
        .filter(|rid| *rid > 0)
        .and_then(|rid| crate::sync_engine::local_id(conn, "users", rid));
    struct PreviousRow {
        assignee: Option<i64>,
        status: Option<String>,
        priority: Option<String>,
        customer: Option<i64>,
        mailbox: Option<i64>,
        snoozed_until: Option<String>,
        thread_count: Option<i64>,
    }
    let previous: Option<PreviousRow> = conn
        .query_row(
            "SELECT assignee_local_id, status, priority, customer_local_id,
                    mailbox_local_id, snoozed_until, thread_count
             FROM conversations WHERE remote_id = ?1",
            params![c.remote_id],
            |r| {
                Ok(PreviousRow {
                    assignee: r.get(0)?,
                    status: r.get(1)?,
                    priority: r.get(2)?,
                    customer: r.get(3)?,
                    mailbox: r.get(4)?,
                    snoozed_until: r.get(5)?,
                    thread_count: r.get(6)?,
                })
            },
        )
        .ok();
    conn.execute(
        "INSERT INTO conversations (remote_id, number, subject, preview, status, state, type,
            source_type, source_via, mailbox_local_id, assignee_local_id, customer_local_id,
            priority, created_at,
            updated_at, closed_at, snoozed_until, thread_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
         ON CONFLICT(remote_id) DO UPDATE SET
            number = excluded.number,
            subject = excluded.subject,
            preview = excluded.preview,
            status = excluded.status,
            state = excluded.state,
            type = excluded.type,
            source_type = excluded.source_type,
            source_via = excluded.source_via,
            mailbox_local_id = excluded.mailbox_local_id,
            assignee_local_id = excluded.assignee_local_id,
            customer_local_id = excluded.customer_local_id,
            priority = excluded.priority,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at,
            closed_at = excluded.closed_at,
            snoozed_until = excluded.snoozed_until,
            thread_count = excluded.thread_count,
            -- DB-05 (M18): resurrect on upsert — a soft-deleted row that
            -- reappears remotely (undelete, restore, re-merge-out target)
            -- comes back to life (conversationRepo.ts:136 `deleted_at=NULL`).
            deleted_at = NULL",
        params![
            c.remote_id,
            c.number,
            c.subject,
            c.preview,
            c.status,
            c.state,
            c.kind,
            c.source_type,
            c.source_via,
            mailbox_local,
            assignee_local,
            customer_local,
            c.priority,
            c.created_at,
            c.updated_at,
            c.closed_at,
            c.snoozed_until,
            c.thread_count,
        ],
    )?;
    let local: Option<i64> = conn
        .query_row(
            "SELECT id FROM conversations WHERE remote_id = ?1",
            params![c.remote_id],
            |r| r.get(0),
        )
        .ok();
    // Clean-audit item 29: read the pre-write tag set so tags_changed can
    // carry a real previous value — it used to ride with previous=null and
    // fired on every re-ingest, tags unchanged or not.
    let previous_tags: Option<Vec<String>> = match local {
        Some(conv_local) => {
            let tags = conversation_tag_names(conn, conv_local)?;
            crate::conversation_ops::write_conversation_tags(conn, conv_local, &c.tags);
            Some(tags)
        }
        None => None,
    };
    if let (Some(prev), Some(conv_local), Some(prev_tags)) = (previous, local, previous_tags) {
        let observed_at = crate::activity::now_iso();
        let emit_diff = |event_type: &str,
                         prev_val: &serde_json::Value,
                         next_val: &serde_json::Value|
         -> Result<()> {
            if prev_val != next_val {
                crate::activity::record_full_event(
                    conn,
                    &crate::activity::FullActivityEvent {
                        base: crate::activity::ActivityEvent {
                            id: None,
                            conversation_id: conv_local,
                            event_type: event_type.into(),
                            actor_type: "user".into(),
                            actor_id: None,
                            occurred_at: observed_at.clone(),
                            dedup_key: format!("{event_type}:{}:{observed_at}", c.remote_id),
                        },
                        thread_local_id: None,
                        source: "sync".into(),
                        metadata: Some(
                            serde_json::json!({
                                "previous": prev_val,
                                "next": next_val,
                                "observed": true
                            })
                            .to_string(),
                        ),
                    },
                )?;
            }
            Ok(())
        };
        emit_diff(
            "assignment_changed",
            &serde_json::json!(prev.assignee),
            &serde_json::json!(assignee_local),
        )?;
        emit_diff(
            "status_changed",
            &serde_json::json!(prev.status),
            &serde_json::json!(c.status),
        )?;
        emit_diff(
            "priority_changed",
            &serde_json::json!(prev.priority),
            &serde_json::json!(c.priority),
        )?;
        emit_diff(
            "customer_changed",
            &serde_json::json!(prev.customer),
            &serde_json::json!(customer_local),
        )?;
        emit_diff(
            "mailbox_changed",
            &serde_json::json!(prev.mailbox),
            &serde_json::json!(mailbox_local),
        )?;
        emit_diff(
            "snooze_changed",
            &serde_json::json!(prev.snoozed_until),
            &serde_json::json!(c.snoozed_until),
        )?;
        emit_diff(
            "thread_count_changed",
            &serde_json::json!(prev.thread_count),
            &serde_json::json!(c.thread_count),
        )?;
        // Item 29: the tag comparison uses normalized sets (sorted +
        // deduplicated), so a pure reorder of the platform's tag array is
        // not an event — only membership changes are.
        emit_diff(
            "tags_changed",
            &serde_json::json!(normalized_tag_set(&prev_tags)),
            &serde_json::json!(normalized_tag_set(&c.tags)),
        )?;
    }
    Ok(())
}

/// The mirror's current tag names for one conversation (item 29: the
/// previous state for the tags_changed diff).
fn conversation_tag_names(conn: &Connection, conversation_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM conversation_tags ct
           JOIN tags t ON t.id = ct.tag_id
          WHERE ct.conversation_id = ?1",
    )?;
    let names = stmt
        .query_map(params![conversation_id], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(names)
}

/// Sort + dedupe a tag list so set comparisons ignore the platform's
/// ordering and repeated names.
fn normalized_tag_set(tags: &[String]) -> Vec<String> {
    let mut set = tags.to_vec();
    set.sort();
    set.dedup();
    set
}

pub fn upsert_customer(conn: &Connection, c: &HsCustomer) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO customers (remote_id, first_name, last_name, email, organization, job_title,
            phone, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(remote_id) DO UPDATE SET
            first_name = excluded.first_name,
            last_name = excluded.last_name,
            email = excluded.email,
            organization = excluded.organization,
            job_title = excluded.job_title,
            phone = excluded.phone,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at",
        params![
            c.remote_id,
            c.first_name,
            c.last_name,
            c.email,
            c.organization,
            c.job_title,
            c.phone,
            c.created_at,
            c.updated_at,
        ],
    )?;
    let local: i64 = tx.query_row(
        "SELECT id FROM customers WHERE remote_id = ?1",
        params![c.remote_id],
        |r| r.get(0),
    )?;
    let replace_set =
        |delete_sql: &str, insert_sql: &str, rows: Vec<Vec<Box<dyn rusqlite::ToSql>>>| {
            tx.execute(delete_sql, params![local])?;
            for row in rows {
                let mut bind: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(row.len() + 1);
                bind.push(&local);
                for v in &row {
                    bind.push(v.as_ref());
                }
                tx.execute(insert_sql, bind.as_slice())?;
            }
            crate::error::Result::Ok(())
        };
    replace_set(
        "DELETE FROM customer_emails WHERE customer_id = ?1",
        "INSERT INTO customer_emails (customer_id, value, type) VALUES (?1, ?2, ?3)",
        c.emails
            .iter()
            .filter_map(|e| {
                e.value
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        vec![
                            Box::new(v.trim().to_string()) as Box<dyn rusqlite::ToSql>,
                            Box::new(e.kind.clone()),
                        ]
                    })
            })
            .collect(),
    )?;
    replace_set(
        "DELETE FROM customer_phones WHERE customer_id = ?1",
        "INSERT INTO customer_phones (customer_id, value, type) VALUES (?1, ?2, ?3)",
        c.phones
            .iter()
            .filter_map(|p| {
                p.value
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        vec![
                            Box::new(v.trim().to_string()) as Box<dyn rusqlite::ToSql>,
                            Box::new(p.kind.clone()),
                        ]
                    })
            })
            .collect(),
    )?;
    replace_set(
        "DELETE FROM customer_websites WHERE customer_id = ?1",
        "INSERT INTO customer_websites (customer_id, value) VALUES (?1, ?2)",
        c.websites
            .iter()
            .filter_map(|w| {
                w.value
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| vec![Box::new(v.trim().to_string()) as Box<dyn rusqlite::ToSql>])
            })
            .collect(),
    )?;
    replace_set(
        "DELETE FROM customer_social_profiles WHERE customer_id = ?1",
        "INSERT INTO customer_social_profiles (customer_id, value, type) VALUES (?1, ?2, ?3)",
        c.social_profiles
            .iter()
            .filter_map(|s| {
                s.value
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        vec![
                            Box::new(v.trim().to_string()) as Box<dyn rusqlite::ToSql>,
                            Box::new(s.kind.clone()),
                        ]
                    })
            })
            .collect(),
    )?;
    tx.execute(
        "DELETE FROM customer_addresses WHERE customer_id = ?1",
        params![local],
    )?;
    if let Some(a) = &c.address {
        let lines = [a.line1.as_deref(), a.line2.as_deref()]
            .into_iter()
            .flatten()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        tx.execute(
            "INSERT INTO customer_addresses (customer_id, lines, city, state, postal_code, country)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                local,
                if lines.is_empty() { None } else { Some(lines) },
                a.city,
                a.state,
                a.postal_code,
                a.country,
            ],
        )?;
    }
    for p in &c.properties {
        let value = p.value.as_deref().filter(|v| !v.trim().is_empty());
        if value.is_none() || p.definition_remote_id.is_none() {
            continue;
        }
        let def_remote = p.definition_remote_id.unwrap_or(0);
        let def_local: Option<i64> = tx
            .query_row(
                "SELECT id FROM customer_property_definitions WHERE remote_id = ?1",
                params![def_remote],
                |r| r.get(0),
            )
            .ok();
        let def_local = match def_local {
            Some(id) => id,
            None => {
                let name = p
                    .name
                    .clone()
                    .or_else(|| p.key.clone())
                    .unwrap_or_else(|| format!("Property {def_remote}"));
                tx.execute(
                    "INSERT INTO customer_property_definitions (remote_id, name, slug, type, sort_order, last_synced_at)
                     VALUES (?1, ?2, ?3, 'text', 0, datetime('now'))
                     ON CONFLICT(remote_id) DO UPDATE SET name = excluded.name",
                    params![def_remote, name, p.key],
                )?;
                tx.query_row(
                    "SELECT id FROM customer_property_definitions WHERE remote_id = ?1",
                    params![def_remote],
                    |r| r.get(0),
                )?
            }
        };
        tx.execute(
            "INSERT INTO customer_properties (customer_id, definition_id, value)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(customer_id, definition_id) DO UPDATE SET value = excluded.value",
            params![local, def_local, value],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod batch21_audit29_tests {
    use super::*;
    use crate::helpscout::HsConversation;

    fn conn() -> Connection {
        let mut conn = Connection::open_in_memory().expect("memory db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn conv(tags: &[&str]) -> HsConversation {
        HsConversation {
            remote_id: 101,
            number: 778,
            status: "active".into(),
            mailbox_id: 1,
            customer_id: 900,
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            ..Default::default()
        }
    }

    fn tags_events(conn: &Connection) -> Vec<serde_json::Value> {
        let mut stmt = conn
            .prepare("SELECT metadata FROM activity_events WHERE event_type = 'tags_changed'")
            .expect("prepare");
        stmt.query_map([], |r| r.get::<_, String>(0))
            .expect("query")
            .filter_map(std::result::Result::ok)
            .map(|m| serde_json::from_str(&m).expect("metadata json"))
            .collect()
    }

    /// Audit item 29: `tags_changed` rode with previous=null and fired on
    /// every re-ingest — tags unchanged or not. It now carries the mirror's
    /// real pre-write tag set (as a normalized set, so reorders are not
    /// events) and fires only on membership changes.
    #[test]
    fn audit29_tags_changed_fires_only_when_the_tag_set_changes() {
        let conn = conn();

        // First ingest: no previous row, so no diff events at all.
        upsert_conversation(&conn, &conv(&["login", "urgent"])).expect("first ingest");
        assert!(
            tags_events(&conn).is_empty(),
            "the first ingest must not emit tags_changed"
        );

        // Re-ingest with the same tags in a different order: a reorder is
        // not a change — no event.
        upsert_conversation(&conn, &conv(&["urgent", "login"])).expect("reordered re-ingest");
        assert!(
            tags_events(&conn).is_empty(),
            "an unchanged (reordered) tag set must not emit tags_changed"
        );

        // A real membership change fires exactly one event with the real
        // previous state instead of null.
        upsert_conversation(&conn, &conv(&["login"])).expect("changed tags");
        let events = tags_events(&conn);
        assert_eq!(events.len(), 1, "one membership change, one event");
        assert_eq!(
            events[0]["previous"],
            serde_json::json!(["login", "urgent"]),
            "previous must be the mirror's pre-write tag set"
        );
        assert_eq!(events[0]["next"], serde_json::json!(["login"]));

        // Re-ingesting the new set again stays silent.
        upsert_conversation(&conn, &conv(&["login"])).expect("stable re-ingest");
        assert_eq!(
            tags_events(&conn).len(),
            1,
            "a stable tag set must not emit more events"
        );
    }

    /// The normalized comparison also dedupes repeated names, so a
    /// platform payload repeating a tag is not a change either.
    #[test]
    fn audit29_repeated_tag_names_are_not_a_change() {
        let conn = conn();
        upsert_conversation(&conn, &conv(&["vip"])).expect("first ingest");
        upsert_conversation(&conn, &conv(&["vip", "vip"])).expect("duplicated tag payload");
        assert!(
            tags_events(&conn).is_empty(),
            "repeated names are the same tag set"
        );
    }
}

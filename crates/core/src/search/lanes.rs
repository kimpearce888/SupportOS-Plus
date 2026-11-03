use super::*;

pub fn search_conversations(
    conn: &Connection,
    query: &str,
    filters: &SearchFilters,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    let fts = fts_query(query);
    if fts == "\"\"" && !has_filters(filters) {
        return recent_conversations(conn, limit);
    }

    let mut where_sql = vec![
        "c.deleted_at IS NULL".to_string(),
        "c.merged_into_conversation_id IS NULL".to_string(),
    ];
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();

    if status_applies(filters) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.status = ?{idx}"));
        args.push(Box::new(filters.status.clone().unwrap_or_default()));
    }
    if let Some(mailbox) = filters.mailbox_id.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.mailbox_local_id = ?{idx}"));
        args.push(Box::new(mailbox));
    }
    if let Some(tag) = filters.tag.clone().filter(|t| !t.is_empty()) {
        let idx = args.len() + 1;
        where_sql.push(format!(
            "EXISTS (SELECT 1 FROM conversation_tags ct JOIN tags t ON t.id = ct.tag_id \
             WHERE ct.conversation_id = c.id AND t.name = ?{idx} COLLATE NOCASE)"
        ));
        args.push(Box::new(tag));
    }
    if let Some(days) = filters.since_days.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!(
            "julianday(c.created_at) >= julianday('now', '-' || ?{idx} || ' days')"
        ));
        args.push(Box::new(days));
    }
    if let Some(assignee) = filters.assignee_id.filter(|v| *v != 0) {
        let idx = args.len() + 1;
        where_sql.push(format!("c.assignee_local_id = ?{idx}"));
        args.push(Box::new(assignee));
    }
    let trimmed = query.trim();
    if !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_digit())
        && let Ok(num) = trimmed.parse::<i64>()
    {
        let idx = args.len() + 1;
        where_sql.push(format!("c.number = ?{idx}"));
        args.push(Box::new(num));
    }

    let mut join_fts = String::new();
    if fts != "\"\"" {
        let idx = args.len() + 1;
        join_fts = format!(
            "JOIN (
                SELECT conversation_id, MIN(rank) AS r FROM (
                    SELECT conversation_id, rank FROM fts_conversations WHERE fts_conversations MATCH ?{idx}
                    UNION ALL
                    SELECT f.conversation_id, rank FROM fts_threads f WHERE fts_threads MATCH ?{idx}
                    UNION ALL
                    SELECT c2.id, rank FROM fts_ai_analyses a JOIN conversations c2 ON c2.id = a.conversation_id WHERE fts_ai_analyses MATCH ?{idx}
                ) GROUP BY conversation_id
            ) m ON m.conversation_id = c.id"
        );
        args.push(Box::new(fts));
    }
    let rank_expr = if join_fts.is_empty() {
        "0"
    } else {
        "COALESCE(m.r, 999999)"
    };
    let limit_idx = args.len() + 1;
    args.push(Box::new(limit));

    let sql = format!(
        "SELECT c.id, c.number, c.subject, c.preview, c.status, c.created_at,
           TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')) AS customer,
           COALESCE((SELECT ce.value FROM customer_emails ce WHERE ce.customer_id = cu.id LIMIT 1), '') AS customer_email,
           {rank_expr} AS rank
         FROM conversations c
         LEFT JOIN customers cu ON cu.id = c.customer_local_id
         {join_fts}
         WHERE {}
         ORDER BY rank ASC, c.created_at DESC
         LIMIT ?{limit_idx}",
        where_sql.join(" AND ")
    );

    let mut stmt = conn.prepare(&sql)?;
    let arg_refs: Vec<&dyn ToSql> = args.iter().map(Box::as_ref).collect();
    let rows = stmt
        .query_map(arg_refs.as_slice(), |r| {
            let id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            let subject: Option<String> = r.get(2)?;
            Ok(SearchHit {
                scope: "tickets".into(),
                id,
                title: format!(
                    "#{number} {}",
                    subject.unwrap_or_else(|| "(no subject)".into())
                ),
                subtitle: [
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, String>(4)?,
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · "),
                snippet: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                score: 1.0,
                href: format!("/inbox/conversation/{id}"),
                why: vec!["keyword match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn recent_conversations(conn: &Connection, limit: i64) -> Result<Vec<SearchHit>> {
    let mut stmt = conn.prepare(
        "SELECT id, number, subject, preview, status
           FROM conversations
          WHERE deleted_at IS NULL AND merged_into_conversation_id IS NULL
          ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit], |r| {
            let id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            Ok(SearchHit {
                scope: "tickets".into(),
                id,
                title: format!(
                    "#{number} {}",
                    r.get::<_, Option<String>>(2)?
                        .unwrap_or_else(|| "(no subject)".into())
                ),
                subtitle: r.get(4)?,
                snippet: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                score: 0.5,
                href: format!("/inbox/conversation/{id}"),
                why: vec!["recent".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn status_applies(f: &SearchFilters) -> bool {
    matches!(f.status.as_deref(), Some(s) if !s.is_empty() && s != "all")
}

fn has_filters(f: &SearchFilters) -> bool {
    status_applies(f)
        || f.mailbox_id.is_some_and(|v| v != 0)
        || f.tag.as_deref().is_some_and(|t| !t.is_empty())
        || f.since_days.is_some_and(|v| v != 0)
        || f.assignee_id.is_some_and(|v| v != 0)
}

pub(crate) fn search_customers(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let like = format!("%{}%", escape_like(query));
    let mut stmt = conn.prepare(
        "SELECT c.id, c.first_name, c.last_name,
           (SELECT ce.value FROM customer_emails ce WHERE ce.customer_id = c.id LIMIT 1) AS email,
           o.name AS org,
           (SELECT COUNT(*) FROM conversations cv WHERE cv.customer_local_id = c.id) AS conv_count
         FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id
         WHERE c.deleted_at IS NULL AND (
           c.first_name LIKE ?1 ESCAPE '\\' OR c.last_name LIKE ?1 ESCAPE '\\' OR
           EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.id AND ce.value LIKE ?1 ESCAPE '\\')
         )
         ORDER BY c.last_name LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![like, limit], |r| {
            let id: i64 = r.get(0)?;
            let first: Option<String> = r.get(1)?;
            let last: Option<String> = r.get(2)?;
            let email: Option<String> = r.get(3)?;
            let org: Option<String> = r.get(4)?;
            let conv_count: i64 = r.get(5)?;
            let title = [first, last]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            Ok(SearchHit {
                scope: "customers".into(),
                id,
                title: if title.is_empty() {
                    "Unknown".into()
                } else {
                    title
                },
                subtitle: email.unwrap_or_default(),
                snippet: match org.filter(|o| !o.is_empty()) {
                    Some(org) => format!("{org} · {conv_count} conversations"),
                    None => format!("{conv_count} conversations"),
                },
                score: 1.0,
                href: format!("/customers/{id}"),
                why: vec!["name/email match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[allow(clippy::ref_option)]
pub(crate) fn search_knowledge_raw(
    conn: &Connection,
    query: &str,
    visibility: Option<&str>,
    limit: i64,
    mode: &str,
) -> Result<Vec<KnowledgeRawHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let fts = if mode == "or" {
        fts_query_or(query, 10)
    } else {
        fts_query(query)
    };
    let vis_sql = visibility.map_or("", |_| "AND f.visibility = ?2");
    let sql = format!(
        "SELECT f.document_id, f.title, snippet(fts_knowledge, 1, '[', ']', '…', 14) AS snippet, f.visibility
           FROM fts_knowledge f WHERE fts_knowledge MATCH ?1 {vis_sql} ORDER BY rank"
    );
    let limit = limit.max(0) as usize;
    let mut stmt = conn.prepare(&sql)?;
    let map_row = |r: &rusqlite::Row<'_>| -> rusqlite::Result<KnowledgeRawHit> {
        Ok(KnowledgeRawHit {
            document_id: r.get(0)?,
            title: r.get(1)?,
            snippet: r.get(2)?,
            visibility: r.get(3)?,
        })
    };
    let rows = match visibility {
        Some(v) => stmt.query_map(params![fts, v], map_row)?,
        None => stmt.query_map(params![fts], map_row)?,
    };
    let mut hits: Vec<KnowledgeRawHit> = Vec::new();
    let mut seen_documents: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for row in rows {
        let hit = row?;
        if seen_documents.insert(hit.document_id) {
            hits.push(hit);
            if hits.len() >= limit {
                break;
            }
        }
    }
    Ok(hits)
}

pub fn search_knowledge(
    conn: &Connection,
    query: &str,
    visibility: Option<&str>,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    Ok(search_knowledge_raw(conn, query, visibility, limit, "and")?
        .into_iter()
        .map(|r| SearchHit {
            scope: "knowledge".into(),
            id: r.document_id,
            title: r.title,
            subtitle: format!(
                "Knowledge · {}",
                if r.visibility == "customer_safe" {
                    "customer-safe"
                } else {
                    "internal"
                }
            ),
            snippet: r.snippet,
            score: 1.0,
            href: format!("/knowledge?doc={}", r.document_id),
            why: vec!["knowledge match".into()],
        })
        .collect())
}

pub fn search_known_issues(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !table_exists(conn, "known_issues")? {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT ki.id, ki.title, snippet(fts_known_issues, 0, '[', ']', '…', 14) AS snippet
           FROM fts_known_issues f JOIN known_issues ki ON ki.id = f.known_issue_id
          WHERE fts_known_issues MATCH ?1 ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            Ok(SearchHit {
                scope: "issues".into(),
                id: r.get(0)?,
                title: r.get(1)?,
                subtitle: "Known issue".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: "/issues?tab=known".into(),
                why: vec!["known issue match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn search_saved_replies(conn: &Connection, query: &str, limit: i64) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !table_exists(conn, "saved_replies")? {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT sr.id, sr.name, snippet(fts_saved_replies, 1, '[', ']', '…', 14) AS snippet
           FROM fts_saved_replies f JOIN saved_replies sr ON sr.id = f.saved_reply_id
          WHERE fts_saved_replies MATCH ?1 AND sr.deleted_at IS NULL ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            Ok(SearchHit {
                scope: "saved_replies".into(),
                id: r.get(0)?,
                title: r.get(1)?,
                subtitle: "Saved reply".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: "/inbox".into(),
                why: vec!["saved reply match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn search_ai_analyses(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let fts = fts_query(query);
    let mut stmt = conn.prepare(
        "SELECT a.conversation_id, c.number, snippet(fts_ai_analyses, 0, '[', ']', '…', 14) AS snippet, c.subject
           FROM fts_ai_analyses a JOIN conversations c ON c.id = a.conversation_id
          WHERE fts_ai_analyses MATCH ?1 AND c.deleted_at IS NULL ORDER BY rank LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![fts, limit], |r| {
            let conversation_id: i64 = r.get(0)?;
            let number: i64 = r.get(1)?;
            let subject: Option<String> = r.get(3)?;
            Ok(SearchHit {
                scope: "ai".into(),
                id: conversation_id,
                title: format!("AI analysis · #{number} {}", subject.unwrap_or_default()),
                subtitle: "AI-derived (marked as AI-generated)".into(),
                snippet: r.get(2)?,
                score: 1.0,
                href: format!("/inbox/conversation/{conversation_id}"),
                why: vec!["AI analysis match".into()],
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

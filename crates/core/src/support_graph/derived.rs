use super::*;

pub(super) struct DerivedEdge {
    pub(super) relation: &'static str,
    pub(super) origin: &'static str,
    pub(super) source_kind: GraphNodeKind,
    pub(super) source_id: i64,
    pub(super) target_kind: GraphNodeKind,
    pub(super) target_id: i64,
    pub(super) note: Option<String>,
    pub(super) at: Option<String>,
}

pub(super) fn derived_edges_touching(
    conn: &Connection,
    kind: GraphNodeKind,
    local_id: i64,
) -> Result<Vec<DerivedEdge>> {
    const BRANCH_CAP: i64 = GRAPH_MAX_NEIGHBOR_EDGES as i64;
    let mut out: Vec<DerivedEdge> = Vec::new();
    let mut push = |e: DerivedEdge| out.push(e);

    let one_i64 = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Option<i64> {
        conn.query_row(sql, p, |r| r.get(0)).ok()
    };
    let many_i64 = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Vec<i64> {
        crate::db::query_rows(conn, sql, p, |r| r.get::<_, i64>(0)).unwrap_or_default()
    };
    let product_id_by_name = |name: &str| -> Option<i64> {
        conn.query_row(
            "SELECT id FROM products WHERE name = ?1 COLLATE NOCASE",
            params![name],
            |r| r.get(0),
        )
        .ok()
    };
    let mirror = |relation: &'static str,
                  source_kind: GraphNodeKind,
                  source_id: i64,
                  target_kind: GraphNodeKind,
                  target_id: i64| {
        DerivedEdge {
            relation,
            origin: "helpscout_mirror",
            source_kind,
            source_id,
            target_kind,
            target_id,
            note: None,
            at: None,
        }
    };

    match kind {
        GraphNodeKind::Customer => {
            if let Some(org) = one_i64(
                "SELECT organization_id FROM customers
                 WHERE id = ?1 AND deleted_at IS NULL AND organization_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror(
                    "belongs_to",
                    kind,
                    local_id,
                    GraphNodeKind::Organization,
                    org,
                ));
            }
            for conv in many_i64(
                "SELECT id FROM conversations WHERE customer_local_id = ?1 AND deleted_at IS NULL
                 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "involves",
                    GraphNodeKind::Conversation,
                    conv,
                    kind,
                    local_id,
                ));
            }
            for campaign in many_i64(
                "SELECT campaign_id FROM outreach_recipients
                 WHERE customer_local_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "sent_to",
                    GraphNodeKind::Campaign,
                    campaign,
                    kind,
                    local_id,
                ));
            }
        }
        GraphNodeKind::Organization => {
            for cust in many_i64(
                "SELECT id FROM customers WHERE organization_id = ?1 AND deleted_at IS NULL
                 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "belongs_to",
                    GraphNodeKind::Customer,
                    cust,
                    kind,
                    local_id,
                ));
            }
        }
        GraphNodeKind::Conversation => {
            if let Some(cust) = one_i64(
                "SELECT customer_local_id FROM conversations
                 WHERE id = ?1 AND deleted_at IS NULL AND customer_local_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror(
                    "involves",
                    kind,
                    local_id,
                    GraphNodeKind::Customer,
                    cust,
                ));
            }
            if let Some(agent) = one_i64(
                "SELECT assignee_local_id FROM conversations
                 WHERE id = ?1 AND deleted_at IS NULL AND assignee_local_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror(
                    "assigned_to",
                    kind,
                    local_id,
                    GraphNodeKind::Agent,
                    agent,
                ));
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT known_issue_id, link_type FROM known_issue_links
                     WHERE conversation_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })?;
                for row in rows {
                    let (issue, link_type) = row?;
                    let origin = match link_type.as_deref() {
                        Some("human") => "human_local",
                        _ => "ai_derived",
                    };
                    push(DerivedEdge {
                        relation: "linked_to_issue",
                        origin,
                        source_kind: kind,
                        source_id: local_id,
                        target_kind: GraphNodeKind::KnownIssue,
                        target_id: issue,
                        note: None,
                        at: None,
                    });
                }
            }
            for cluster in many_i64(
                "SELECT cluster_id FROM issue_cluster_conversations WHERE conversation_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "clustered_into",
                    origin: "deterministic_local",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::IssueCluster,
                    target_id: cluster,
                    note: None,
                    at: None,
                });
            }
            for incident in many_i64(
                "SELECT incident_id FROM incident_conversations WHERE conversation_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "affected_by",
                    origin: "deterministic_local",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::Incident,
                    target_id: incident,
                    note: None,
                    at: None,
                });
            }
            for doc in many_i64(
                "SELECT s.source_id FROM ai_sources s
                 JOIN ai_runs r ON r.id = s.run_id
                 WHERE r.conversation_id = ?1 AND s.source_type = 'knowledge_document' LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "cites",
                    origin: "ai_derived",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::KnowledgeDocument,
                    target_id: doc,
                    note: None,
                    at: None,
                });
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT value FROM ai_attributes
                     WHERE conversation_id = ?1 AND attribute = 'product'
                       AND superseded_at IS NULL AND value_type = 'text' LIMIT ?2",
                )?;
                let names =
                    stmt.query_map(params![local_id, BRANCH_CAP], |r| r.get::<_, String>(0))?;
                for name in names {
                    if let Some(product) = product_id_by_name(&name?) {
                        push(DerivedEdge {
                            relation: "about_product",
                            origin: "ai_derived",
                            source_kind: kind,
                            source_id: local_id,
                            target_kind: GraphNodeKind::Product,
                            target_id: product,
                            note: None,
                            at: None,
                        });
                    }
                }
            }
            if let Some(campaign) = one_i64(
                "SELECT r.campaign_id FROM outreach_recipients r
                 JOIN conversations cv ON cv.remote_id = r.hs_conversation_remote_id
                 WHERE cv.id = ?1 AND r.hs_conversation_remote_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror(
                    "generated_conversation",
                    GraphNodeKind::Campaign,
                    campaign,
                    kind,
                    local_id,
                ));
            }
        }
        GraphNodeKind::KnownIssue => {
            let product_name: Option<String> = conn
                .query_row(
                    "SELECT product FROM known_issues
                     WHERE id = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                    params![local_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(name) = product_name
                && let Some(product) = product_id_by_name(&name)
            {
                push(DerivedEdge {
                    relation: "about_product",
                    origin: "deterministic_local",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::Product,
                    target_id: product,
                    note: None,
                    at: None,
                });
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT conversation_id, link_type FROM known_issue_links
                     WHERE known_issue_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })?;
                for row in rows {
                    let (conv, link_type) = row?;
                    let origin = match link_type.as_deref() {
                        Some("human") => "human_local",
                        _ => "ai_derived",
                    };
                    push(DerivedEdge {
                        relation: "linked_to_issue",
                        origin,
                        source_kind: GraphNodeKind::Conversation,
                        source_id: conv,
                        target_kind: kind,
                        target_id: local_id,
                        note: None,
                        at: None,
                    });
                }
            }
            for cluster in many_i64(
                "SELECT id FROM issue_clusters WHERE known_issue_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "promoted_to_issue",
                    GraphNodeKind::IssueCluster,
                    cluster,
                    kind,
                    local_id,
                ));
            }
        }
        GraphNodeKind::IssueCluster => {
            if let Some(issue) = one_i64(
                "SELECT known_issue_id FROM issue_clusters
                 WHERE id = ?1 AND known_issue_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror(
                    "promoted_to_issue",
                    kind,
                    local_id,
                    GraphNodeKind::KnownIssue,
                    issue,
                ));
            }
            let product_name: Option<String> = conn
                .query_row(
                    "SELECT product FROM issue_clusters
                     WHERE id = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                    params![local_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(name) = product_name
                && let Some(product) = product_id_by_name(&name)
            {
                push(DerivedEdge {
                    relation: "about_product",
                    origin: "deterministic_local",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::Product,
                    target_id: product,
                    note: None,
                    at: None,
                });
            }
            for conv in many_i64(
                "SELECT conversation_id FROM issue_cluster_conversations WHERE cluster_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "clustered_into",
                    origin: "deterministic_local",
                    source_kind: GraphNodeKind::Conversation,
                    source_id: conv,
                    target_kind: kind,
                    target_id: local_id,
                    note: None,
                    at: None,
                });
            }
        }
        GraphNodeKind::Incident => {
            if let Some(agent) = one_i64(
                "SELECT owner_user_local_id FROM incidents
                 WHERE id = ?1 AND owner_user_local_id IS NOT NULL",
                &[&local_id],
            ) {
                push(mirror("owns", kind, local_id, GraphNodeKind::Agent, agent));
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT target_kind, target_local_id, note FROM incident_related
                     WHERE incident_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })?;
                for row in rows {
                    let (target_kind, target_id, note) = row?;
                    let Some(tk) = GraphNodeKind::parse(&target_kind) else {
                        continue;
                    };
                    push(DerivedEdge {
                        relation: "related_to",
                        origin: "human_local",
                        source_kind: kind,
                        source_id: local_id,
                        target_kind: tk,
                        target_id,
                        note,
                        at: None,
                    });
                }
            }
            let product_name: Option<String> = conn
                .query_row(
                    "SELECT product FROM incidents
                     WHERE id = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                    params![local_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(name) = product_name
                && let Some(product) = product_id_by_name(&name)
            {
                push(DerivedEdge {
                    relation: "about_product",
                    origin: "deterministic_local",
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: GraphNodeKind::Product,
                    target_id: product,
                    note: None,
                    at: None,
                });
            }
            for conv in many_i64(
                "SELECT conversation_id FROM incident_conversations WHERE incident_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "affected_by",
                    origin: "deterministic_local",
                    source_kind: GraphNodeKind::Conversation,
                    source_id: conv,
                    target_kind: kind,
                    target_id: local_id,
                    note: None,
                    at: None,
                });
            }
        }
        GraphNodeKind::KnowledgeDocument => {
            for conv in many_i64(
                "SELECT r.conversation_id FROM ai_sources s
                 JOIN ai_runs r ON r.id = s.run_id
                 WHERE s.source_type = 'knowledge_document' AND s.source_id = ?1
                   AND r.conversation_id IS NOT NULL LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(DerivedEdge {
                    relation: "cites",
                    origin: "ai_derived",
                    source_kind: GraphNodeKind::Conversation,
                    source_id: conv,
                    target_kind: kind,
                    target_id: local_id,
                    note: None,
                    at: None,
                });
            }
            {
                let mut stmt = conn.prepare(
                    "SELECT related_document_ids FROM knowledge_gap_candidates
                     WHERE related_document_ids NOT IN ('[]', '')",
                )?;
                let lists = stmt.query_map([], |r| r.get::<_, String>(0))?;
                for list in lists {
                    let list = list?;
                    if let Ok(ids) = serde_json::from_str::<Vec<i64>>(&list).or_else(|_| {
                        serde_json::from_str::<Vec<String>>(&list).map(|v| {
                            v.iter()
                                .filter_map(|s| s.trim().parse::<i64>().ok())
                                .collect::<Vec<i64>>()
                        })
                    }) && ids.contains(&local_id)
                    {
                        for other in ids {
                            if other != local_id {
                                push(DerivedEdge {
                                    relation: "gap_evidence",
                                    origin: "deterministic_local",
                                    source_kind: kind,
                                    source_id: local_id,
                                    target_kind: kind,
                                    target_id: other,
                                    note: None,
                                    at: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        GraphNodeKind::Agent => {
            for conv in many_i64(
                "SELECT id FROM conversations WHERE assignee_local_id = ?1 AND deleted_at IS NULL LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "assigned_to",
                    GraphNodeKind::Conversation,
                    conv,
                    kind,
                    local_id,
                ));
            }
            for incident in many_i64(
                "SELECT id FROM incidents WHERE owner_user_local_id = ?1 LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "owns",
                    GraphNodeKind::Incident,
                    incident,
                    kind,
                    local_id,
                ));
            }
        }
        GraphNodeKind::Campaign => {
            {
                let mut stmt = conn.prepare(
                    "SELECT customer_local_id, sent_at FROM outreach_recipients
                     WHERE campaign_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })?;
                for row in rows {
                    let (cust, sent_at) = row?;
                    push(DerivedEdge {
                        relation: "sent_to",
                        origin: "helpscout_mirror",
                        source_kind: kind,
                        source_id: local_id,
                        target_kind: GraphNodeKind::Customer,
                        target_id: cust,
                        note: None,
                        at: sent_at,
                    });
                }
            }
            for conv in many_i64(
                "SELECT cv.id FROM outreach_recipients r
                 JOIN conversations cv ON cv.remote_id = r.hs_conversation_remote_id
                 WHERE r.campaign_id = ?1 AND r.hs_conversation_remote_id IS NOT NULL LIMIT ?2",
                &[&local_id, &BRANCH_CAP],
            ) {
                push(mirror(
                    "generated_conversation",
                    kind,
                    local_id,
                    GraphNodeKind::Conversation,
                    conv,
                ));
            }
        }
        GraphNodeKind::Product => {
            let product_name: Option<String> = conn
                .query_row(
                    "SELECT name FROM products WHERE id = ?1",
                    params![local_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(name) = product_name {
                for sql in [
                    "SELECT id FROM incidents WHERE product = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                    "SELECT id FROM known_issues WHERE product = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                    "SELECT id FROM issue_clusters WHERE product = ?1 AND product IS NOT NULL AND TRIM(product) != ''",
                ] {
                    for source in many_i64(&format!("{sql} LIMIT {BRANCH_CAP}"), &[&name]) {
                        let source_kind = if sql.contains("incidents") {
                            GraphNodeKind::Incident
                        } else if sql.contains("known_issues") {
                            GraphNodeKind::KnownIssue
                        } else {
                            GraphNodeKind::IssueCluster
                        };
                        push(DerivedEdge {
                            relation: "about_product",
                            origin: "deterministic_local",
                            source_kind,
                            source_id: source,
                            target_kind: kind,
                            target_id: local_id,
                            note: None,
                            at: None,
                        });
                    }
                }
                for conv in many_i64(
                    "SELECT DISTINCT conversation_id FROM ai_attributes
                     WHERE attribute = 'product' AND superseded_at IS NULL AND value_type = 'text'
                       AND value = ?1 LIMIT ?2",
                    &[&name, &BRANCH_CAP],
                ) {
                    push(DerivedEdge {
                        relation: "about_product",
                        origin: "ai_derived",
                        source_kind: GraphNodeKind::Conversation,
                        source_id: conv,
                        target_kind: kind,
                        target_id: local_id,
                        note: None,
                        at: None,
                    });
                }
            }
        }
        GraphNodeKind::CustomObject => {
            let mut stmt = conn.prepare(
                "SELECT target_kind, target_local_id, linked_by, linked_at
                     FROM custom_object_links WHERE object_id = ?1 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?;
            for row in rows {
                let (target_kind, target_id, linked_by, linked_at) = row?;
                let Some(tk) = GraphNodeKind::parse(&target_kind) else {
                    continue;
                };
                let origin = match linked_by.as_deref() {
                    Some("ai") => "ai_derived",
                    _ => "human_local",
                };
                push(DerivedEdge {
                    relation: "linked_to",
                    origin,
                    source_kind: kind,
                    source_id: local_id,
                    target_kind: tk,
                    target_id,
                    note: None,
                    at: linked_at,
                });
            }
        }
        GraphNodeKind::ConnectorData => {}
    }

    {
        let mut stmt = conn.prepare(
            "SELECT incident_id, note FROM incident_related
             WHERE target_kind = ?1 AND target_local_id = ?2 LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![kind.as_str(), local_id, BRANCH_CAP], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
        })?;
        for row in rows {
            let (incident, note) = row?;
            push(DerivedEdge {
                relation: "related_to",
                origin: "human_local",
                source_kind: GraphNodeKind::Incident,
                source_id: incident,
                target_kind: kind,
                target_id: local_id,
                note,
                at: None,
            });
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT object_id, linked_by, linked_at FROM custom_object_links
             WHERE target_kind = ?1 AND target_local_id = ?2 LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![kind.as_str(), local_id, BRANCH_CAP], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (object, linked_by, linked_at) = row?;
            let origin = match linked_by.as_deref() {
                Some("ai") => "ai_derived",
                _ => "human_local",
            };
            push(DerivedEdge {
                relation: "linked_to",
                origin,
                source_kind: GraphNodeKind::CustomObject,
                source_id: object,
                target_kind: kind,
                target_id: local_id,
                note: None,
                at: linked_at,
            });
        }
    }

    Ok(out)
}

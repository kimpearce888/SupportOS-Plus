use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::catalog::{GraphHumanRelation, GraphNodeKind};
use crate::error::Result;
use crate::search::escape_like;

const GRAPH_EDGES_SQL: &str = r#"
    CREATE TABLE IF NOT EXISTS graph_edges (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        source_kind TEXT NOT NULL
          CHECK (source_kind IN (
            'customer','organization','conversation','known_issue','issue_cluster',
            'incident','knowledge_document','agent','campaign','product',
            'custom_object','connector_data'
          )),
        source_local_id INTEGER NOT NULL,
        target_kind TEXT NOT NULL
          CHECK (target_kind IN (
            'customer','organization','conversation','known_issue','issue_cluster',
            'incident','knowledge_document','agent','campaign','product',
            'custom_object','connector_data'
          )),
        target_local_id INTEGER NOT NULL,
        relation TEXT NOT NULL
          CHECK (relation IN ('related_to','depends_on','blocks','mentions','duplicate_of')),
        note TEXT,
        created_by_user_local_id INTEGER REFERENCES users (id) ON DELETE SET NULL,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        provenance TEXT NOT NULL DEFAULT 'human_local',
        UNIQUE (source_kind, source_local_id, target_kind, target_local_id, relation)
    );
    CREATE INDEX IF NOT EXISTS idx_graph_edges_source
        ON graph_edges (source_kind, source_local_id);
    CREATE INDEX IF NOT EXISTS idx_graph_edges_target
        ON graph_edges (target_kind, target_local_id);
"#;

pub fn ensure_graph_edges_schema(conn: &Connection) -> Result<()> {
    let legacy_shape = table_columns(conn, "graph_edges")?
        .iter()
        .any(|c| c == "source_id");
    if legacy_shape {
        conn.execute("DROP TABLE graph_edges", [])?;
    }
    conn.execute_batch(GRAPH_EDGES_SQL)?;
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(cols)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNodeRef {
    pub kind: String,
    pub local_id: i64,
    pub label: String,
    pub sublabel: Option<String>,
    pub deleted: bool,
}

impl GraphNodeRef {
    fn removed(kind: GraphNodeKind, local_id: i64) -> Self {
        Self {
            kind: kind.as_str().to_string(),
            local_id,
            label: format!("#{local_id} (removed)"),
            sublabel: None,
            deleted: true,
        }
    }

    fn from_row(kind: GraphNodeKind, r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            kind: kind.as_str().to_string(),
            local_id: r.get(0)?,
            label: r.get(1)?,
            sublabel: r.get(2)?,
            deleted: r.get::<_, i64>(3)? != 0,
        })
    }
}

pub fn node_ref(conn: &Connection, kind: GraphNodeKind, id: i64) -> Result<Option<GraphNodeRef>> {
    let sql = match kind {
        GraphNodeKind::Customer => "SELECT cu.id,
                COALESCE(NULLIF(TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')), ''), 'Customer #' || cu.id),
                COALESCE(o.name, NULLIF(TRIM(cu.organization), '')),
                (cu.deleted_at IS NOT NULL)
             FROM customers cu LEFT JOIN organizations o ON o.id = cu.organization_id
             WHERE cu.id = ?1",
        GraphNodeKind::Organization => "SELECT o.id, o.name, o.domains, (o.deleted_at IS NOT NULL)
             FROM organizations o WHERE o.id = ?1",
        GraphNodeKind::Conversation => {
            "SELECT c.id, '#' || c.number || ' ' || COALESCE(SUBSTR(c.subject, 1, 100), '(no subject)'),
                c.status, (c.deleted_at IS NOT NULL)
             FROM conversations c WHERE c.id = ?1"
        }
        GraphNodeKind::KnownIssue => {
            "SELECT ki.id, COALESCE(NULLIF(TRIM(ki.title), ''), ki.name), ki.status, 0
             FROM known_issues ki WHERE ki.id = ?1"
        }
        GraphNodeKind::IssueCluster => {
            "SELECT ic.id, COALESCE(NULLIF(TRIM(ic.title), ''), ic.name), 'cluster', 0
             FROM issue_clusters ic WHERE ic.id = ?1"
        }
        GraphNodeKind::Incident => {
            "SELECT i.id, COALESCE(NULLIF(TRIM(COALESCE(i.code, '') || ' ' || COALESCE(i.title, '')), ''), 'Incident #' || i.id),
                i.status, 0
             FROM incidents i WHERE i.id = ?1"
        }
        GraphNodeKind::KnowledgeDocument => {
            "SELECT kd.id, kd.title, kd.visibility, 0 FROM knowledge_documents kd WHERE kd.id = ?1"
        }
        GraphNodeKind::Agent => {
            "SELECT u.id,
                COALESCE(NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), ''), 'User #' || u.id),
                u.email, (u.deleted_at IS NOT NULL)
             FROM users u WHERE u.id = ?1"
        }
        GraphNodeKind::Campaign => {
            "SELECT oc.id, oc.name, oc.status, 0 FROM outreach_campaigns oc WHERE oc.id = ?1"
        }
        GraphNodeKind::Product => {
            "SELECT p.id, p.name, p.description, 0 FROM products p WHERE p.id = ?1"
        }
        GraphNodeKind::CustomObject => {
            "SELECT co.id, co.title, cot.name, (co.deleted_at IS NOT NULL)
             FROM custom_objects co JOIN custom_object_types cot ON cot.id = co.type_id
             WHERE co.id = ?1"
        }
        GraphNodeKind::ConnectorData => {
            "SELECT cr.id, 'row ' || cr.row_key, cn.name, 0
             FROM connector_rows cr JOIN connectors cn ON cn.id = cr.connector_id
             WHERE cr.id = ?1"
        }
    };
    let row = conn.query_row(sql, params![id], |r| GraphNodeRef::from_row(kind, r));
    Ok(row.ok())
}

fn node_ref_or_tombstone(conn: &Connection, kind: GraphNodeKind, id: i64) -> Result<GraphNodeRef> {
    Ok(node_ref(conn, kind, id)?.unwrap_or_else(|| GraphNodeRef::removed(kind, id)))
}

#[derive(Debug, Clone, PartialEq)]
pub struct HumanEdge {
    pub id: i64,
    pub source: GraphNodeRef,
    pub target: GraphNodeRef,
    pub relation: GraphHumanRelation,
    pub note: Option<String>,
    pub created_at: String,
    pub created_by: Option<String>,
}

impl HumanEdge {
    pub fn to_json(&self) -> Value {
        let source = serde_json::to_value(&self.source).unwrap_or(Value::Null);
        let target = serde_json::to_value(&self.target).unwrap_or(Value::Null);
        json!({
            "id": self.id,
            "source": source,
            "target": target,
            "relation": self.relation.as_str(),
            "note": self.note,
            "created_at": self.created_at,
            "created_by": self.created_by,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkError {
    SelfEdge,
    SourceNotFound,
    TargetNotFound,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkInput {
    pub source_kind: GraphNodeKind,
    pub source_local_id: i64,
    pub target_kind: GraphNodeKind,
    pub target_local_id: i64,
    pub relation: GraphHumanRelation,
    pub note: Option<String>,
    pub user_local_id: Option<i64>,
}

pub fn human_edges_touching(
    conn: &Connection,
    kind: GraphNodeKind,
    local_id: i64,
) -> Result<Vec<Value>> {
    type TouchingRow = (bool, String, Option<String>, Option<String>, String, i64);

    let center = node_ref_or_tombstone(conn, kind, local_id)?;
    let mut raw: Vec<TouchingRow> = Vec::new();

    {
        let mut stmt = conn.prepare(
            "SELECT relation, note, created_at, target_kind, target_local_id
               FROM graph_edges WHERE source_kind = ?1 AND source_local_id = ?2
               ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![kind.as_str(), local_id], |r| {
                Ok((true, r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raw.extend(rows);
    }
    {
        let mut stmt = conn.prepare(
            "SELECT relation, note, created_at, source_kind, source_local_id
               FROM graph_edges WHERE target_kind = ?1 AND target_local_id = ?2
               ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![kind.as_str(), local_id], |r| {
                Ok((false, r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        raw.extend(rows);
    }

    let mut out = Vec::with_capacity(raw.len());
    for (outgoing, relation, note, at, other_kind_str, other_id) in raw {
        let Some(other_kind) = GraphNodeKind::ALL
            .into_iter()
            .find(|k| k.as_str() == other_kind_str)
        else {
            continue;
        };
        let other = node_ref_or_tombstone(conn, other_kind, other_id)?;
        let (source, target) = if outgoing {
            (center.clone(), other)
        } else {
            (other, center.clone())
        };
        out.push(json!({
            "relation": relation,
            "origin": "human_local",
            "source": source,
            "target": target,
            "note": note,
            "at": at,
        }));
    }
    Ok(out)
}

struct DerivedEdge {
    relation: &'static str,
    origin: &'static str,
    source_kind: GraphNodeKind,
    source_id: i64,
    target_kind: GraphNodeKind,
    target_id: i64,
    note: Option<String>,
    at: Option<String>,
}

fn derived_edges_touching(
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
        conn.prepare(sql)
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map(p, |r| r.get::<_, i64>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect())
                    .ok()
            })
            .unwrap_or_default()
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
            {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT stp.user_local_id, st.created_at
                     FROM side_thread_participants stp
                     JOIN side_threads st ON st.id = stp.side_thread_id
                     WHERE st.conversation_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })?;
                for row in rows {
                    let (agent, created_at) = row?;
                    push(DerivedEdge {
                        relation: "collaborated_on",
                        origin: "human_local",
                        source_kind: GraphNodeKind::Agent,
                        source_id: agent,
                        target_kind: kind,
                        target_id: local_id,
                        note: None,
                        at: created_at,
                    });
                }
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
            {
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT st.conversation_id, st.created_at
                     FROM side_thread_participants stp
                     JOIN side_threads st ON st.id = stp.side_thread_id
                     WHERE stp.user_local_id = ?1 LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![local_id, BRANCH_CAP], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                })?;
                for row in rows {
                    let (conv, created_at) = row?;
                    push(DerivedEdge {
                        relation: "collaborated_on",
                        origin: "human_local",
                        source_kind: kind,
                        source_id: local_id,
                        target_kind: GraphNodeKind::Conversation,
                        target_id: conv,
                        note: None,
                        at: created_at,
                    });
                }
            }
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

pub const GRAPH_MAX_NEIGHBOR_EDGES: usize = 200;
pub const GRAPH_MAX_SUBGRAPH_NODES: usize = 250;
pub const GRAPH_MAX_SUBGRAPH_DEPTH: u32 = 2;
pub const GRAPH_MAX_SEARCH_PER_KIND: usize = 10;
pub const GRAPH_MAX_SEARCH_TOTAL: usize = 80;

fn kind_label(kind: GraphNodeKind) -> &'static str {
    match kind {
        GraphNodeKind::Customer => "Customer",
        GraphNodeKind::Organization => "Organization",
        GraphNodeKind::Conversation => "Conversation",
        GraphNodeKind::KnownIssue => "Known issue",
        GraphNodeKind::IssueCluster => "Issue cluster",
        GraphNodeKind::Incident => "Incident",
        GraphNodeKind::KnowledgeDocument => "Knowledge document",
        GraphNodeKind::Agent => "Agent",
        GraphNodeKind::Campaign => "Campaign",
        GraphNodeKind::Product => "Product",
        GraphNodeKind::CustomObject => "Custom object",
        GraphNodeKind::ConnectorData => "Connector row",
    }
}

pub fn graph_stats(conn: &Connection) -> Result<Value> {
    let count = |sql: &str| -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) AS n FROM ({sql})"), [], |r| {
            r.get(0)
        })
        .unwrap_or(0)
    };
    let node_specs: [(&str, &str); 12] = [
        (
            "customer",
            "SELECT id FROM customers WHERE deleted_at IS NULL",
        ),
        (
            "organization",
            "SELECT id FROM organizations WHERE deleted_at IS NULL",
        ),
        (
            "conversation",
            "SELECT id FROM conversations WHERE deleted_at IS NULL",
        ),
        ("known_issue", "SELECT id FROM known_issues"),
        ("issue_cluster", "SELECT id FROM issue_clusters"),
        ("incident", "SELECT id FROM incidents"),
        ("knowledge_document", "SELECT id FROM knowledge_documents"),
        ("agent", "SELECT id FROM users WHERE deleted_at IS NULL"),
        ("campaign", "SELECT id FROM outreach_campaigns"),
        ("product", "SELECT id FROM products"),
        (
            "custom_object",
            "SELECT id FROM custom_objects WHERE deleted_at IS NULL",
        ),
        ("connector_data", "SELECT id FROM connector_rows"),
    ];
    let nodes: Vec<Value> = node_specs
        .iter()
        .map(|(kind, sql)| {
            let kind = GraphNodeKind::ALL
                .into_iter()
                .find(|k| k.as_str() == *kind)
                .expect("12 kinds");
            json!({"kind": kind.as_str(), "label": kind_label(kind), "count": count(sql)})
        })
        .collect();
    let edge_specs: [(&str, &str, &str); 18] = [
        (
            "belongs_to",
            "helpscout_mirror",
            "SELECT 1 FROM customers cu JOIN organizations o ON o.id = cu.organization_id",
        ),
        (
            "involves",
            "helpscout_mirror",
            "SELECT 1 FROM conversations c WHERE c.customer_local_id IS NOT NULL AND c.deleted_at IS NULL",
        ),
        (
            "assigned_to",
            "helpscout_mirror",
            "SELECT 1 FROM conversations c WHERE c.assignee_local_id IS NOT NULL AND c.deleted_at IS NULL",
        ),
        (
            "owns",
            "helpscout_mirror",
            "SELECT 1 FROM incidents i WHERE i.owner_user_local_id IS NOT NULL",
        ),
        (
            "linked_to_issue",
            "ai_derived",
            "SELECT 1 FROM known_issue_links",
        ),
        (
            "clustered_into",
            "deterministic_local",
            "SELECT 1 FROM issue_cluster_conversations",
        ),
        (
            "promoted_to_issue",
            "helpscout_mirror",
            "SELECT 1 FROM issue_clusters WHERE known_issue_id IS NOT NULL",
        ),
        (
            "affected_by",
            "deterministic_local",
            "SELECT 1 FROM incident_conversations",
        ),
        (
            "related_to",
            "human_local",
            "SELECT 1 FROM incident_related",
        ),
        (
            "linked_to",
            "human_local",
            "SELECT 1 FROM custom_object_links",
        ),
        (
            "sent_to",
            "helpscout_mirror",
            "SELECT 1 FROM outreach_recipients",
        ),
        (
            "generated_conversation",
            "helpscout_mirror",
            "SELECT 1 FROM outreach_recipients WHERE hs_conversation_remote_id IS NOT NULL",
        ),
        (
            "cites",
            "ai_derived",
            "SELECT 1 FROM ai_sources s JOIN ai_runs r ON r.id = s.run_id WHERE s.source_type = 'knowledge_document' AND r.conversation_id IS NOT NULL",
        ),
        (
            "collaborated_on",
            "human_local",
            "SELECT 1 FROM side_thread_participants stp JOIN side_threads st ON st.id = stp.side_thread_id",
        ),
        (
            "about_product",
            "deterministic_local",
            "SELECT 1 FROM (SELECT 1 FROM incidents WHERE product IS NOT NULL AND TRIM(product) != '' UNION ALL SELECT 1 FROM known_issues WHERE product IS NOT NULL AND TRIM(product) != '' UNION ALL SELECT 1 FROM issue_clusters WHERE product IS NOT NULL AND TRIM(product) != '')",
        ),
        (
            "about_product",
            "ai_derived",
            "SELECT 1 FROM ai_attributes WHERE attribute = 'product' AND superseded_at IS NULL AND value_type = 'text'",
        ),
        (
            "gap_evidence",
            "deterministic_local",
            "SELECT 1 FROM knowledge_gap_candidates WHERE related_document_ids NOT IN ('[]', '')",
        ),
        ("human_edge", "human_local", "SELECT 1 FROM graph_edges"),
    ];
    let edges: Vec<Value> = edge_specs
        .iter()
        .map(|(relation, origin, sql)| {
            json!({"relation": relation, "origin": origin, "count": count(sql)})
        })
        .collect();
    Ok(json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "nodes": nodes,
        "edges": edges,
        "human_edges": count("SELECT 1 FROM graph_edges"),
        "notes": [
            "Counts are live counts over the local mirror - no denormalized totals that could go stale.",
            "Connector rows carry no derived edges by design (rows are keyed only by row_key); humans can link them explicitly.",
            "linked_to_issue edges aggregate the human and AI link provenance stored per row."
        ]
    }))
}

pub fn graph_search(
    conn: &Connection,
    query: &str,
    kinds: Option<&[GraphNodeKind]>,
) -> Result<Vec<Value>> {
    let q: String = query.trim().chars().take(120).collect();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let like = format!("%{}%", escape_like(&q));
    let number: i64 = q.parse::<i64>().unwrap_or(-1);
    let wanted: Vec<GraphNodeKind> = match kinds {
        Some(list) => list.to_vec(),
        None => GraphNodeKind::ALL.to_vec(),
    };
    let per_kind = GRAPH_MAX_SEARCH_PER_KIND;
    let customer_label = "COALESCE(NULLIF(TRIM(COALESCE(cu.first_name, '') || ' ' || COALESCE(cu.last_name, '')), ''), 'Customer #' || cu.id)";
    let user_label = "COALESCE(NULLIF(TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')), ''), 'User #' || u.id)";
    let conversation_label =
        "'#' || c.number || ' ' || COALESCE(SUBSTR(c.subject, 1, 100), '(no subject)')";
    let incident_label =
        "COALESCE(i.code, 'INC') || ' ' || COALESCE(i.title, 'Incident #' || i.id)";
    let mut results: Vec<Value> = Vec::new();
    for kind in wanted {
        if results.len() >= GRAPH_MAX_SEARCH_TOTAL {
            break;
        }
        let (sql, number_bound) = match kind {
            GraphNodeKind::Conversation => (
                format!(
                    "SELECT c.id, {conversation_label} label, c.status sublabel, (c.deleted_at IS NOT NULL) deleted
                       FROM conversations c
                      WHERE (c.number = ?1 OR c.subject LIKE ?2 ESCAPE '\\') AND c.deleted_at IS NULL
                      LIMIT {per_kind}"
                ),
                true,
            ),
            GraphNodeKind::Customer => (
                format!(
                    "SELECT cu.id, {customer_label} label, NULL sublabel, (cu.deleted_at IS NOT NULL) deleted
                       FROM customers cu
                      WHERE ({customer_label} LIKE ?1 ESCAPE '\\' OR cu.last_name LIKE ?1 ESCAPE '\\')
                        AND cu.deleted_at IS NULL
                      LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::Organization => (
                format!(
                    "SELECT o.id, o.name label, o.domains sublabel, (o.deleted_at IS NOT NULL) deleted
                       FROM organizations o WHERE o.name LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::KnownIssue => (
                format!(
                    "SELECT ki.id, COALESCE(ki.title, ki.name) label, ki.status sublabel, 0 deleted
                       FROM known_issues ki WHERE COALESCE(ki.title, ki.name) LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::IssueCluster => (
                format!(
                    "SELECT ic.id, COALESCE(ic.title, ic.name) label, 'cluster' sublabel, 0 deleted
                       FROM issue_clusters ic WHERE COALESCE(ic.title, ic.name) LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::Incident => (
                format!(
                    "SELECT i.id, {incident_label} label, i.status sublabel, 0 deleted
                       FROM incidents i WHERE (i.title LIKE ?1 ESCAPE '\\' OR i.code LIKE ?1 ESCAPE '\\') LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::KnowledgeDocument => (
                format!(
                    "SELECT kd.id, kd.title label, kd.visibility sublabel, 0 deleted
                       FROM knowledge_documents kd WHERE kd.title LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::Agent => (
                format!(
                    "SELECT u.id, {user_label} label, u.email sublabel, (u.deleted_at IS NOT NULL) deleted
                       FROM users u WHERE ({user_label} LIKE ?1 ESCAPE '\\' OR u.email LIKE ?1 ESCAPE '\\') LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::Campaign => (
                format!(
                    "SELECT oc.id, oc.name label, oc.status sublabel, 0 deleted
                       FROM outreach_campaigns oc WHERE oc.name LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::Product => (
                format!(
                    "SELECT p.id, p.name label, p.description sublabel, 0 deleted
                       FROM products p WHERE p.name LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::CustomObject => (
                format!(
                    "SELECT co.id, co.title label, cot.name sublabel, (co.deleted_at IS NOT NULL) deleted
                       FROM custom_objects co JOIN custom_object_types cot ON cot.id = co.type_id
                      WHERE co.title LIKE ?1 ESCAPE '\\' AND co.deleted_at IS NULL LIMIT {per_kind}"
                ),
                false,
            ),
            GraphNodeKind::ConnectorData => (
                format!(
                    "SELECT cr.id, 'row ' || cr.row_key label, cn.name sublabel, 0 deleted
                       FROM connector_rows cr JOIN connectors cn ON cn.id = cr.connector_id
                      WHERE cr.row_key LIKE ?1 ESCAPE '\\' LIMIT {per_kind}"
                ),
                false,
            ),
        };
        let rows: Vec<Value> = conn
            .prepare(&sql)
            .and_then(|mut stmt| {
                let rows: Vec<rusqlite::Result<Value>> = if number_bound {
                    stmt.query_map(params![number, like], map_search_row(kind))?
                        .collect()
                } else {
                    stmt.query_map(params![like], map_search_row(kind))?
                        .collect()
                };
                Ok(rows.into_iter().filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default();
        results.extend(rows);
    }
    Ok(results)
}

fn map_search_row(kind: GraphNodeKind) -> impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    move |r| {
        Ok(json!({
            "kind": kind.as_str(),
            "local_id": r.get::<_, i64>(0)?,
            "label": r.get::<_, String>(1)?,
            "sublabel": r.get::<_, Option<String>>(2)?,
            "deleted": r.get::<_, i64>(3)? != 0,
        }))
    }
}

pub fn neighbors_json(
    conn: &Connection,
    kind: GraphNodeKind,
    id: i64,
    direction: &str,
    limit: i64,
) -> Result<Option<Value>> {
    let Some(center) = node_ref(conn, kind, id)? else {
        return Ok(None);
    };
    let center_json = serde_json::to_value(&center).unwrap_or(Value::Null);
    let limit = limit.clamp(1, GRAPH_MAX_NEIGHBOR_EDGES as i64) as usize;
    let mut raw: usize = 0;
    let mut edges: Vec<(String, String, Value)> = Vec::new();
    let human_ref = |kind: GraphNodeKind, id: i64| -> Value {
        node_ref(conn, kind, id)
            .ok()
            .flatten()
            .and_then(|r| serde_json::to_value(r).ok())
            .unwrap_or_else(|| {
                json!({
                    "kind": kind.as_str(),
                    "local_id": id,
                    "label": format!("#{id} (removed)"),
                    "sublabel": Value::Null,
                    "deleted": true,
                })
            })
    };
    if direction != "in" {
        let mut stmt = conn.prepare(
            "SELECT relation, note, created_at, target_kind, target_local_id
               FROM graph_edges WHERE source_kind = ?1 AND source_local_id = ?2
               LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![kind.as_str(), id, GRAPH_MAX_NEIGHBOR_EDGES as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )?;
        for row in rows {
            let (relation, note, at, target_kind, target_local_id) = row?;
            let Some(tk) = GraphNodeKind::ALL
                .into_iter()
                .find(|k| k.as_str() == target_kind)
            else {
                continue;
            };
            raw += 1;
            let target = human_ref(tk, target_local_id);
            let label = target["label"].as_str().unwrap_or("").to_string();
            edges.push((
                relation.clone(),
                label,
                json!({
                    "relation": relation,
                    "origin": "human_local",
                    "source": center_json,
                    "target": target,
                    "note": note,
                    "at": at,
                }),
            ));
        }
    }
    if direction != "out" {
        let mut stmt = conn.prepare(
            "SELECT relation, note, created_at, source_kind, source_local_id
               FROM graph_edges WHERE target_kind = ?1 AND target_local_id = ?2
               LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![kind.as_str(), id, GRAPH_MAX_NEIGHBOR_EDGES as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )?;
        for row in rows {
            let (relation, note, at, source_kind, source_local_id) = row?;
            let Some(sk) = GraphNodeKind::ALL
                .into_iter()
                .find(|k| k.as_str() == source_kind)
            else {
                continue;
            };
            raw += 1;
            let source = human_ref(sk, source_local_id);
            edges.push((
                relation.clone(),
                center_json["label"].as_str().unwrap_or("").to_string(),
                json!({
                    "relation": relation,
                    "origin": "human_local",
                    "source": source,
                    "target": center_json,
                    "note": note,
                    "at": at,
                }),
            ));
        }
    }
    for de in derived_edges_touching(conn, kind, id)? {
        let is_out = de.source_kind == kind && de.source_id == id;
        let is_in = de.target_kind == kind && de.target_id == id;
        let (outward, far) = if is_out {
            (true, (de.target_kind, de.target_id))
        } else if is_in {
            (false, (de.source_kind, de.source_id))
        } else {
            continue;
        };
        if (outward && direction == "in") || (!outward && direction == "out") {
            continue;
        }
        raw += 1;
        let far_json = human_ref(far.0, far.1);
        let (source, target, sort_label) = if outward {
            (
                center_json.clone(),
                far_json.clone(),
                far_json["label"].as_str().unwrap_or("").to_string(),
            )
        } else {
            (
                far_json.clone(),
                center_json.clone(),
                center_json["label"].as_str().unwrap_or("").to_string(),
            )
        };
        edges.push((
            de.relation.to_string(),
            sort_label,
            json!({
                "relation": de.relation,
                "origin": de.origin,
                "source": source,
                "target": target,
                "note": de.note,
                "at": de.at,
            }),
        ));
    }
    edges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let truncated = raw > limit;
    let mut notes: Vec<String> = vec![
        "Derived edges are computed live from the local mirror - they can never drift from the data they describe.".to_string(),
        "about_product edges from conversations carry AI-attribute provenance; everything deterministic is labeled as such.".to_string(),
        "Connector rows have no derived links by design - only human-asserted edges can connect them.".to_string(),
    ];
    if truncated {
        notes.push(format!(
            "Bounded to the first {limit} of {raw} edges (deep hubs: expand from a specific neighbor)."
        ));
    }
    Ok(Some(json!({
        "node": center_json,
        "edges": edges.into_iter().take(limit).map(|(_, _, e)| e).collect::<Vec<_>>(),
        "total_edges": raw,
        "truncated": truncated,
        "notes": notes,
    })))
}

pub fn subgraph_json(
    conn: &Connection,
    kind: GraphNodeKind,
    id: i64,
    depth: u32,
    max_nodes: u32,
) -> Result<Option<Value>> {
    let Some(seed) = node_ref(conn, kind, id)? else {
        return Ok(None);
    };
    let depth = depth.clamp(1, GRAPH_MAX_SUBGRAPH_DEPTH);
    let max_nodes = max_nodes.clamp(10, GRAPH_MAX_SUBGRAPH_NODES as u32) as usize;
    let seed_json = serde_json::to_value(&seed).unwrap_or(Value::Null);
    let node_key = |v: &Value| {
        format!(
            "{}:{}",
            v["kind"].as_str().unwrap_or(""),
            v["local_id"].as_i64().unwrap_or(-1)
        )
    };
    let mut nodes: Vec<Value> = vec![seed_json.clone()];
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::from([node_key(&seed_json)]);
    let mut edges: Vec<Value> = Vec::new();
    // Audit item 118: a mutual link is two graph_edges rows (A→B and
    // B→A), and "both" expansion returns both — so every node's
    // expansion pushed the same logical link again, and expanding the far
    // node pushed it twice more. Edges are deduped by their canonical
    // unordered (relation, node-pair) key, so a mutual link renders once
    // regardless of how many expansions return it; genuinely distinct
    // relations between the same pair stay distinct.
    let mut edge_keys: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    let edge_key = |e: &Value| -> (String, String, String) {
        let relation = e["relation"].as_str().unwrap_or("").to_string();
        let sk = node_key(&e["source"]);
        let tk = node_key(&e["target"]);
        if sk <= tk {
            (relation, sk, tk)
        } else {
            (relation, tk, sk)
        }
    };
    let mut frontier: Vec<Value> = vec![seed_json.clone()];
    let mut depth_reached: u32 = 0;
    for d in 0..depth {
        if nodes.len() >= max_nodes {
            break;
        }
        let mut next: Vec<Value> = Vec::new();
        for f in &frontier {
            if nodes.len() >= max_nodes {
                break;
            }
            let (Some(fk), Some(fi)) = (
                f["kind"].as_str().and_then(GraphNodeKind::parse),
                f["local_id"].as_i64(),
            ) else {
                continue;
            };
            let Some(nb) = neighbors_json(conn, fk, fi, "both", 40)? else {
                continue;
            };
            for edge in nb["edges"].as_array().cloned().unwrap_or_default() {
                if !edge_keys.insert(edge_key(&edge)) {
                    continue;
                }
                edges.push(edge.clone());
                let far = if node_key(&edge["source"]) == node_key(f) {
                    edge["target"].clone()
                } else {
                    edge["source"].clone()
                };
                if !seen.contains(&node_key(&far)) {
                    if nodes.len() >= max_nodes {
                        break;
                    }
                    seen.insert(node_key(&far));
                    nodes.push(far.clone());
                    next.push(far);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
        depth_reached = d + 1;
    }
    let truncated = nodes.len() >= max_nodes;
    let edge_cap = (max_nodes * 3).min(edges.len());
    Ok(Some(json!({
        "seeds": [seed_json],
        "nodes": nodes,
        "edges": edges[..edge_cap],
        "truncated": truncated,
        "depth_reached": depth_reached,
        "notes": [
            format!("Bounded exploration: at most {depth} hop(s) and {max_nodes} nodes."),
            if truncated {
                "Node cap reached - expand from a specific neighbor instead of deepening blindly."
            } else {
                "Full expansion within bounds."
            }
        ]
    })))
}

#[cfg(test)]
mod batch31_tests {
    use super::*;
    use rusqlite::Connection;

    /// Audit item 118: a mutual link is two graph_edges rows, and both
    /// directions of "both" expansion return them — the subgraph pushed
    /// the same logical link once per expansion (and twice more when the
    /// far node expanded). The canonical unordered key renders it once.
    #[test]
    fn subgraph_renders_a_mutual_link_once() {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        for remote in [1_i64, 2] {
            conn.execute(
                "INSERT INTO conversations (remote_id, number, subject, preview, status)
                 VALUES (?1, ?1, 'graph node', 'p', 'active')",
                [remote],
            )
            .expect("conversation");
        }
        let a: i64 = conn.last_insert_rowid(); // id 2
        let b = a - 1; // id 1
        // The mutual pair: A→B and B→A.
        conn.execute(
            "INSERT INTO graph_edges (source_kind, source_local_id, target_kind, target_local_id, relation)
             VALUES ('conversation', ?1, 'conversation', ?2, 'related_to')",
            params![b, a],
        )
        .expect("edge A→B");
        conn.execute(
            "INSERT INTO graph_edges (source_kind, source_local_id, target_kind, target_local_id, relation)
             VALUES ('conversation', ?1, 'conversation', ?2, 'related_to')",
            params![a, b],
        )
        .expect("edge B→A");

        let g = subgraph_json(&conn, GraphNodeKind::Conversation, b, 2, 50)
            .expect("subgraph")
            .expect("seed found");
        let edges = g["edges"].as_array().expect("edges");
        assert_eq!(
            edges.len(),
            1,
            "one mutual link, not one per direction per expansion: {edges:?}"
        );
        // The far node is still discovered.
        let nodes = g["nodes"].as_array().expect("nodes");
        assert_eq!(nodes.len(), 2, "both endpoints present");
    }
}

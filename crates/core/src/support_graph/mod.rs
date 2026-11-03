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

pub(crate) fn ensure_graph_edges_schema(conn: &Connection) -> Result<()> {
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

pub(crate) fn node_ref(
    conn: &Connection,
    kind: GraphNodeKind,
    id: i64,
) -> Result<Option<GraphNodeRef>> {
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
pub struct LinkInput {
    pub source_kind: GraphNodeKind,
    pub source_local_id: i64,
    pub target_kind: GraphNodeKind,
    pub target_local_id: i64,
    pub relation: GraphHumanRelation,
    pub note: Option<String>,
    pub user_local_id: Option<i64>,
}

pub(crate) fn human_edges_touching(
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

mod derived;

use derived::derived_edges_touching;

pub(crate) const GRAPH_MAX_NEIGHBOR_EDGES: usize = 200;
pub(crate) const GRAPH_MAX_SEARCH_PER_KIND: usize = 10;
pub(crate) const GRAPH_MAX_SEARCH_TOTAL: usize = 80;

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

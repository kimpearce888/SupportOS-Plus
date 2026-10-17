use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub const MAX_TREE_DEPTH: u32 = 10;
pub const MAX_NODE_COUNT: u32 = 200;

fn escape_like(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for ch in v.chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn lower(s: &str) -> String {
    s.to_lowercase()
}

type SqlParams = Vec<rusqlite::types::Value>;

fn text(s: &str) -> rusqlite::types::Value {
    rusqlite::types::Value::Text(s.to_string())
}

fn integer(i: i64) -> rusqlite::types::Value {
    rusqlite::types::Value::Integer(i)
}

fn real(x: f64) -> rusqlite::types::Value {
    rusqlite::types::Value::Real(x)
}

fn placeholders(n: usize) -> String {
    vec!["?"; n].join(",")
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiAttrTest {
    pub attribute: String,
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TicketCustomFieldTest {
    pub field_local_id: i64,
    pub op: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SegmentNode {
    Group {
        combinator: String,
        children: Vec<SegmentNode>,
    },
    CustomerProperty {
        definition_id: i64,
        name: String,
        prop_type: String,
        op: String,
        value: Option<String>,
        value2: Option<String>,
        values: Vec<String>,
    },
    Contact {
        field: String,
        op: String,
        value: Option<String>,
    },
    Ticket {
        tags: Vec<String>,
        tag_mode: String,
        statuses: Vec<String>,
        mailbox_ids: Vec<i64>,
        assignee_ids: Vec<i64>,
        created_within_days: Option<f64>,
        modified_within_days: Option<f64>,
        number_min: Option<f64>,
        number_max: Option<f64>,
        ai_attribute: Option<AiAttrTest>,
        custom_fields: Vec<TicketCustomFieldTest>,
        channel: Option<String>,
    },
    History {
        metric: String,
        op: String,
        value: f64,
    },
    HistoryTag {
        tag: String,
        within_days: Option<f64>,
    },
    OrganizationProperty {
        field: Option<String>,
        definition_id: Option<i64>,
        name: Option<String>,
        prop_type: Option<String>,
        op: String,
        value: Option<String>,
        value2: Option<String>,
        values: Vec<String>,
    },
    HistoryIssue {
        issue_kind: String,
        issue_local_id: Option<i64>,
        op: String,
        value: f64,
    },
    IncidentExposure {
        incident_id: Option<i64>,
        within_days: Option<f64>,
    },
    CampaignHistory {
        relation: String,
        campaign_id: Option<i64>,
    },
    SupportHealth {
        metric: String,
        op: String,
        value: f64,
    },
    CustomObjectLink {
        type_id: Option<i64>,
    },
    CustomerEvent {
        event_kind: String,
        within_days: Option<f64>,
    },
}

impl SegmentNode {
    pub fn kind(&self) -> &'static str {
        match self {
            SegmentNode::Group { .. } => "group",
            SegmentNode::CustomerProperty { .. } => "customer_property",
            SegmentNode::Contact { .. } => "contact",
            SegmentNode::Ticket { .. } => "ticket",
            SegmentNode::History { .. } => "history",
            SegmentNode::HistoryTag { .. } => "history_tag",
            SegmentNode::OrganizationProperty { .. } => "organization_property",
            SegmentNode::HistoryIssue { .. } => "history_issue",
            SegmentNode::IncidentExposure { .. } => "incident_exposure",
            SegmentNode::CampaignHistory { .. } => "campaign_history",
            SegmentNode::SupportHealth { .. } => "support_health",
            SegmentNode::CustomObjectLink { .. } => "custom_object_link",
            SegmentNode::CustomerEvent { .. } => "customer_event",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SegmentDefinition {
    pub combinator: String,
    pub conditions: Vec<SegmentNode>,
    pub exclude: Vec<SegmentNode>,
}

impl SegmentDefinition {
    pub fn to_json(&self) -> Value {
        json!({
            "combinator": self.combinator,
            "conditions": self.conditions.iter().map(node_to_json).collect::<Vec<_>>(),
            "exclude": self.exclude.iter().map(node_to_json).collect::<Vec<_>>(),
        })
    }
}

fn node_to_json(n: &SegmentNode) -> Value {
    match n {
        SegmentNode::Group {
            combinator,
            children,
        } => json!({
            "kind": "group",
            "combinator": combinator,
            "children": children.iter().map(node_to_json).collect::<Vec<_>>(),
        }),
        SegmentNode::CustomerProperty {
            definition_id,
            name,
            prop_type,
            op,
            value,
            value2,
            values,
        } => json!({
            "kind": "customer_property",
            "definitionId": definition_id,
            "name": name,
            "type": prop_type,
            "op": op,
            "value": value,
            "value2": value2,
            "values": values,
        }),
        SegmentNode::Contact { field, op, value } => json!({
            "kind": "contact",
            "field": field,
            "op": op,
            "value": value,
        }),
        SegmentNode::Ticket {
            tags,
            tag_mode,
            statuses,
            mailbox_ids,
            assignee_ids,
            created_within_days,
            modified_within_days,
            number_min,
            number_max,
            ai_attribute,
            custom_fields,
            channel,
        } => json!({
            "kind": "ticket",
            "tags": tags,
            "tagMode": tag_mode,
            "statuses": statuses,
            "mailboxLocalIds": mailbox_ids,
            "assigneeLocalIds": assignee_ids,
            "createdWithinDays": created_within_days,
            "modifiedWithinDays": modified_within_days,
            "numberMin": number_min,
            "numberMax": number_max,
            "aiAttribute": ai_attribute.as_ref().map(|aa| json!({"attribute": aa.attribute, "op": aa.op, "value": aa.value})),
            "customFields": custom_fields.iter().map(|cf| json!({"fieldLocalId": cf.field_local_id, "op": cf.op, "value": cf.value})).collect::<Vec<_>>(),
            "channel": channel,
        }),
        SegmentNode::History { metric, op, value } => json!({
            "kind": "history",
            "metric": metric,
            "op": op,
            "value": value,
        }),
        SegmentNode::HistoryTag { tag, within_days } => json!({
            "kind": "history_tag",
            "tag": tag,
            "withinDays": within_days,
        }),
        SegmentNode::OrganizationProperty {
            field,
            definition_id,
            name,
            prop_type,
            op,
            value,
            value2,
            values,
        } => {
            let mut v = json!({"kind": "organization_property", "op": op, "value": value, "value2": value2, "values": values});
            if let Some(f) = field {
                v["field"] = json!(f);
            }
            if let Some(d) = definition_id {
                v["definitionId"] = json!(d);
            }
            if let Some(nm) = name {
                v["name"] = json!(nm);
            }
            if let Some(t) = prop_type {
                v["type"] = json!(t);
            }
            v
        }
        SegmentNode::HistoryIssue {
            issue_kind,
            issue_local_id,
            op,
            value,
        } => json!({
            "kind": "history_issue",
            "issueKind": issue_kind,
            "issueLocalId": issue_local_id,
            "op": op,
            "value": value,
        }),
        SegmentNode::IncidentExposure {
            incident_id,
            within_days,
        } => json!({
            "kind": "incident_exposure",
            "incidentId": incident_id,
            "withinDays": within_days,
        }),
        SegmentNode::CampaignHistory {
            relation,
            campaign_id,
        } => json!({
            "kind": "campaign_history",
            "relation": relation,
            "campaignId": campaign_id,
        }),
        SegmentNode::SupportHealth { metric, op, value } => json!({
            "kind": "support_health",
            "metric": metric,
            "op": op,
            "value": value,
        }),
        SegmentNode::CustomObjectLink { type_id } => json!({
            "kind": "custom_object_link",
            "typeId": type_id,
        }),
        SegmentNode::CustomerEvent {
            event_kind,
            within_days,
        } => json!({
            "kind": "customer_event",
            "eventKind": event_kind,
            "withinDays": within_days,
        }),
    }
}

fn as_f64(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn as_i64(v: Option<&Value>) -> Option<i64> {
    match v {
        Some(Value::Number(n)) if n.is_i64() => n.as_i64(),
        Some(Value::Number(n)) => n.as_f64().map(|f| f as i64),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn as_str(v: Option<&Value>) -> Option<String> {
    v.and_then(|x| x.as_str()).map(str::to_string)
}

fn as_str_array(v: Option<&Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn as_i64_array(v: Option<&Value>) -> Vec<i64> {
    v.and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|x| as_i64(Some(x))).collect())
        .unwrap_or_default()
}

pub fn parse_segment_tree(body: &Value) -> Result<SegmentDefinition, String> {
    let obj = body
        .as_object()
        .ok_or_else(|| "Body must be { combinator, conditions[], exclude[] }.".to_string())?;
    let combinator = if obj.get("combinator").and_then(|v| v.as_str()) == Some("any") {
        "any"
    } else {
        "all"
    };
    let conditions = obj
        .get("conditions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Body must be { combinator, conditions[], exclude[] }.".to_string())?;
    let exclude = obj
        .get("exclude")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Body must be { combinator, conditions[], exclude[] }.".to_string())?;
    if conditions.len() > 50 || exclude.len() > 50 {
        return Err("Body must be { combinator, conditions[], exclude[] }.".to_string());
    }
    let mut nodes = 0u32;
    let mut conditions_parsed = Vec::with_capacity(conditions.len());
    for n in conditions {
        conditions_parsed.push(parse_segment_node(n, 1, &mut nodes)?);
    }
    let mut exclude_parsed = Vec::with_capacity(exclude.len());
    for n in exclude {
        exclude_parsed.push(parse_segment_node(n, 1, &mut nodes)?);
    }
    Ok(SegmentDefinition {
        combinator: combinator.to_string(),
        conditions: conditions_parsed,
        exclude: exclude_parsed,
    })
}

fn parse_segment_node(v: &Value, depth: u32, nodes: &mut u32) -> Result<SegmentNode, String> {
    *nodes += 1;
    if *nodes > MAX_NODE_COUNT || depth > MAX_TREE_DEPTH {
        return Err("Body must be { combinator, conditions[], exclude[] }.".to_string());
    }
    let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    if kind == "group" {
        let children = v
            .get("children")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();
        let combinator = if v.get("combinator").and_then(|c| c.as_str()) == Some("any") {
            "any"
        } else {
            "all"
        };
        let mut parsed = Vec::with_capacity(children.len());
        for c in &children {
            parsed.push(parse_segment_node(c, depth + 1, nodes)?);
        }
        return Ok(SegmentNode::Group {
            combinator: combinator.to_string(),
            children: parsed,
        });
    }
    let node = match kind {
        "customer_property" => SegmentNode::CustomerProperty {
            definition_id: as_i64(v.get("definitionId")).unwrap_or(0),
            name: as_str(v.get("name")).unwrap_or_default(),
            prop_type: as_str(v.get("type")).unwrap_or_else(|| "text".to_string()),
            op: as_str(v.get("op")).unwrap_or_else(|| "equals".to_string()),
            value: as_str(v.get("value")),
            value2: as_str(v.get("value2")),
            values: as_str_array(v.get("values")),
        },
        "contact" => SegmentNode::Contact {
            field: as_str(v.get("field")).unwrap_or_else(|| "email".to_string()),
            op: as_str(v.get("op")).unwrap_or_else(|| "equals".to_string()),
            value: as_str(v.get("value")),
        },
        "ticket" => {
            let ai_attribute = v
                .get("aiAttribute")
                .and_then(|aa| aa.as_object())
                .and_then(|obj| {
                    let attr = as_str(aa_get(obj, "attribute"))?;
                    if attr.is_empty() {
                        return None;
                    }
                    Some(AiAttrTest {
                        attribute: attr,
                        op: as_str(aa_get(obj, "op")).unwrap_or_else(|| "equals".to_string()),
                        value: as_str(aa_get(obj, "value")).unwrap_or_default(),
                    })
                });
            let custom_fields = v
                .get("customFields")
                .and_then(|c| c.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|cf| {
                            let id = as_i64(cf.get("fieldLocalId"))?;
                            if id <= 0 {
                                return None;
                            }
                            Some(TicketCustomFieldTest {
                                field_local_id: id,
                                op: as_str(cf.get("op"))
                                    .unwrap_or_else(|| "is_not_empty".to_string()),
                                value: as_str(cf.get("value")),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            SegmentNode::Ticket {
                tags: as_str_array(v.get("tags")),
                tag_mode: as_str(v.get("tagMode")).unwrap_or_else(|| "any".to_string()),
                statuses: as_str_array(v.get("statuses")),
                mailbox_ids: as_i64_array(v.get("mailboxLocalIds")),
                assignee_ids: as_i64_array(v.get("assigneeLocalIds")),
                created_within_days: as_f64(v.get("createdWithinDays")),
                modified_within_days: as_f64(v.get("modifiedWithinDays")),
                number_min: as_f64(v.get("numberMin")),
                number_max: as_f64(v.get("numberMax")),
                ai_attribute,
                custom_fields,
                channel: as_str(v.get("channel")).filter(|c| !c.trim().is_empty()),
            }
        }
        "history" => SegmentNode::History {
            metric: as_str(v.get("metric")).unwrap_or_else(|| "ticket_count".to_string()),
            op: as_str(v.get("op")).unwrap_or_else(|| "gte".to_string()),
            value: as_f64(v.get("value")).unwrap_or(0.0),
        },
        "history_tag" => SegmentNode::HistoryTag {
            tag: as_str(v.get("tag")).unwrap_or_default(),
            within_days: as_f64(v.get("withinDays")),
        },
        "organization_property" => SegmentNode::OrganizationProperty {
            field: as_str(v.get("field")).filter(|f| f == "name" || f == "domains"),
            definition_id: as_i64(v.get("definitionId")),
            name: as_str(v.get("name")),
            prop_type: as_str(v.get("type")),
            op: as_str(v.get("op")).unwrap_or_else(|| "equals".to_string()),
            value: as_str(v.get("value")),
            value2: as_str(v.get("value2")),
            values: as_str_array(v.get("values")),
        },
        "history_issue" => SegmentNode::HistoryIssue {
            issue_kind: as_str(v.get("issueKind")).unwrap_or_else(|| "known_issue".to_string()),
            issue_local_id: as_i64(v.get("issueLocalId")).filter(|id| *id > 0),
            op: as_str(v.get("op")).unwrap_or_else(|| "gte".to_string()),
            value: as_f64(v.get("value")).unwrap_or(0.0),
        },
        "incident_exposure" => SegmentNode::IncidentExposure {
            incident_id: as_i64(v.get("incidentId")).filter(|id| *id > 0),
            within_days: as_f64(v.get("withinDays")).map(|d| d.max(0.0)),
        },
        "campaign_history" => SegmentNode::CampaignHistory {
            relation: as_str(v.get("relation")).unwrap_or_else(|| "received".to_string()),
            campaign_id: as_i64(v.get("campaignId")).filter(|id| *id > 0),
        },
        "support_health" => SegmentNode::SupportHealth {
            metric: as_str(v.get("metric")).unwrap_or_else(|| "avg_rating".to_string()),
            op: as_str(v.get("op")).unwrap_or_else(|| "gte".to_string()),
            value: as_f64(v.get("value")).unwrap_or(0.0),
        },
        "custom_object_link" => SegmentNode::CustomObjectLink {
            type_id: as_i64(v.get("typeId")).filter(|id| *id > 0),
        },
        "customer_event" => SegmentNode::CustomerEvent {
            event_kind: as_str(v.get("eventKind")).unwrap_or_default(),
            within_days: as_f64(v.get("withinDays")).map(|d| d.max(0.0)),
        },
        _ => SegmentNode::Contact {
            field: String::new(),
            op: "unknown_kind".to_string(),
            value: None,
        },
    };
    Ok(node)
}

fn aa_get<'v>(obj: &'v serde_json::Map<String, Value>, key: &str) -> Option<&'v Value> {
    obj.get(key)
}

fn describe_op(op: &str) -> &str {
    match op {
        "equals" => "=",
        "not_equals" => "is not",
        "contains" => "contains",
        "not_contains" => "does not contain",
        "starts_with" => "starts with",
        "ends_with" => "ends with",
        "is_empty" => "is empty",
        "is_not_empty" => "is set",
        _ => op,
    }
}

fn describe_contact_presence(field: &str) -> &str {
    match field {
        "has_email" => "has an email address",
        "has_phone" => "has a phone number",
        "has_multiple_emails" => "has multiple email addresses",
        f => f,
    }
}

fn describe_history(metric: &str, op: &str, value: f64) -> String {
    let m = match metric {
        "ticket_count" => "total tickets",
        "open_count" => "open tickets",
        "closed_count" => "closed tickets",
        "last_contact_within_days" => "contacted within the last (days)",
        "first_contact_before_days" => "first contact older than (days)",
        other => other,
    };
    let o = match op {
        "gte" => "is at least",
        "lte" => "is at most",
        _ => "=",
    };
    format!("{m} {o} {value}")
}

pub struct SegmentEngine<'a> {
    conn: &'a Connection,
}

impl<'a> SegmentEngine<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn preview(&self, def: &SegmentDefinition, page: i64, page_size: i64) -> Value {
        let mut notes: Vec<String> = Vec::new();
        let include = self.evaluate_nodes(&def.conditions, def.combinator == "all", &mut notes, 0);
        let mut exclude = HashSet::new();
        if !def.exclude.is_empty() {
            for id in self.evaluate_nodes(&def.exclude, false, &mut notes, 0) {
                exclude.insert(id);
            }
        }
        let dnc: HashSet<i64> = self.dnc_ids().into_iter().collect();
        let matched: Vec<i64> = include
            .iter()
            .copied()
            .filter(|id| !exclude.contains(id) && !dnc.contains(id))
            .collect();
        let rows = if matched.is_empty() {
            Vec::new()
        } else {
            self.build_rows(&matched, def, page, page_size, &exclude, &dnc)
        };
        let without_email = matched
            .iter()
            .filter(|id| self.primary_email(**id).is_none())
            .count();
        if !def.exclude.is_empty() && !exclude.is_empty() {
            notes.push(format!(
                "{} customer(s) matched exclusion conditions.",
                exclude.len()
            ));
        }
        let dnc_removed = include.iter().filter(|id| dnc.contains(*id)).count();
        if dnc_removed > 0 {
            notes.push(format!(
                "{dnc_removed} customer(s) removed by the Do-Not-Contact list."
            ));
        }
        json!({
            "matched": matched.len(),
            "excluded": exclude.len(),
            "without_email": without_email,
            "on_dnc": dnc.len(),
            "rows": rows,
            "notes": notes,
        })
    }

    pub fn count(&self, def: &SegmentDefinition) -> i64 {
        let mut notes = Vec::new();
        let include = self.evaluate_nodes(&def.conditions, def.combinator == "all", &mut notes, 0);
        if include.is_empty() {
            return 0;
        }
        let exclude: HashSet<i64> = self
            .evaluate_nodes(&def.exclude, false, &mut notes, 0)
            .into_iter()
            .collect();
        let dnc: HashSet<i64> = self.dnc_ids().into_iter().collect();
        include
            .iter()
            .filter(|id| !exclude.contains(id) && !dnc.contains(id))
            .count() as i64
    }

    fn evaluate_nodes(
        &self,
        nodes: &[SegmentNode],
        intersect: bool,
        notes: &mut Vec<String>,
        depth: u32,
    ) -> Vec<i64> {
        if depth > 16 {
            notes.push("Condition tree rejected: nesting deeper than supported.".to_string());
            return Vec::new();
        }
        if nodes.is_empty() {
            return if intersect {
                self.all_customer_ids()
            } else {
                Vec::new()
            };
        }
        let mut acc: Option<Vec<i64>> = None;
        for node in nodes {
            let ids = self.evaluate_node(node, notes, depth);
            match &mut acc {
                None => acc = Some(ids),
                Some(a) if intersect => {
                    let set: HashSet<i64> = ids.iter().copied().collect();
                    a.retain(|id| set.contains(id));
                }
                Some(a) => {
                    let set: HashSet<i64> = a.iter().copied().collect();
                    for id in ids {
                        if !set.contains(&id) {
                            a.push(id);
                        }
                    }
                }
            }
        }
        acc.unwrap_or_default()
    }

    fn evaluate_node(&self, node: &SegmentNode, notes: &mut Vec<String>, depth: u32) -> Vec<i64> {
        match node {
            SegmentNode::Group {
                combinator,
                children,
            } => self.evaluate_nodes(children, combinator == "all", notes, depth + 1),
            SegmentNode::CustomerProperty { .. } => self.eval_customer_property(node),
            SegmentNode::Contact { .. } => self.eval_contact(node),
            SegmentNode::Ticket { .. } => self.eval_ticket(node, notes),
            SegmentNode::History { .. } => self.eval_history(node),
            SegmentNode::HistoryTag { .. } => self.eval_history_tag(node),
            SegmentNode::OrganizationProperty { .. } => self.eval_organization_property(node),
            SegmentNode::HistoryIssue { .. } => self.eval_history_issue(node),
            SegmentNode::IncidentExposure { .. } => self.eval_incident_exposure(node),
            SegmentNode::CampaignHistory { .. } => self.eval_campaign_history(node),
            SegmentNode::SupportHealth { .. } => self.eval_support_health(node),
            SegmentNode::CustomObjectLink { .. } => self.eval_custom_object_link(node),
            SegmentNode::CustomerEvent { .. } => self.eval_customer_event(node),
        }
    }

    fn all_customer_ids(&self) -> Vec<i64> {
        self.ids("SELECT id FROM customers WHERE deleted_at IS NULL", &[])
    }

    fn dnc_ids(&self) -> Vec<i64> {
        self.ids("SELECT customer_id FROM do_not_contact", &[])
    }

    fn ids(&self, sql: &str, params: &[rusqlite::types::Value]) -> Vec<i64> {
        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    sql = sql.chars().take(300).collect::<String>(),
                    "SegmentEngine::ids: prepare failed — returning empty result"
                );
                return Vec::new();
            }
        };
        let refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();
        stmt.query_map(refs.as_slice(), |r| r.get::<_, i64>(0))
            .map(|rows| rows.filter_map(|x| x.ok()).collect())
            .unwrap_or_else(|e| {
                tracing::warn!(
                    error = %e,
                    sql = sql.chars().take(300).collect::<String>(),
                    "SegmentEngine::ids: query_map failed — returning empty result"
                );
                Vec::new()
            })
    }

    fn primary_email(&self, cid: i64) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM customer_emails
                  WHERE customer_id = ?1 AND value IS NOT NULL AND value <> ''
                  ORDER BY CASE WHEN LOWER(COALESCE(type, '')) LIKE '%work%' THEN 0 ELSE 1 END, id
                  LIMIT 1",
                params![cid],
                |r| r.get::<_, String>(0),
            )
            .ok()
    }

    fn build_rows(
        &self,
        matched: &[i64],
        def: &SegmentDefinition,
        page: i64,
        page_size: i64,
        exclude: &HashSet<i64>,
        dnc: &HashSet<i64>,
    ) -> Vec<Value> {
        let start = ((page - 1).max(0) * page_size) as usize;
        let slice: Vec<i64> = matched
            .iter()
            .skip(start)
            .take(page_size.max(0) as usize)
            .copied()
            .collect();
        let def_names: HashMap<i64, String> = self
            .conn
            .prepare("SELECT id, name FROM customer_property_definitions")
            .and_then(|mut s| {
                let rows =
                    s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                Ok(rows.filter_map(|x| x.ok()).collect::<HashMap<_, _>>())
            })
            .unwrap_or_default();
        slice
            .iter()
            .map(|&cid| {
                let emails: Vec<String> = self
                    .conn
                    .prepare("SELECT value FROM customer_emails WHERE customer_id = ?1 AND value IS NOT NULL ORDER BY id")
                    .and_then(|mut s| {
                        let rows = s.query_map(params![cid], |r| r.get::<_, String>(0))?;
                        Ok(rows.filter_map(|x| x.ok()).collect())
                    })
                    .unwrap_or_default();
                let properties: Vec<Value> = self
                    .conn
                    .prepare("SELECT definition_id, value FROM customer_properties WHERE customer_id = ?1")
                    .and_then(|mut s| {
                        let rows = s.query_map(params![cid], |r| {
                            Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?))
                        })?;
                        Ok(rows.filter_map(|x| x.ok()).collect::<Vec<(i64, Option<String>)>>())
                    })
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|(did, v)| {
                        let v = v?;
                        if v.is_empty() { None } else {
                            Some(json!({
                                "name": def_names.get(&did).cloned()
                                    .unwrap_or_else(|| format!("property #{did}")),
                                "value": v,
                            }))
                        }
                    })
                    .collect();
                let why = self.explain_for(cid, def);
                let matching_tickets = self.collect_matching_tickets(cid, def);
                let excluded = exclude.contains(&cid) || dnc.contains(&cid);
                let cust = self
                    .conn
                    .query_row(
                        "SELECT c.id, c.remote_id, c.first_name, c.last_name, c.job_title,
                                o.name AS organization,
                                (SELECT COUNT(*) FROM conversations cv
                                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS total_tickets,
                                (SELECT COUNT(*) FROM conversations cv
                                  WHERE cv.customer_local_id = c.id AND cv.status = 'active'
                                    AND cv.deleted_at IS NULL) AS open_tickets,
                                (SELECT MAX(COALESCE(cv.last_activity_at, cv.remote_created_at, cv.created_at))
                                   FROM conversations cv
                                  WHERE cv.customer_local_id = c.id AND cv.deleted_at IS NULL) AS last_contact
                           FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id
                          WHERE c.id = ?1",
                        params![cid],
                        |r| {
                            Ok((
                                r.get::<_, i64>(1)?,
                                r.get::<_, Option<String>>(2)?,
                                r.get::<_, Option<String>>(3)?,
                                r.get::<_, Option<String>>(4)?,
                                r.get::<_, Option<String>>(5)?,
                                r.get::<_, i64>(6)?,
                                r.get::<_, i64>(7)?,
                                r.get::<_, Option<String>>(8)?,
                            ))
                        },
                    )
                    .ok();
                match cust {
                    None => json!({
                        "customer_local_id": cid,
                        "customer_remote_id": 0,
                        "first_name": null,
                        "last_name": null,
                        "emails": emails,
                        "chosen_email": self.primary_email(cid),
                        "organization": null,
                        "job_title": null,
                        "properties": properties,
                        "open_tickets": 0,
                        "total_tickets": 0,
                        "last_contact": null,
                        "why": why,
                        "matching_tickets": matching_tickets,
                        "excluded": excluded,
                        "exclusion_reason": if excluded {
                            Value::from("matched an exclusion condition")
                        } else {
                            Value::Null
                        },
                    }),
                    Some((remote_id, first_name, last_name, job_title, organization, total, open, last_contact)) => json!({
                        "customer_local_id": cid,
                        "customer_remote_id": remote_id,
                        "first_name": first_name,
                        "last_name": last_name,
                        "emails": emails,
                        "chosen_email": self.primary_email(cid),
                        "organization": organization,
                        "job_title": job_title,
                        "properties": properties,
                        "open_tickets": open,
                        "total_tickets": total,
                        "last_contact": last_contact,
                        "why": why,
                        "matching_tickets": matching_tickets,
                        "excluded": excluded,
                        "exclusion_reason": if excluded {
                            Value::from("matched an exclusion condition")
                        } else {
                            Value::Null
                        },
                    }),
                }
            })
            .collect()
    }

    fn explain_for(&self, cid: i64, def: &SegmentDefinition) -> Vec<Value> {
        let mut out: Vec<Value> = Vec::new();
        fn walk(engine: &SegmentEngine<'_>, cid: i64, nodes: &[SegmentNode], out: &mut Vec<Value>) {
            for n in nodes {
                match n {
                    SegmentNode::Group { children, .. } => walk(engine, cid, children, out),
                    SegmentNode::CustomerProperty {
                        definition_id,
                        name,
                        op,
                        value: _,
                        values,
                        ..
                    } => {
                        let row: Option<Option<String>> = engine
                            .conn
                            .query_row(
                                "SELECT cp.value FROM customer_properties cp
                                  WHERE cp.customer_id = ?1 AND cp.definition_id = ?2",
                                params![cid, definition_id],
                                |r| r.get::<_, Option<String>>(0),
                            )
                            .ok();
                        let Some(v) = row.flatten() else { continue };
                        if v.is_empty() {
                            continue;
                        }
                        if op == "is_empty" {
                            continue;
                        }
                        if op == "is_any_of" || op == "is_none_of" {
                            out.push(json!({
                                "text": format!(
                                    "{}: \"{}\" {} {}",
                                    name, v,
                                    if op == "is_any_of" { "is one of" } else { "is none of" },
                                    values.join(", ")),
                            }));
                        } else {
                            out.push(json!({ "text": format!("{} = {}", name, v) }));
                        }
                    }
                    SegmentNode::Contact { field, op, value } => match field.as_str() {
                        "email_domain" => {
                            if let Some(email) = engine.primary_email(cid) {
                                let domain = email.split('@').nth(1).unwrap_or("");
                                out.push(json!({ "text": format!("email domain: {domain}") }));
                            }
                        }
                        "email" => {
                            if let Some(email) = engine.primary_email(cid) {
                                out.push(json!({
                                    "text": format!("email {} {}",
                                        describe_op(op),
                                        value.clone().unwrap_or(email)),
                                }));
                            }
                        }
                        "organization" => {
                            let org: Option<String> = engine
                                .conn
                                .query_row(
                                    "SELECT o.name FROM customers c
                                          LEFT JOIN organizations o ON o.id = c.organization_id
                                         WHERE c.id = ?1",
                                    params![cid],
                                    |r| r.get::<_, Option<String>>(0),
                                )
                                .ok()
                                .flatten();
                            if let Some(org) = org {
                                out.push(json!({
                                    "text": format!("organization {} {}",
                                        describe_op(op),
                                        value.clone().unwrap_or(org)),
                                }));
                            }
                        }
                        "has_email" | "has_phone" | "has_multiple_emails" => {
                            out.push(json!({ "text": describe_contact_presence(field) }));
                        }
                        other => {
                            let col = match other {
                                "name" => "name",
                                "job_title" => "job title",
                                "location" => "location",
                                "background" => "background",
                                f => f,
                            };
                            out.push(json!({
                                "text": format!("{} {} {}",
                                    col, describe_op(op), value.clone().unwrap_or_default()),
                            }));
                        }
                    },
                    SegmentNode::Ticket {
                        tags,
                        tag_mode,
                        statuses,
                        mailbox_ids,
                        created_within_days,
                        modified_within_days,
                        ..
                    } => {
                        let tickets = engine.tickets_for_customer_matching(cid, n);
                        if !tickets.is_empty() {
                            let mut bits: Vec<String> = Vec::new();
                            let tags_clean: Vec<String> = tags
                                .iter()
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                                .collect();
                            if !tags_clean.is_empty() {
                                let mode = match tag_mode.as_str() {
                                    "all" => "ALL of",
                                    "none" => "NONE of",
                                    _ => "ANY of",
                                };
                                bits.push(format!("has {mode}: {}", tags_clean.join(", ")));
                            }
                            let statuses_clean: Vec<String> =
                                statuses.iter().filter(|s| !s.is_empty()).cloned().collect();
                            if !statuses_clean.is_empty() {
                                bits.push(format!("status: {}", statuses_clean.join("/")));
                            }
                            if !mailbox_ids.is_empty() {
                                bits.push("inbox filtered".to_string());
                            }
                            if let Some(d) = created_within_days {
                                bits.push(format!("created within {d} days"));
                            }
                            if let Some(d) = modified_within_days {
                                bits.push(format!("modified within {d} days"));
                            }
                            let text = if bits.is_empty() {
                                "Ticket match".to_string()
                            } else {
                                format!("Ticket {}", bits.join(" + "))
                            };
                            out.push(json!({ "text": text, "tickets": tickets }));
                        } else {
                            out.push(json!({ "text": "Ticket condition satisfied" }));
                        }
                    }
                    SegmentNode::History { metric, op, value } => {
                        out.push(json!({ "text": describe_history(metric, op, *value) }));
                    }
                    SegmentNode::HistoryTag { tag, within_days } => {
                        let mut s = format!("has a ticket tagged \"{tag}\"");
                        if let Some(d) = within_days {
                            s.push_str(&format!(" within {d} days"));
                        }
                        out.push(json!({ "text": s }));
                    }
                    SegmentNode::OrganizationProperty {
                        field,
                        definition_id,
                        name,
                        op,
                        value,
                        ..
                    } => {
                        let field_name = match field.as_deref() {
                            Some("name") => "organization name".to_string(),
                            Some("domains") => "organization domains".to_string(),
                            _ => match (name, definition_id) {
                                (Some(n), _) => n.clone(),
                                (None, Some(id)) => format!("organization property #{id}"),
                                (None, None) => "organization property #?".to_string(),
                            },
                        };
                        if op == "is_empty" || op == "is_not_empty" {
                            out.push(
                                json!({ "text": format!("{} {}", field_name, describe_op(op)) }),
                            );
                        } else {
                            out.push(json!({
                                "text": format!("{} {} {}", field_name, describe_op(op),
                                    value.clone().unwrap_or_default()),
                            }));
                        }
                    }
                    SegmentNode::HistoryIssue {
                        issue_kind,
                        issue_local_id,
                        ..
                    } => {
                        let label = if issue_kind == "cluster" {
                            "issue cluster"
                        } else {
                            "known issue"
                        };
                        let s = match issue_local_id {
                            Some(id) => format!("linked to {label} #{id}"),
                            None => format!("linked to at least one {label}"),
                        };
                        out.push(json!({ "text": s }));
                    }
                    SegmentNode::IncidentExposure {
                        incident_id,
                        within_days,
                    } => {
                        let mut s = match incident_id {
                            Some(id) => format!("exposed to incident #{id}"),
                            None => "exposed to an active incident".to_string(),
                        };
                        if let Some(d) = within_days {
                            s.push_str(&format!(" within {d} days"));
                        }
                        out.push(json!({ "text": s }));
                    }
                    SegmentNode::CampaignHistory {
                        relation,
                        campaign_id,
                    } => {
                        let rel = match relation.as_str() {
                            "received" => "received an outreach campaign",
                            "replied" => "replied to an outreach campaign",
                            _ => "never received an outreach campaign",
                        };
                        let s = match campaign_id {
                            Some(id) => format!("{rel} (#{id})"),
                            None => rel.to_string(),
                        };
                        out.push(json!({ "text": s }));
                    }
                    SegmentNode::SupportHealth { metric, op, value } => {
                        out.push(json!({
                            "text": format!("support health: {} {} {}",
                                metric.replace('_', " "),
                                if op == "gte" { "at least" } else { "at most" },
                                value),
                        }));
                    }
                    SegmentNode::CustomObjectLink { type_id } => {
                        let s = match type_id {
                            Some(id) => format!("linked to a custom object of type #{id}"),
                            None => "linked to a custom object".to_string(),
                        };
                        out.push(json!({ "text": s }));
                    }
                    SegmentNode::CustomerEvent {
                        event_kind,
                        within_days,
                    } => {
                        let mut s = format!(
                            "timeline includes a \"{}\" event",
                            event_kind.replace('_', " ")
                        );
                        if let Some(d) = within_days {
                            s.push_str(&format!(" within {d} days"));
                        }
                        out.push(json!({ "text": s }));
                    }
                }
            }
        }
        walk(self, cid, &def.conditions, &mut out);
        out
    }

    fn tickets_for_customer_matching(&self, cid: i64, t: &SegmentNode) -> Vec<Value> {
        let SegmentNode::Ticket {
            tags,
            tag_mode,
            statuses,
            created_within_days,
            modified_within_days,
            ..
        } = t
        else {
            return Vec::new();
        };
        let mut where_clauses = vec![
            "c.deleted_at IS NULL".to_string(),
            "c.customer_local_id = ?N_CID".to_string(),
        ];
        let mut p: SqlParams = vec![integer(cid)];
        let statuses: Vec<String> = statuses.iter().filter(|s| !s.is_empty()).cloned().collect();
        if !statuses.is_empty() {
            let ph = (0..statuses.len())
                .map(|i| format!("?N_ST{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            where_clauses.push(format!("c.status IN ({ph})"));
            p.extend(statuses.iter().map(|s| text(s)));
        }
        if let Some(days) = created_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?N_CD)"
                    .to_string(),
            );
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(days) = modified_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_updated_at, c.updated_at, c.created_at)) >= julianday('now', ?N_MD)".to_string());
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        let tags_clean: Vec<String> = tags
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let tag_mode = if tag_mode.is_empty() {
            "any"
        } else {
            tag_mode.as_str()
        };
        if !tags_clean.is_empty() {
            let ph = (0..tags_clean.len())
                .map(|i| format!("?N_TG{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let clause = if tag_mode == "all" {
                format!(
                    "(SELECT COUNT(DISTINCT LOWER(tg.name)) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph})) = {}",
                    tags_clean.len()
                )
            } else if tag_mode == "none" {
                format!(
                    "NOT EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                )
            } else {
                format!(
                    "EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                )
            };
            where_clauses.push(clause);
            p.extend(tags_clean.iter().map(|t| text(&lower(t))));
        }
        let sql = format!(
            "SELECT c.id, c.number, c.subject, c.status, COALESCE(c.remote_created_at, c.created_at),
                    (SELECT GROUP_CONCAT(tg.name) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id) AS tags
               FROM conversations c WHERE {}
              ORDER BY COALESCE(c.remote_created_at, c.created_at) DESC LIMIT 20",
            where_clauses.join(" AND "),
        );
        let sql = renumber_placeholders(&sql, &mut 0);
        let Ok(mut stmt) = self.conn.prepare(&sql) else {
            return Vec::new();
        };
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .map(|rows| rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
            .unwrap_or_default();
        rows.into_iter()
            .map(|(id, number, subject, status, created_at, tags)| json!({
                "conversationId": id,
                "number": number,
                "subject": subject,
                "status": status,
                "tags": tags.map(|t| t.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect::<Vec<_>>())
                    .unwrap_or_default(),
                "createdAt": created_at,
            }))
            .collect()
    }

    fn collect_matching_tickets(&self, cid: i64, def: &SegmentDefinition) -> Vec<Value> {
        let mut seen: HashMap<i64, Value> = HashMap::new();
        fn walk(
            engine: &SegmentEngine<'_>,
            cid: i64,
            nodes: &[SegmentNode],
            seen: &mut HashMap<i64, Value>,
        ) {
            for n in nodes {
                match n {
                    SegmentNode::Group { children, .. } => walk(engine, cid, children, seen),
                    SegmentNode::Ticket { .. } => {
                        for t in engine.tickets_for_customer_matching(cid, n) {
                            let id = t
                                .get("conversationId")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            seen.entry(id).or_insert(t);
                        }
                    }
                    _ => {}
                }
            }
        }
        walk(self, cid, &def.conditions, &mut seen);
        walk(self, cid, &def.exclude, &mut seen);
        let mut out: Vec<Value> = seen.into_values().collect();
        out.sort_by(|a, b| {
            let ka = a.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
            let kb = b.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
            kb.cmp(ka)
        });
        out
    }

    fn eval_customer_property(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomerProperty {
            definition_id,
            op,
            value,
            value2,
            values,
            ..
        } = n
        else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        match op.as_str() {
            "is_empty" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND cp.value IS NOT NULL AND cp.value <> '')",
                &[integer(*definition_id)],
            ),
            "is_not_empty" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value IS NOT NULL AND cp.value <> ''",
                &[integer(*definition_id)],
            ),
            "equals" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND LOWER(cp.value) = LOWER(?2)",
                &[integer(*definition_id), text(&val)],
            ),
            "not_equals" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND LOWER(cp.value) = LOWER(?2))",
                &[integer(*definition_id), text(&val)],
            ),
            "contains" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("%{}%", escape_like(&val))),
                ],
            ),
            "not_contains" => self.ids(
                "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                 (SELECT cp.customer_id FROM customer_properties cp
                   WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\')",
                &[
                    integer(*definition_id),
                    text(&format!("%{}%", escape_like(&val))),
                ],
            ),
            "starts_with" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("{}%", escape_like(&val))),
                ],
            ),
            "ends_with" => self.ids(
                "SELECT cp.customer_id AS cid FROM customer_properties cp
                  WHERE cp.definition_id = ?1 AND cp.value LIKE ?2 ESCAPE '\\'",
                &[
                    integer(*definition_id),
                    text(&format!("%{}", escape_like(&val))),
                ],
            ),
            "gt" | "gte" | "lt" | "lte" => {
                let Some(num) = val.parse::<f64>().ok() else {
                    return Vec::new();
                };
                let op_sql = match op.as_str() {
                    "gt" => ">",
                    "gte" => ">=",
                    "lt" => "<",
                    _ => "<=",
                };
                self.ids(
                    &format!(
                        "SELECT cp.customer_id AS cid FROM customer_properties cp
                               WHERE cp.definition_id = ?1 AND CAST(cp.value AS REAL) {op_sql} ?2"
                    ),
                    &[integer(*definition_id), real(num)],
                )
            }
            "between" => {
                let (Some(a), b) = (
                    val.parse::<f64>().ok(),
                    value2.as_deref().unwrap_or("").to_string(),
                ) else {
                    return Vec::new();
                };
                let Some(b) = b.trim().parse::<f64>().ok() else {
                    return Vec::new();
                };
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND CAST(cp.value AS REAL) BETWEEN ?2 AND ?3",
                    &[integer(*definition_id), real(a.min(b)), real(a.max(b))],
                )
            }
            "before" => {
                if val.is_empty() {
                    return Vec::new();
                }
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND cp.value < ?2",
                    &[integer(*definition_id), text(&val)],
                )
            }
            "after" => {
                if val.is_empty() {
                    return Vec::new();
                }
                self.ids(
                    "SELECT cp.customer_id AS cid FROM customer_properties cp
                      WHERE cp.definition_id = ?1 AND cp.value > ?2",
                    &[integer(*definition_id), text(&val)],
                )
            }
            "is_any_of" => {
                let list: Vec<String> = values
                    .iter()
                    .map(|s| lower(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                if list.is_empty() {
                    return Vec::new();
                }
                let mut p = vec![integer(*definition_id)];
                p.extend(list.iter().map(|s| text(s)));
                self.ids(
                    &format!(
                        "SELECT cp.customer_id AS cid FROM customer_properties cp
                               WHERE cp.definition_id = ?1 AND LOWER(cp.value) IN ({})",
                        placeholders(list.len())
                    ),
                    &p,
                )
            }
            "is_none_of" => {
                let list: Vec<String> = values
                    .iter()
                    .map(|s| lower(s))
                    .filter(|s| !s.is_empty())
                    .collect();
                if list.is_empty() {
                    return self.all_customer_ids();
                }
                let mut p = vec![integer(*definition_id)];
                p.extend(list.iter().map(|s| text(s)));
                self.ids(
                    &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                               (SELECT cp.customer_id FROM customer_properties cp
                                 WHERE cp.definition_id = ?1 AND LOWER(cp.value) IN ({}))",
                             placeholders(list.len())),
                    &p,
                )
            }
            _ => Vec::new(),
        }
    }

    fn eval_contact(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::Contact { field, op, value } = n else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        match field.as_str() {
            "has_email" | "has_phone" => {
                let table = if field == "has_email" {
                    "customer_emails"
                } else {
                    "customer_phones"
                };
                if op == "is_empty" {
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL
                                   AND NOT EXISTS (SELECT 1 FROM {table} x WHERE x.customer_id = c.id)"),
                        &[],
                    )
                } else {
                    self.ids(
                        &format!("SELECT DISTINCT x.customer_id AS cid FROM {table} x
                                   JOIN customers c ON c.id = x.customer_id WHERE c.deleted_at IS NULL"),
                        &[],
                    )
                }
            }
            "has_multiple_emails" => self.ids(
                "SELECT ce.customer_id AS cid FROM customer_emails ce
                   JOIN customers c ON c.id = ce.customer_id WHERE c.deleted_at IS NULL
                  GROUP BY ce.customer_id HAVING COUNT(*) > 1",
                &[],
            ),
            "email" => {
                if op == "is_empty" || op == "is_not_empty" {
                    return if op == "is_empty" {
                        self.ids(
                            "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL
                              AND NOT EXISTS (SELECT 1 FROM customer_emails ce WHERE ce.customer_id = c.id)",
                            &[],
                        )
                    } else {
                        self.ids(
                            "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                               JOIN customers c ON c.id = ce.customer_id WHERE c.deleted_at IS NULL",
                            &[],
                        )
                    };
                }
                if val.is_empty() {
                    return Vec::new();
                }
                match op.as_str() {
                    "equals" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND LOWER(ce.value) = LOWER(?1)",
                        &[text(&val)],
                    ),
                    "not_equals" => self.ids(
                        "SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND c.id NOT IN
                           (SELECT ce.customer_id FROM customer_emails ce WHERE LOWER(ce.value) = LOWER(?1))",
                        &[text(&val)],
                    ),
                    "contains" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("%{}%", escape_like(&val)))],
                    ),
                    "starts_with" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("{}%", escape_like(&val)))],
                    ),
                    "ends_with" => self.ids(
                        "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                           JOIN customers c ON c.id = ce.customer_id
                          WHERE c.deleted_at IS NULL AND ce.value LIKE ?1 ESCAPE '\\'",
                        &[text(&format!("%{}", escape_like(&val)))],
                    ),
                    _ => Vec::new(),
                }
            }
            "email_domain" => {
                let domain = val.trim_start_matches('@').to_lowercase();
                if domain.is_empty() {
                    return Vec::new();
                }
                self.ids(
                    "SELECT DISTINCT ce.customer_id AS cid FROM customer_emails ce
                       JOIN customers c ON c.id = ce.customer_id
                      WHERE c.deleted_at IS NULL AND LOWER(ce.value) LIKE ?1 ESCAPE '\\'",
                    &[text(&format!("%@{}%", escape_like(&domain)))],
                )
            }
            _ => {
                let expr = match field.as_str() {
                    "name" => "(COALESCE(c.first_name,'') || ' ' || COALESCE(c.last_name,''))",
                    "organization" => "o.name",
                    "job_title" => "c.job_title",
                    "location" => "c.location",
                    "background" => "c.background",
                    _ => return Vec::new(),
                };
                let from = "FROM customers c LEFT JOIN organizations o ON o.id = c.organization_id";
                match op.as_str() {
                    "is_empty" => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND ({expr} IS NULL OR TRIM({expr}) = '')"),
                        &[],
                    ),
                    "is_not_empty" => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND ({expr} IS NOT NULL AND TRIM({expr}) <> '')"),
                        &[],
                    ),
                    "equals" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND LOWER({expr}) = LOWER(?1)"),
                        &[text(&val)],
                    ),
                    "not_equals" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND (LOWER({expr}) <> LOWER(?1) OR {expr} IS NULL)"),
                        &[text(&val)],
                    ),
                    "contains" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("%{}%", escape_like(&val)))],
                    ),
                    "starts_with" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("{}%", escape_like(&val)))],
                    ),
                    "ends_with" if !val.is_empty() => self.ids(
                        &format!("SELECT c.id AS cid {from} WHERE c.deleted_at IS NULL AND {expr} LIKE ?1 ESCAPE '\\'"),
                        &[text(&format!("%{}", escape_like(&val)))],
                    ),
                    _ => Vec::new(),
                }
            }
        }
    }

    fn eval_ticket(&self, n: &SegmentNode, notes: &mut Vec<String>) -> Vec<i64> {
        let SegmentNode::Ticket {
            tags,
            tag_mode,
            statuses,
            mailbox_ids,
            assignee_ids,
            created_within_days,
            modified_within_days,
            number_min,
            number_max,
            ai_attribute,
            custom_fields,
            channel,
        } = n
        else {
            return Vec::new();
        };

        let mut where_clauses: Vec<String> = vec![
            "c.deleted_at IS NULL".to_string(),
            "c.customer_local_id IS NOT NULL".to_string(),
        ];
        let mut p: SqlParams = Vec::new();

        let statuses: Vec<String> = statuses.iter().filter(|s| !s.is_empty()).cloned().collect();
        if !statuses.is_empty() {
            where_clauses.push(format!("c.status IN ({})", placeholders(statuses.len())));
            p.extend(statuses.iter().map(|s| text(s)));
        }
        let mailboxes: Vec<i64> = mailbox_ids.iter().copied().filter(|id| *id > 0).collect();
        if !mailboxes.is_empty() {
            where_clauses.push(format!(
                "c.mailbox_local_id IN ({})",
                placeholders(mailboxes.len())
            ));
            p.extend(mailboxes.iter().map(|id| integer(*id)));
        }
        let assignees: Vec<i64> = assignee_ids.clone();
        if !assignees.is_empty() {
            let unassigned = assignees.contains(&-1);
            let listed: Vec<i64> = assignees.iter().copied().filter(|a| *a != -1).collect();
            if unassigned && !listed.is_empty() {
                where_clauses.push(format!(
                    "(c.assignee_local_id IS NULL OR c.assignee_local_id IN ({}))",
                    placeholders(listed.len())
                ));
                p.extend(listed.iter().map(|id| integer(*id)));
            } else if unassigned {
                where_clauses.push("c.assignee_local_id IS NULL".to_string());
            } else {
                where_clauses.push(format!(
                    "c.assignee_local_id IN ({})",
                    placeholders(listed.len())
                ));
                p.extend(listed.iter().map(|id| integer(*id)));
            }
        }
        if let Some(ch) = channel {
            let ch = ch.trim();
            if !ch.is_empty() {
                where_clauses.push("LOWER(COALESCE(c.source_type, '')) = LOWER(?N_CH)".to_string());
                p.push(text(ch));
            }
        }
        if let Some(days) = created_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?N_CD)"
                    .to_string(),
            );
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(days) = modified_within_days.filter(|d| d.is_finite()) {
            where_clauses.push(
                "julianday(COALESCE(c.remote_updated_at, c.updated_at, c.created_at)) >= julianday('now', ?N_MD)".to_string());
            p.push(text(&format!("-{} days", days.max(0.0) as i64)));
        }
        if let Some(nmin) = number_min.filter(|d| d.is_finite()) {
            where_clauses.push("c.number >= ?N_MIN".to_string());
            p.push(real(nmin));
        }
        if let Some(nmax) = number_max.filter(|d| d.is_finite()) {
            where_clauses.push("c.number <= ?N_MAX".to_string());
            p.push(real(nmax));
        }

        let tags_clean: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        let tag_mode = if tag_mode.is_empty() {
            "any"
        } else {
            tag_mode.as_str()
        };
        if !tags_clean.is_empty() {
            let ph = placeholders(tags_clean.len());
            if tag_mode == "all" {
                where_clauses.push(format!(
                    "(SELECT COUNT(DISTINCT LOWER(tg.name)) FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph})) = {}",
                    tags_clean.len()
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
                if tags_clean.len() > 1 {
                    notes.push(format!(
                        "Tag mode ALL requires one single conversation carrying all {} tags (conversation-level intersection).",
                        tags_clean.len()));
                }
            } else if tag_mode == "none" {
                where_clauses.push(format!(
                    "NOT EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
            } else {
                where_clauses.push(format!(
                    "EXISTS (SELECT 1 FROM conversation_tags ct
                       JOIN tags tg ON tg.id = ct.tag_id
                      WHERE ct.conversation_id = c.id AND LOWER(tg.name) IN ({ph}))"
                ));
                p.extend(tags_clean.iter().map(|t| text(&lower(t))));
            }
        }

        if let Some(aa) = ai_attribute {
            let Some(def) = crate::catalog::AiAttributeKey::parse(&aa.attribute) else {
                notes
                    .push(format!(
                    "AI attribute '{}' is not in the catalog - condition matched no conversations.",
                    aa.attribute));
                return Vec::new();
            };
            if aa.value.to_lowercase() == "unknown" {
                if aa.op != "equals" {
                    return Vec::new();
                }
                where_clauses.push(
                    "NOT EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                       AND a.attribute = ?N_AA AND a.superseded_at IS NULL)"
                        .to_string(),
                );
                p.push(text(&aa.attribute));
            } else if def.value_type() == crate::catalog::AttributeValueType::Number {
                let Ok(num) = aa.value.parse::<f64>() else {
                    return Vec::new();
                };
                let Some(op_sql) = (match aa.op.as_str() {
                    "gt" => Some(">"),
                    "gte" => Some(">="),
                    "lt" => Some("<"),
                    "lte" => Some("<="),
                    _ => None,
                }) else {
                    return Vec::new();
                };
                where_clauses.push(format!(
                    "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                       AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                       AND CAST(a.value AS REAL) {op_sql} ?N_AA2)"
                ));
                p.push(text(&aa.attribute));
                p.push(real(num));
            } else {
                match aa.op.as_str() {
                    "equals" => {
                        where_clauses.push(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND LOWER(a.value) = LOWER(?N_AA2))"
                                .to_string(),
                        );
                        p.push(text(&aa.attribute));
                        p.push(text(&aa.value));
                    }
                    "not_equals" => {
                        where_clauses.push(
                            "NOT EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND LOWER(a.value) = LOWER(?N_AA2))".to_string());
                        p.push(text(&aa.attribute));
                        p.push(text(&aa.value));
                    }
                    "contains" => {
                        where_clauses.push(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND a.value LIKE ?N_AA2 ESCAPE '\\')"
                                .to_string(),
                        );
                        p.push(text(&aa.attribute));
                        p.push(text(&format!("%{}%", escape_like(&aa.value))));
                    }
                    "gt" | "gte" | "lt" | "lte" => {
                        let vocab: Vec<String> =
                            def.values().iter().map(|v| v.to_string()).collect();
                        let Some(idx) = vocab.iter().position(|v| *v == aa.value) else {
                            return Vec::new();
                        };
                        let list: Vec<String> = if aa.op == "gt" || aa.op == "gte" {
                            vocab[if aa.op == "gt" { idx + 1 } else { idx }..].to_vec()
                        } else {
                            vocab[..if aa.op == "lt" { idx } else { idx + 1 }].to_vec()
                        };
                        if list.is_empty() {
                            return Vec::new();
                        }
                        where_clauses.push(format!(
                            "EXISTS (SELECT 1 FROM ai_attributes a WHERE a.conversation_id = c.id
                               AND a.attribute = ?N_AA1 AND a.superseded_at IS NULL
                               AND a.value IN ({}))",
                            placeholders(list.len())
                        ));
                        p.push(text(&aa.attribute));
                        p.extend(list.iter().map(|v| text(v)));
                    }
                    _ => return Vec::new(),
                }
            }
            notes.push(format!(
                "AI attribute '{}' {} '{}' evaluated over current local attribute rows (missing = unknown).",
                def.label(), aa.op, aa.value));
        }

        for cf in custom_fields {
            if cf.field_local_id <= 0 {
                continue;
            }
            let exists: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM inbox_fields WHERE id = ?1",
                    params![cf.field_local_id],
                    |r| r.get(0),
                )
                .ok();
            if exists.is_none() {
                notes
                    .push(format!(
                    "Custom field #{} does not exist locally - condition matched no conversations.",
                    cf.field_local_id));
                return Vec::new();
            }
            let val = cf.value.as_deref().unwrap_or("").trim().to_string();
            match cf.op.as_str() {
                "is_empty" => {
                    where_clauses.push(
                        "NOT EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) IS NOT NULL
                           AND COALESCE(f.text_value, f.value) <> '')".to_string());
                    p.push(integer(cf.field_local_id));
                }
                "is_not_empty" => {
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) IS NOT NULL
                           AND COALESCE(f.text_value, f.value) <> '')"
                            .to_string(),
                    );
                    p.push(integer(cf.field_local_id));
                }
                "equals" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND LOWER(COALESCE(f.text_value, f.value)) = LOWER(?N_CF2))".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&val));
                }
                "not_equals" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "NOT EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND LOWER(COALESCE(f.text_value, f.value)) = LOWER(?N_CF2))".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&val));
                }
                "contains" => {
                    if val.is_empty() {
                        return Vec::new();
                    }
                    where_clauses.push(
                        "EXISTS (SELECT 1 FROM conversation_fields f WHERE f.conversation_id = c.id
                           AND f.field_id = ?N_CF1 AND COALESCE(f.text_value, f.value) LIKE ?N_CF2 ESCAPE '\\')".to_string());
                    p.push(integer(cf.field_local_id));
                    p.push(text(&format!("%{}%", escape_like(&val))));
                }
                _ => return Vec::new(),
            }
        }

        let mut sql = format!(
            "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c WHERE {}",
            where_clauses.join(" AND ")
        );
        let mut counter = 0usize;
        sql = renumber_placeholders(&sql, &mut counter);
        self.ids(&sql, &p)
    }

    fn eval_history(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::History { metric, op, value } = n else {
            return Vec::new();
        };
        if !value.is_finite() {
            return Vec::new();
        }
        let cmp = match op.as_str() {
            "gte" => ">=",
            "lte" => "<=",
            _ => "=",
        };
        let count_cond = |status_filter: &str| {
            format!(
                "(SELECT COUNT(*) FROM conversations cv WHERE cv.customer_local_id = c.id
                       AND cv.deleted_at IS NULL{status_filter}) {cmp} ?1"
            )
        };
        match metric.as_str() {
            "ticket_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond("")),
                &[real(*value)],
            ),
            "open_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond(" AND cv.status = 'active'")),
                &[real(*value)],
            ),
            "closed_count" => self.ids(
                &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND {}",
                         count_cond(" AND cv.status = 'closed'")),
                &[real(*value)],
            ),
            "last_contact_within_days" => self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN customers cu ON cu.id = c.customer_local_id
                  WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL
                    AND julianday(COALESCE(c.last_activity_at, c.remote_created_at, c.created_at))
                        >= julianday('now', ?1)",
                &[text(&format!("-{} days", value.max(0.0) as i64))],
            ),
            "first_contact_before_days" => self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN customers cu ON cu.id = c.customer_local_id
                  WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL
                    AND julianday(COALESCE(c.remote_created_at, c.created_at)) < julianday('now', ?1)",
                &[text(&format!("-{} days", value.max(0.0) as i64))],
            ),
            "waited_over_hours_count" => {
                let over = self.ids(
                    "SELECT c.customer_local_id AS cid FROM conversations c
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL
                        AND julianday(COALESCE(c.closed_at, datetime('now')))
                            - julianday(COALESCE(c.last_customer_reply_at, c.first_customer_message_at,
                                                 c.remote_created_at, c.created_at)) > ?1",
                    &[real(value.max(0.0) / 24.0)],
                );
                match op.as_str() {
                    "gte" => over,
                    "lte" => {
                        let set: HashSet<i64> = over.into_iter().collect();
                        self.all_customer_ids().into_iter().filter(|id| !set.contains(id)).collect()
                    }
                    _ => Vec::new(),
                }
            }
            _ => Vec::new(),
        }
    }

    fn eval_history_tag(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::HistoryTag { tag, within_days } = n else {
            return Vec::new();
        };
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() {
            return Vec::new();
        }
        if let Some(days) = within_days.filter(|d| d.is_finite()) {
            self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN conversation_tags ct ON ct.conversation_id = c.id
                   JOIN tags tg ON tg.id = ct.tag_id
                   JOIN customers cu ON cu.id = c.customer_local_id
                 WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL AND LOWER(tg.name) = ?1
                   AND julianday(COALESCE(c.remote_created_at, c.created_at)) >= julianday('now', ?2)",
                &[text(&tag), text(&format!("-{} days", days.max(0.0) as i64))],
            )
        } else {
            self.ids(
                "SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                   JOIN conversation_tags ct ON ct.conversation_id = c.id
                   JOIN tags tg ON tg.id = ct.tag_id
                   JOIN customers cu ON cu.id = c.customer_local_id
                 WHERE c.deleted_at IS NULL AND cu.deleted_at IS NULL AND LOWER(tg.name) = ?1",
                &[text(&tag)],
            )
        }
    }

    fn eval_organization_property(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::OrganizationProperty {
            field,
            definition_id,
            op,
            value,
            value2,
            values,
            ..
        } = n
        else {
            return Vec::new();
        };
        let val = value.as_deref().unwrap_or("").trim().to_string();
        if field.as_deref() == Some("name") || field.as_deref() == Some("domains") {
            let expr = if field.as_deref() == Some("name") {
                "o.name"
            } else {
                "o.domains"
            };
            match op.as_str() {
                "is_empty" => self.ids(
                    &format!("SELECT c.id AS cid FROM customers c
                               LEFT JOIN organizations o ON o.id = c.organization_id
                              WHERE c.deleted_at IS NULL AND ({expr} IS NULL OR TRIM({expr}) = '')"),
                    &[],
                ),
                "is_not_empty" => self.ids(
                    &format!("SELECT c.id AS cid FROM customers c
                               LEFT JOIN organizations o ON o.id = c.organization_id
                              WHERE c.deleted_at IS NULL AND ({expr} IS NOT NULL AND TRIM({expr}) <> '')"),
                    &[],
                ),
                "equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND LOWER({expr}) LIKE ?N_V"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&lower(&val))))],
                    )
                }
                "not_equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   LEFT JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL
                                    AND ({expr} IS NULL OR LOWER({expr}) NOT LIKE ?N_V)"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&lower(&val))))],
                    )
                }
                "contains" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("%{}%", escape_like(&val)))],
                    )
                }
                "starts_with" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("{}%", escape_like(&val)))],
                    )
                }
                "ends_with" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c
                                   JOIN organizations o ON o.id = c.organization_id
                                  WHERE c.deleted_at IS NULL AND {expr} LIKE ?N_V ESCAPE '\\'"),
            &mut 0),

                        &[text(&format!("%{}", escape_like(&val)))],
                    )
                }
                _ => Vec::new(),
            }
        } else {
            let Some(def_id) = definition_id.filter(|id| *id > 0) else {
                return Vec::new();
            };
            match op.as_str() {
                "is_empty" => self.ids(
                    "SELECT c.id AS cid FROM customers c
                      WHERE c.deleted_at IS NULL
                        AND (c.organization_id IS NULL OR c.organization_id NOT IN
                             (SELECT op.organization_id FROM organization_properties op
                               WHERE op.definition_id = ?1 AND op.value IS NOT NULL AND op.value <> ''))",
                    &[integer(def_id)],
                ),
                "is_not_empty" => self.ids(
                    "SELECT c.id AS cid FROM customers c
                       JOIN organization_properties op ON op.organization_id = c.organization_id
                      WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                        AND op.value IS NOT NULL AND op.value <> ''",
                    &[integer(def_id)],
                ),
                "equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                           JOIN organization_properties op ON op.organization_id = c.organization_id
                          WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                            AND LOWER(op.value) = LOWER(?2)",
                        &[integer(def_id), text(&val)],
                    )
                }
                "not_equals" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                          WHERE c.deleted_at IS NULL
                            AND (c.organization_id IS NULL OR c.organization_id NOT IN
                                 (SELECT op.organization_id FROM organization_properties op
                                   WHERE op.definition_id = ?1 AND LOWER(op.value) = LOWER(?2)))",
                        &[integer(def_id), text(&val)],
                    )
                }
                "contains" => {
                    if val.is_empty() { return Vec::new(); }
                    self.ids(
                        "SELECT c.id AS cid FROM customers c
                           JOIN organization_properties op ON op.organization_id = c.organization_id
                          WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                            AND op.value LIKE ?2 ESCAPE '\\'",
                        &[integer(def_id), text(&format!("%{}%", escape_like(&val)))],
                    )
                }
                "is_any_of" => {
                    let list: Vec<String> = values.iter().map(|s| lower(s)).filter(|s| !s.is_empty()).collect();
                    if list.is_empty() { return Vec::new(); }
                    let mut p = vec![integer(def_id)];
                    p.extend(list.iter().map(|s| text(s)));
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c
                                    JOIN organization_properties op ON op.organization_id = c.organization_id
                                   WHERE c.deleted_at IS NULL AND op.definition_id = ?1
                                     AND LOWER(op.value) IN ({})", placeholders(list.len())),
                        &p,
                    )
                }
                "is_none_of" => {
                    let list: Vec<String> = values.iter().map(|s| lower(s)).filter(|s| !s.is_empty()).collect();
                    if list.is_empty() { return self.all_customer_ids(); }
                    let mut p = vec![integer(def_id)];
                    p.extend(list.iter().map(|s| text(s)));
                    self.ids(
                        &format!("SELECT c.id AS cid FROM customers c
                                   WHERE c.deleted_at IS NULL
                                     AND (c.organization_id IS NULL OR c.organization_id NOT IN
                                          (SELECT op.organization_id FROM organization_properties op
                                            WHERE op.definition_id = ?1 AND LOWER(op.value) IN ({})))",
                                 placeholders(list.len())),
                        &p,
                    )
                }
                "gt" | "gte" | "lt" | "lte" => {
                    let (Some(a), b) = (val.parse::<f64>().ok(), value2.as_deref().unwrap_or("").to_string()) else {
                        return Vec::new();
                    };
                    let _ = b;
                    let op_sql = match op.as_str() {
                        "gt" => ">",
                        "gte" => ">=",
                        "lt" => "<",
                        _ => "<=",
                    };
                    self.ids(
                        &renumber_placeholders(
                            &format!("SELECT c.id AS cid FROM customers c
                                    JOIN organization_properties op ON op.organization_id = c.organization_id
                                   WHERE c.deleted_at IS NULL AND op.definition_id = ?N_D
                                     AND CAST(op.value AS REAL) {op_sql} ?N_V"),
                            &mut 0),
                        &[integer(def_id), real(a)],
                    )
                }
                _ => Vec::new(),
            }
        }
    }

    fn eval_history_issue(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::HistoryIssue {
            issue_kind,
            issue_local_id,
            op,
            value,
        } = n
        else {
            return Vec::new();
        };
        if issue_kind != "cluster" && issue_kind != "known_issue" {
            return Vec::new();
        }
        if !value.is_finite() || *value < 0.0 {
            return Vec::new();
        }
        let (link_table, id_col) = if issue_kind == "cluster" {
            ("issue_cluster_conversations", "cluster_id")
        } else {
            ("known_issue_links", "known_issue_id")
        };
        let specific = issue_local_id.map(|id| id > 0).unwrap_or(false);
        let (issue_filter, mut p): (String, SqlParams) = if specific {
            (
                format!(" AND l.{id_col} = ?N_I"),
                vec![integer(issue_local_id.unwrap_or(0))],
            )
        } else {
            (String::new(), Vec::new())
        };
        p.push(real((*value).ceil().max(1.0)));
        let sql = renumber_placeholders(
            &format!(
                "SELECT c.customer_local_id AS cid FROM conversations c
                        JOIN {link_table} l ON l.conversation_id = c.id
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL{issue_filter}
                      GROUP BY c.customer_local_id HAVING COUNT(DISTINCT c.id) >= ?N_V"
            ),
            &mut 0,
        );
        let matching = self.ids(&sql, &p);
        if op == "gte" {
            return matching;
        }
        let set: HashSet<i64> = matching.into_iter().collect();
        self.all_customer_ids()
            .into_iter()
            .filter(|id| !set.contains(id))
            .collect()
    }

    fn eval_incident_exposure(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::IncidentExposure {
            incident_id,
            within_days,
        } = n
        else {
            return Vec::new();
        };
        let specific = incident_id.map(|id| id > 0).unwrap_or(false);
        let within = within_days.filter(|d| d.is_finite()).map(|d| d.max(0.0));
        let mut p: SqlParams = Vec::new();
        let incident_filter = if specific {
            p.push(integer(incident_id.unwrap_or(0)));
            " AND inc.id = ?N_I".to_string()
        } else {
            " AND inc.status <> 'resolved'".to_string()
        };
        let time_filter = if let Some(w) = within {
            p.push(text(&format!("-{} days", w as i64)));
            " AND julianday(COALESCE(c.remote_created_at, c.created_at, inc.created_at)) >= julianday('now', ?N_W)".to_string()
        } else {
            String::new()
        };
        let sql = renumber_placeholders(
            &format!("SELECT DISTINCT c.customer_local_id AS cid FROM conversations c
                        JOIN incident_conversations ic ON ic.conversation_id = c.id
                        JOIN incidents inc ON inc.id = ic.incident_id
                      WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL{incident_filter}{time_filter}"),
            &mut 0,
        );
        self.ids(&sql, &p)
    }

    fn eval_campaign_history(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CampaignHistory {
            relation,
            campaign_id,
        } = n
        else {
            return Vec::new();
        };
        let specific = campaign_id.map(|id| id > 0).unwrap_or(false);
        let (campaign_filter, p): (String, SqlParams) = if specific {
            (
                " AND r.campaign_id = ?N_C".to_string(),
                vec![integer(campaign_id.unwrap_or(0))],
            )
        } else {
            (String::new(), Vec::new())
        };
        match relation.as_str() {
            "received" => self.ids(
                &renumber_placeholders(
                    &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r
                                JOIN customers c ON c.id = r.customer_local_id
                              WHERE c.deleted_at IS NULL AND r.sent_at IS NOT NULL{campaign_filter}"),
                    &mut 0),
                &p,
            ),
            "replied" => self.ids(
                &renumber_placeholders(
                    &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r
                                JOIN customers c ON c.id = r.customer_local_id
                              WHERE c.deleted_at IS NULL AND r.replied_at IS NOT NULL{campaign_filter}"),
                    &mut 0),
                &p,
            ),
            "not_received" => {
                let received = self.ids(
                    &renumber_placeholders(
                        &format!("SELECT DISTINCT r.customer_local_id AS cid FROM outreach_recipients r WHERE 1=1{campaign_filter}"),
                        &mut 0),
                    &p,
                );
                let set: HashSet<i64> = received.into_iter().collect();
                self.all_customer_ids().into_iter().filter(|id| !set.contains(id)).collect()
            }
            _ => Vec::new(),
        }
    }

    fn eval_support_health(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::SupportHealth { metric, op, value } = n else {
            return Vec::new();
        };
        if !value.is_finite() {
            return Vec::new();
        }
        let cmp = if op == "gte" { ">=" } else { "<=" };
        match metric.as_str() {
            "avg_rating" => self.ids(
                &renumber_placeholders(
            &format!("SELECT c.id AS cid FROM customers c WHERE c.deleted_at IS NULL AND
                             (SELECT AVG(CASE ra.rating WHEN 'great' THEN 5.0 WHEN 'okay' THEN 3.0 WHEN 'not-good' THEN 1.0 ELSE NULL END)
                              FROM ratings ra WHERE ra.customer_local_id = c.id) {cmp} ?N_V"),
            &mut 0),

                &[real(*value)],
            ),
            "avg_effort_score" | "first_response_resolution_rate" | "high_friction_rate" => {
                let effort = metric == "avg_effort_score";
                let resolved = metric == "first_response_resolution_rate";
                let convs: Vec<(i64, String)> = self
                    .conn
                    .prepare("SELECT c.id, c.status FROM conversations c
                               WHERE c.deleted_at IS NULL AND c.customer_local_id IS NOT NULL")
                    .and_then(|mut s| {
                        let rows = s.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                        Ok(rows.filter_map(|x| x.ok()).collect::<Vec<_>>())
                    })
                    .unwrap_or_default();
                let mut acc: HashMap<i64, (f64, usize)> = HashMap::new();
                for (conv_id, status) in convs {
                    let threads = crate::quality::published_threads(self.conn, conv_id);
                    let Some(customer_id) = self
                        .conn
                        .query_row(
                            "SELECT customer_local_id FROM conversations WHERE id = ?1",
                            params![conv_id],
                            |r| r.get::<_, Option<i64>>(0),
                        )
                        .ok()
                        .flatten()
                    else { continue };
                    let out = crate::quality::derive_outcome(
                        &threads, &status, (conv_id, 0, None));
                    let sample: Option<f64> = if effort {
                        out.effort_score
                    } else if resolved {
                        Some(out.resolved_after_first.map(|v| v as f64).unwrap_or(0.0))
                    } else {
                        Some(if out.friction == "high" { 1.0 } else { 0.0 })
                    };
                    if let Some(v) = sample {
                        let e = acc.entry(customer_id).or_insert((0.0, 0));
                        e.0 += v;
                        e.1 += 1;
                    }
                }
                acc.into_iter()
                    .filter_map(|(cid, (sum, count))| {
                        if count == 0 { return None; }
                        let avg = sum / count as f64;
                        let pass = if op == "gte" { avg >= *value } else { avg <= *value };
                        pass.then_some(cid)
                    })
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn eval_custom_object_link(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomObjectLink { type_id } = n else {
            return Vec::new();
        };
        let specific = type_id.map(|id| id > 0).unwrap_or(false);
        let (join, filter, p): (String, String, SqlParams) = if specific {
            (
                "JOIN custom_object_types t ON t.id = o.type_id AND t.deleted_at IS NULL"
                    .to_string(),
                " AND t.id = ?N_T".to_string(),
                vec![integer(type_id.unwrap_or(0))],
            )
        } else {
            (String::new(), String::new(), Vec::new())
        };
        let sql = renumber_placeholders(
            &format!(
                "SELECT DISTINCT l.target_local_id AS cid FROM custom_object_links l
                        JOIN custom_objects o ON o.id = l.object_id AND o.deleted_at IS NULL
                        {join}
                      WHERE l.target_kind = 'customer'{filter}"
            ),
            &mut 0,
        );
        self.ids(&sql, &p)
    }

    fn eval_customer_event(&self, n: &SegmentNode) -> Vec<i64> {
        let SegmentNode::CustomerEvent {
            event_kind,
            within_days,
        } = n
        else {
            return Vec::new();
        };
        let within = within_days.filter(|d| d.is_finite()).map(|d| d.max(0.0));
        let mut p: SqlParams = vec![text(event_kind)];
        let time_filter = if let Some(w) = within {
            p.push(text(&format!("-{} days", w as i64)));
            " AND COALESCE(julianday(e.occurred_at), julianday(e.created_at)) >= julianday('now', ?N_W)".to_string()
        } else {
            String::new()
        };
        let sql = renumber_placeholders(
            &format!(
                "SELECT DISTINCT e.customer_local_id AS cid FROM customer_events e
                        JOIN customers c ON c.id = e.customer_local_id
                      WHERE c.deleted_at IS NULL AND e.event_kind = ?N_K{time_filter}"
            ),
            &mut 0,
        );
        self.ids(&sql, &p)
    }
}

fn renumber_placeholders(sql: &str, counter: &mut usize) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '?' && chars.peek() == Some(&'N') && chars.clone().nth(1) == Some('_') {
            chars.next();
            chars.next();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() {
                    chars.next();
                } else {
                    break;
                }
            }
            *counter += 1;
            out.push_str(&format!("?{}", counter));
        } else {
            out.push(ch);
        }
    }
    out
}

const KNOWN_CONDITION_KINDS: &[&str] = &[
    "customer_property",
    "contact",
    "ticket",
    "history",
    "history_tag",
    "organization_property",
    "history_issue",
    "incident_exposure",
    "campaign_history",
    "support_health",
    "custom_object_link",
    "customer_event",
];
const KNOWN_CONTACT_FIELDS: &[&str] = &[
    "name",
    "email",
    "email_domain",
    "organization",
    "job_title",
    "location",
    "background",
    "has_email",
    "has_phone",
    "has_multiple_emails",
];
const KNOWN_HISTORY_METRICS: &[&str] = &[
    "ticket_count",
    "open_count",
    "closed_count",
    "last_contact_within_days",
    "first_contact_before_days",
    "waited_over_hours_count",
];
const KNOWN_HEALTH_METRICS: &[&str] = &[
    "avg_rating",
    "avg_effort_score",
    "first_response_resolution_rate",
    "high_friction_rate",
];
const KNOWN_EVENT_KINDS: &[&str] = &[
    "signup",
    "support_conversation",
    "customer_message",
    "campaign",
    "campaign_reply",
    "rating",
    "incident_exposure",
    "custom_object_event",
];

pub fn validate_segment_tree(input: &Value) -> Result<SegmentDefinition, String> {
    let Some(obj) = input.as_object() else {
        return Err("The suggestion was not a JSON object.".to_string());
    };
    let combinator = if obj.get("combinator").and_then(|v| v.as_str()) == Some("any") {
        "any"
    } else {
        "all"
    };
    let (Some(conditions), Some(exclude)) = (
        obj.get("conditions").and_then(|v| v.as_array()),
        obj.get("exclude").and_then(|v| v.as_array()),
    ) else {
        return Err("The suggestion must contain conditions[] and exclude[] arrays.".to_string());
    };
    if conditions.len() > 20 || exclude.len() > 20 {
        return Err("Too many conditions in the suggestion.".to_string());
    }
    let mut nodes = 0usize;
    fn check_node(n: &Value, depth: usize, nodes: &mut usize) -> Result<(), String> {
        let Some(o) = n.as_object() else {
            return Err("A node was not an object.".to_string());
        };
        *nodes += 1;
        if *nodes > 60 || depth > 6 {
            return Err("The suggested tree is too large or too deep.".to_string());
        }
        let kind = o.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind == "group" {
            let Some(children) = o.get("children").and_then(|v| v.as_array()) else {
                return Err("A group node had no children.".to_string());
            };
            if children.is_empty() {
                return Err("A group node had no children.".to_string());
            }
            for child in children {
                check_node(child, depth + 1, nodes)?;
            }
            return Ok(());
        }
        if !KNOWN_CONDITION_KINDS.contains(&kind) {
            return Err(format!("Unknown condition kind '{kind}'."));
        }
        let c = o;
        match kind {
            "customer_property" => {
                let id = c.get("definitionId").and_then(|v| v.as_i64()).unwrap_or(0);
                if id <= 0 {
                    return Err(
                        "customer_property needs a positive integer definitionId.".to_string()
                    );
                }
                if c.get("op").and_then(|v| v.as_str()).is_none()
                    || c.get("value").map(|v| !v.is_string()).unwrap_or(false)
                {
                    return Err("customer_property needs op and value.".to_string());
                }
            }
            "contact" => {
                let field = c.get("field").and_then(|v| v.as_str()).unwrap_or("");
                if !KNOWN_CONTACT_FIELDS.contains(&field) {
                    return Err(format!("Unknown contact field '{field}'."));
                }
                if c.get("op").and_then(|v| v.as_str()).is_none() {
                    return Err("contact needs op.".to_string());
                }
            }
            "ticket" => {
                for (key, err) in [
                    ("tags", "ticket.tags must be an array."),
                    ("statuses", "ticket.statuses must be an array."),
                    (
                        "mailboxLocalIds",
                        "ticket.mailboxLocalIds must be an array.",
                    ),
                    (
                        "assigneeLocalIds",
                        "ticket.assigneeLocalIds must be an array.",
                    ),
                    ("customFields", "ticket.customFields must be an array."),
                ] {
                    if let Some(v) = c.get(key) {
                        if !v.is_array() {
                            return Err(err.to_string());
                        }
                    }
                }
                if let Some(ch) = c.get("channel") {
                    if !ch.is_string() {
                        return Err("ticket.channel must be a string.".to_string());
                    }
                }
                if let Some(aa) = c.get("aiAttribute").filter(|v| !v.is_null()) {
                    let Some(aa) = aa.as_object() else {
                        return Err("ticket.aiAttribute must be an object.".to_string());
                    };
                    let attr = aa.get("attribute").and_then(|v| v.as_str()).unwrap_or("");
                    if crate::catalog::AiAttributeKey::parse(attr).is_none() {
                        return Err(format!("Unknown AI attribute '{attr}'."));
                    }
                    if aa.get("value").and_then(|v| v.as_str()).is_none()
                        || aa.get("op").and_then(|v| v.as_str()).is_none()
                    {
                        return Err("ticket.aiAttribute needs op and value.".to_string());
                    }
                }
            }
            "history" => {
                let metric = c.get("metric").and_then(|v| v.as_str()).unwrap_or("");
                if !KNOWN_HISTORY_METRICS.contains(&metric) {
                    return Err(format!("Unknown history metric '{metric}'."));
                }
                if c.get("value").and_then(|v| v.as_f64()).is_none() {
                    return Err("history needs a numeric value.".to_string());
                }
            }
            "history_tag" => {
                let tag = c
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if tag.is_empty() {
                    return Err("history_tag needs a tag.".to_string());
                }
            }
            "organization_property" => {
                match c.get("field").and_then(|v| v.as_str()) {
                    None => {
                        let id = c.get("definitionId").and_then(|v| v.as_i64()).unwrap_or(0);
                        if id <= 0 {
                            return Err(
                                "organization_property needs field or definitionId.".to_string()
                            );
                        }
                    }
                    Some("name") | Some("domains") => {}
                    Some(f) => return Err(format!("Unknown organization field '{f}'.")),
                }
                if c.get("op").and_then(|v| v.as_str()).is_none() {
                    return Err("organization_property needs op.".to_string());
                }
            }
            "history_issue" => {
                let issue_kind = c.get("issueKind").and_then(|v| v.as_str()).unwrap_or("");
                if issue_kind != "cluster" && issue_kind != "known_issue" {
                    return Err(
                        "history_issue.issueKind must be cluster or known_issue.".to_string()
                    );
                }
                if c.get("value").and_then(|v| v.as_f64()).is_none() {
                    return Err("history_issue needs a numeric value.".to_string());
                }
            }
            "incident_exposure" => {}
            "campaign_history" => {
                let relation = c.get("relation").and_then(|v| v.as_str()).unwrap_or("");
                if !["received", "replied", "not_received"].contains(&relation) {
                    return Err(
                        "campaign_history.relation must be received, replied or not_received."
                            .to_string(),
                    );
                }
            }
            "support_health" => {
                let metric = c.get("metric").and_then(|v| v.as_str()).unwrap_or("");
                if !KNOWN_HEALTH_METRICS.contains(&metric) {
                    return Err(format!("Unknown support_health metric '{metric}'."));
                }
                if c.get("value").and_then(|v| v.as_f64()).is_none() {
                    return Err("support_health needs a numeric value.".to_string());
                }
            }
            "custom_object_link" => {}
            "customer_event" => {
                let kind = c.get("eventKind").and_then(|v| v.as_str()).unwrap_or("");
                if !KNOWN_EVENT_KINDS.contains(&kind) {
                    return Err(format!("Unknown customer event kind '{kind}'."));
                }
            }
            _ => return Err("Unreachable.".to_string()),
        }
        Ok(())
    }
    for n in conditions {
        check_node(n, 1, &mut nodes)?;
    }
    for n in exclude {
        check_node(n, 1, &mut nodes)?;
    }
    let merged = json!({
        "combinator": combinator,
        "conditions": conditions,
        "exclude": exclude,
    });
    parse_segment_tree(&merged)
}

pub fn suggest_catalog_context(conn: &Connection) -> String {
    let names = |sql: &str| -> Vec<String> {
        conn.prepare(sql)
            .and_then(|mut s| {
                let rows = s.query_map([], |r| r.get::<_, String>(0))?;
                Ok(rows.filter_map(|x| x.ok()).collect())
            })
            .unwrap_or_default()
    };
    let tags = names("SELECT name FROM tags WHERE deleted_at IS NULL ORDER BY name LIMIT 40");
    let mailboxes: Vec<String> = conn
        .prepare("SELECT id, name FROM mailboxes WHERE deleted_at IS NULL LIMIT 20")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "{}={}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let prop_defs: Vec<String> = conn
        .prepare("SELECT id, name, type FROM customer_property_definitions LIMIT 30")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "{}={} ({})",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?
                        .unwrap_or_else(|| "text".to_string())
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let org_prop_defs: Vec<String> = conn
        .prepare("SELECT id, name, type FROM organization_property_definitions LIMIT 30")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "{}={} ({})",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?
                        .unwrap_or_else(|| "text".to_string())
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let issues: Vec<String> = conn
        .prepare("SELECT id, title FROM known_issues LIMIT 20")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "known_issue {}={}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let incidents: Vec<String> = conn
        .prepare("SELECT id, code, title FROM incidents WHERE status <> 'resolved' LIMIT 20")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "incident {}={} {}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let campaigns: Vec<String> = conn
        .prepare("SELECT id, name FROM outreach_campaigns LIMIT 20")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "campaign {}={}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let object_types: Vec<String> = conn
        .prepare("SELECT id, name FROM custom_object_types WHERE deleted_at IS NULL LIMIT 20")
        .and_then(|mut s| {
            let rows = s.query_map([], |r| {
                Ok(format!(
                    "type {}={}",
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?
                ))
            })?;
            Ok(rows.filter_map(|x| x.ok()).collect())
        })
        .unwrap_or_default();
    let attributes: Vec<String> = crate::catalog::AiAttributeKey::ALL
        .iter()
        .map(|a| {
            let values = a.values();
            if values.is_empty() {
                format!("{} ({})", a.as_str(), a.value_type().as_str())
            } else {
                format!(
                    "{} ({}: {})",
                    a.as_str(),
                    a.value_type().as_str(),
                    values.join("|")
                )
            }
        })
        .collect();
    [
        format!(
            "tags: {}",
            if tags.is_empty() {
                "(none)".to_string()
            } else {
                tags.join(", ")
            }
        ),
        format!(
            "mailboxes (local ids): {}",
            if mailboxes.is_empty() {
                "(none)".to_string()
            } else {
                mailboxes.join(", ")
            }
        ),
        format!(
            "customer property definitions (id=name): {}",
            if prop_defs.is_empty() {
                "(none)".to_string()
            } else {
                prop_defs.join(", ")
            }
        ),
        format!(
            "organization property definitions (id=name): {}",
            if org_prop_defs.is_empty() {
                "(none)".to_string()
            } else {
                org_prop_defs.join(", ")
            }
        ),
        format!(
            "known issues: {}",
            if issues.is_empty() {
                "(none)".to_string()
            } else {
                issues.join(", ")
            }
        ),
        format!(
            "active incidents: {}",
            if incidents.is_empty() {
                "(none)".to_string()
            } else {
                incidents.join(", ")
            }
        ),
        format!(
            "campaigns: {}",
            if campaigns.is_empty() {
                "(none)".to_string()
            } else {
                campaigns.join(", ")
            }
        ),
        format!(
            "custom object types: {}",
            if object_types.is_empty() {
                "(none)".to_string()
            } else {
                object_types.join(", ")
            }
        ),
        format!("AI attributes (key: values): {}", attributes.join(", ")),
    ]
    .join("\n")
}

pub fn suggest_system_prompt() -> String {
    [
        "You translate a natural-language audience request into a structured segment definition (JSON).",
        "Output ONLY a JSON object: {\"combinator\":\"all\"|\"any\",\"conditions\":[...],\"exclude\":[...]}",
        "Available condition kinds and their fields:",
        "- customer_property: {kind, definitionId (from the catalog), op (equals|not_equals|contains|not_contains|starts_with|ends_with|is_empty|is_not_empty|gt|gte|lt|lte|between|before|after|is_any_of|is_none_of), value}",
        "- contact: {kind, field (name|email|email_domain|organization|job_title|location|background|has_email|has_phone|has_multiple_emails), op (equals|not_equals|contains|starts_with|ends_with|is_empty|is_not_empty), value}",
        "- ticket: {kind, tags:[], tagMode (any|all|none), statuses:[], mailboxLocalIds:[], assigneeLocalIds:[], createdWithinDays, modifiedWithinDays, channel, customFields:[{fieldLocalId, op, value}], aiAttribute:{attribute, op, value}}",
        "- history: {kind, metric (ticket_count|open_count|closed_count|last_contact_within_days|first_contact_before_days|waited_over_hours_count), op (gte|lte|eq), value}",
        "- history_tag: {kind, tag, withinDays}",
        "- organization_property: {kind, field (name|domains) OR definitionId, op, value}",
        "- history_issue: {kind, issueKind (cluster|known_issue), issueLocalId (omit for any), op (gte|eq), value}",
        "- incident_exposure: {kind, incidentId (omit for any active), withinDays}",
        "- campaign_history: {kind, relation (received|replied|not_received), campaignId (omit for any)}",
        "- support_health: {kind, metric (avg_rating|avg_effort_score|first_response_resolution_rate|high_friction_rate), op (gte|lte), value}",
        "- custom_object_link: {kind, typeId (omit for any)}",
        "- customer_event: {kind, eventKind (signup|support_conversation|customer_message|campaign|campaign_reply|rating|incident_exposure|custom_object_event), withinDays}",
        "Rules: use ONLY ids/names from the catalog below; when unsure of an id, prefer tag/contact/text conditions instead of guessing ids; put exclusions in exclude[]; never invent fields.",
    ]
    .join("\n")
}

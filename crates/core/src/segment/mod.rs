use std::collections::{HashMap, HashSet};

use crate::search::escape_like;
use rusqlite::{Connection, params};
use serde_json::{Value, json};

mod eval;

pub(crate) const MAX_TREE_DEPTH: u32 = 10;
pub(crate) const MAX_NODE_COUNT: u32 = 200;

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

#[cfg(test)]
mod batch30_tests {
    use super::*;
    use rusqlite::Connection;

    fn db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        conn
    }

    fn seed_customer_with_email(conn: &Connection, remote: i64, email: &str) -> i64 {
        conn.execute(
            "INSERT INTO customers (remote_id, first_name) VALUES (?1, 'C')",
            [remote],
        )
        .expect("customer");
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO customer_emails (customer_id, value) VALUES (?1, ?2)",
            rusqlite::params![id, email],
        )
        .expect("email");
        id
    }

    fn contact_ids(conn: &Connection, field: &str, op: &str, value: &str) -> Vec<i64> {
        let engine = SegmentEngine::new(conn);
        let mut notes = Vec::new();
        engine.evaluate_nodes(
            &[SegmentNode::Contact {
                field: field.to_string(),
                op: op.to_string(),
                value: Some(value.to_string()),
            }],
            true,
            &mut notes,
            0,
        )
    }

    #[test]
    fn email_domain_filter_honors_the_operator_and_anchors_the_tail() {
        let conn = db();
        let exact = seed_customer_with_email(&conn, 1, "user@example.com");
        let tld = seed_customer_with_email(&conn, 2, "admin@example.com.au");
        let sub = seed_customer_with_email(&conn, 3, "boss@mail.example.com");
        let other = seed_customer_with_email(&conn, 4, "plain@other.org");
        let no_email = {
            conn.execute(
                "INSERT INTO customers (remote_id, first_name) VALUES (5, 'C')",
                [],
            )
            .expect("customer");
            conn.last_insert_rowid()
        };

        let sorted = |mut v: Vec<i64>| {
            v.sort_unstable();
            v
        };

        assert_eq!(
            sorted(contact_ids(&conn, "email_domain", "equals", "example.com")),
            vec![exact],
            "exact domain match: user@example.com only"
        );
        assert!(!contact_ids(&conn, "email_domain", "equals", "example.com").contains(&tld));

        assert_eq!(
            sorted(contact_ids(
                &conn,
                "email_domain",
                "ends_with",
                "example.com"
            )),
            sorted(vec![exact, sub]),
        );

        assert_eq!(
            sorted(contact_ids(
                &conn,
                "email_domain",
                "contains",
                "example.com"
            )),
            sorted(vec![exact, sub, tld]),
        );

        assert_eq!(
            sorted(contact_ids(&conn, "email_domain", "starts_with", "ex")),
            sorted(vec![exact, tld]),
        );

        assert_eq!(
            sorted(contact_ids(
                &conn,
                "email_domain",
                "not_equals",
                "example.com"
            )),
            sorted(vec![tld, sub, other, no_email]),
            "customers without the exact domain match, including the one with no email"
        );

        assert_eq!(
            contact_ids(&conn, "email_domain", "is_empty", ""),
            vec![no_email]
        );
        assert_eq!(
            sorted(contact_ids(&conn, "email_domain", "is_not_empty", "")),
            sorted(vec![exact, tld, sub, other]),
        );
    }
}

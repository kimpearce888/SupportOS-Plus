use std::sync::atomic::{AtomicU64, Ordering};

use leptos::*;
use serde_json::{json, Value};

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

pub fn next_uid() -> u64 {
    NEXT_UID.fetch_add(1, Ordering::Relaxed)
}

fn uid_of(n: &Value) -> u64 {
    n.get("_uid").and_then(|v| v.as_u64()).unwrap_or(0)
}

pub fn inject_uid(n: &Value) -> Value {
    let mut out = n.clone();
    if let Some(obj) = out.as_object_mut() {
        if !obj.contains_key("_uid") {
            obj.insert("_uid".into(), json!(next_uid()));
        }
    }
    if out.get("kind").and_then(|k| k.as_str()) == Some("group") {
        if let Some(children) = out.get_mut("children").and_then(|c| c.as_array_mut()) {
            for child in children.iter_mut() {
                *child = inject_uid(child);
            }
        }
    }
    out
}

pub fn strip_uid(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                if k == "_uid" {
                    continue;
                }
                out.insert(k.clone(), strip_uid(val));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(strip_uid).collect()),
        other => other.clone(),
    }
}

#[must_use]
pub fn op_label(op: &str) -> String {
    match op {
        "equals" => "=".to_string(),
        "not_equals" => "\u{2260}".to_string(),
        "contains" => "contains".to_string(),
        "not_contains" => "does not contain".to_string(),
        "starts_with" => "starts with".to_string(),
        "ends_with" => "ends with".to_string(),
        "is_empty" => "is empty".to_string(),
        "is_not_empty" => "is set".to_string(),
        "gt" => ">".to_string(),
        "gte" => "\u{2265}".to_string(),
        "lt" => "<".to_string(),
        "lte" => "\u{2264}".to_string(),
        "between" => "between".to_string(),
        "before" => "before".to_string(),
        "after" => "after".to_string(),
        "is_any_of" => "is any of".to_string(),
        "is_none_of" => "is none of".to_string(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn contact_field_label(f: &str) -> String {
    match f {
        "name" => "Name".to_string(),
        "email" => "Email".to_string(),
        "email_domain" => "Email domain".to_string(),
        "organization" => "Organization".to_string(),
        "job_title" => "Job title".to_string(),
        "location" => "Location".to_string(),
        "background" => "Background / notes".to_string(),
        "has_email" => "Has an email".to_string(),
        "has_phone" => "Has a phone".to_string(),
        "has_multiple_emails" => "Has multiple emails".to_string(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn health_metric_label(m: &str) -> String {
    match m {
        "avg_rating" => "average rating (1-5)".to_string(),
        "avg_effort_score" => "average effort score (0-10)".to_string(),
        "first_response_resolution_rate" => "first-response resolution rate (0-1)".to_string(),
        "high_friction_rate" => "high-friction rate (0-1)".to_string(),
        other => other.to_string(),
    }
}

pub const KIND_LABELS: [(&str, &str); 12] = [
    ("customer_property", "Customer property"),
    ("contact", "Contact field"),
    ("ticket", "Ticket condition"),
    ("history", "Support history"),
    ("history_tag", "Ever tagged"),
    ("organization_property", "Organization data / property"),
    ("history_issue", "Previous issues"),
    ("incident_exposure", "Incident exposure"),
    ("campaign_history", "Campaign history"),
    ("support_health", "Support health"),
    ("custom_object_link", "Custom object link"),
    ("customer_event", "Customer timeline event"),
];

fn s(n: &Value, key: &str) -> String {
    n.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn os(n: &Value, key: &str) -> Option<String> {
    n.get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|v| !v.is_empty())
}

fn oi(n: &Value, key: &str) -> Option<i64> {
    n.get(key).and_then(|v| v.as_i64())
}

fn of(n: &Value, key: &str) -> Option<f64> {
    n.get(key).and_then(|v| v.as_f64())
}

fn sarr(n: &Value, key: &str) -> Vec<String> {
    n.get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn i64arr(n: &Value, key: &str) -> Vec<i64> {
    n.get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default()
}

#[must_use]
pub fn num_str(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

#[must_use]
pub fn default_node(kind: &str, meta: Option<&Value>) -> Value {
    let mut node = match kind {
        "customer_property" => {
            let def = meta
                .and_then(|m| m.get("property_definitions"))
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or_default();
            json!({
                "kind": "customer_property",
                "definitionId": def.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                "name": def.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "type": def.get("type").and_then(|v| v.as_str()).unwrap_or("text"),
                "op": "equals",
                "value": "",
            })
        }
        "contact" => json!({"kind": "contact", "field": "email", "op": "contains", "value": ""}),
        "ticket" => json!({"kind": "ticket", "tags": [], "tagMode": "any"}),
        "history" => json!({"kind": "history", "metric": "ticket_count", "op": "gte", "value": 1}),
        "organization_property" => {
            json!({"kind": "organization_property", "field": "name", "op": "contains", "value": ""})
        }
        "history_issue" => json!({
            "kind": "history_issue", "issueKind": "known_issue",
            "issueLocalId": null, "op": "gte", "value": 1
        }),
        "incident_exposure" => {
            json!({"kind": "incident_exposure", "incidentId": null, "withinDays": null})
        }
        "campaign_history" => {
            json!({"kind": "campaign_history", "relation": "received", "campaignId": null})
        }
        "support_health" => {
            json!({"kind": "support_health", "metric": "avg_rating", "op": "gte", "value": 4})
        }
        "custom_object_link" => json!({"kind": "custom_object_link", "typeId": null}),
        "customer_event" => {
            json!({"kind": "customer_event", "eventKind": "campaign_reply", "withinDays": null})
        }
        _ => json!({"kind": "history_tag", "tag": "", "withinDays": null}),
    };
    if let Some(obj) = node.as_object_mut() {
        obj.insert("_uid".into(), json!(next_uid()));
    }
    node
}

#[must_use]
pub fn describe_condition(n: &Value) -> String {
    let kind = n.get("kind").and_then(|k| k.as_str()).unwrap_or("");
    if kind == "group" {
        let sep = if n.get("combinator").and_then(|c| c.as_str()) == Some("any") {
            " OR "
        } else {
            " AND "
        };
        let children = n
            .get("children")
            .and_then(|c| c.as_array())
            .map(|a| {
                a.iter()
                    .map(describe_condition)
                    .collect::<Vec<_>>()
                    .join(sep)
            })
            .unwrap_or_default();
        return format!("({children})");
    }
    let op = s(n, "op");
    match kind {
        "customer_property" => {
            let val = if op == "is_any_of" || op == "is_none_of" {
                sarr(n, "values").join("/")
            } else {
                os(n, "value").unwrap_or_default()
            };
            let between = if op == "between" {
                format!(" and {}", os(n, "value2").unwrap_or_default())
            } else {
                String::new()
            };
            format!("{} {} {}{}", s(n, "name"), op_label(&op), val, between)
        }
        "contact" => {
            let label = contact_field_label(&s(n, "field"));
            format!(
                "{label} {} {}",
                op_label(&op),
                os(n, "value").unwrap_or_default()
            )
            .trim()
            .to_string()
        }
        "ticket" => {
            let mut bits: Vec<String> = Vec::new();
            let tags = sarr(n, "tags");
            if !tags.is_empty() {
                let mode = match s(n, "tagMode").as_str() {
                    "all" => "ALL",
                    "none" => "NONE",
                    _ => "ANY",
                };
                bits.push(format!("ticket has {mode} of: {}", tags.join(", ")));
            }
            let statuses = sarr(n, "statuses");
            if !statuses.is_empty() {
                bits.push(format!("status {}", statuses.join("/")));
            }
            if !i64arr(n, "mailboxLocalIds").is_empty() {
                bits.push("inbox filtered".to_string());
            }
            if !i64arr(n, "assigneeLocalIds").is_empty() {
                bits.push("assignee filtered".to_string());
            }
            let channel = os(n, "channel").unwrap_or_default();
            if !channel.is_empty() {
                bits.push(format!("channel {channel}"));
            }
            let cfs = n
                .get("customFields")
                .and_then(|c| c.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if cfs > 0 {
                bits.push(format!("{cfs} custom field filter(s)"));
            }
            if let Some(d) = of(n, "createdWithinDays") {
                bits.push(format!("created \u{2264} {}d", num_str(d)));
            }
            if let Some(d) = of(n, "modifiedWithinDays") {
                bits.push(format!("modified \u{2264} {}d", num_str(d)));
            }
            if bits.is_empty() {
                "ticket condition".to_string()
            } else {
                bits.join(" + ")
            }
        }
        "history" => {
            let metric_s = s(n, "metric");
            let metric = match metric_s.as_str() {
                "ticket_count" => "total tickets",
                "open_count" => "open tickets",
                "closed_count" => "closed tickets",
                "last_contact_within_days" => "last contact \u{2264} (days)",
                "first_contact_before_days" => "first contact \u{2265} (days)",
                "waited_over_hours_count" => "waited over (hours)",
                other => other,
            };
            let sym = match op.as_str() {
                "gte" => "\u{2265}",
                "lte" => "\u{2264}",
                _ => "=",
            };
            format!("{metric} {sym} {}", num_str(of(n, "value").unwrap_or(0.0)))
        }
        "history_tag" => {
            let within = of(n, "withinDays")
                .map(|d| format!(" within {} days", num_str(d)))
                .unwrap_or_default();
            format!("ever had a ticket tagged \"{}\"{}", s(n, "tag"), within)
        }
        "organization_property" => {
            let target = match os(n, "field").as_deref() {
                Some("name") => "name".to_string(),
                Some("domains") => "domains".to_string(),
                _ => os(n, "name").unwrap_or_else(|| {
                    format!(
                        "property #{}",
                        oi(n, "definitionId")
                            .map(|i| i.to_string())
                            .unwrap_or_else(|| "?".to_string())
                    )
                }),
            };
            format!(
                "organization {target} {} {}",
                op_label(&op),
                os(n, "value").unwrap_or_default()
            )
            .trim()
            .to_string()
        }
        "history_issue" => {
            let subject = match oi(n, "issueLocalId") {
                Some(id) => format!("issue #{id}"),
                None => {
                    if s(n, "issueKind") == "cluster" {
                        "any cluster".to_string()
                    } else {
                        "any known issue".to_string()
                    }
                }
            };
            let count = if op == "gte" {
                format!("\u{2265} {}", num_str(of(n, "value").unwrap_or(0.0)))
            } else {
                "= none".to_string()
            };
            format!("{subject} linked conversations {count}")
        }
        "incident_exposure" => {
            let subject = match oi(n, "incidentId") {
                Some(id) => format!("incident #{id}"),
                None => "an active incident".to_string(),
            };
            let within = of(n, "withinDays")
                .map(|d| format!(" within {}d", num_str(d)))
                .unwrap_or_default();
            format!("exposed to {subject}{within}")
        }
        "campaign_history" => {
            let relation = if s(n, "relation") == "not_received" {
                "never received".to_string()
            } else {
                s(n, "relation")
            };
            let campaign = oi(n, "campaignId")
                .map(|id| format!(" (#{id})"))
                .unwrap_or_default();
            format!("{relation} a campaign{campaign}")
        }
        "support_health" => {
            let label = health_metric_label(&s(n, "metric"));
            let sym = if op == "gte" { "\u{2265}" } else { "\u{2264}" };
            format!(
                "support health: {label} {sym} {}",
                num_str(of(n, "value").unwrap_or(0.0))
            )
        }
        "custom_object_link" => {
            let ty = oi(n, "typeId")
                .map(|id| format!(" of type #{id}"))
                .unwrap_or_default();
            format!("linked to a custom object{ty}")
        }
        "customer_event" => {
            let kind_txt = s(n, "eventKind").replace('_', " ");
            let within = of(n, "withinDays")
                .map(|d| format!(" within {}d", num_str(d)))
                .unwrap_or_default();
            format!("timeline includes \"{kind_txt}\"{within}")
        }
        _ => "condition".to_string(),
    }
}

#[must_use]
pub fn toggle_value(values: &[String], v: &str) -> Vec<String> {
    if values.iter().any(|x| x == v) {
        values.iter().filter(|x| x.as_str() != v).cloned().collect()
    } else {
        let mut out = values.to_vec();
        out.push(v.to_string());
        out
    }
}

#[must_use]
pub fn add_tag(tags: &[String], raw: &str) -> Option<Vec<String>> {
    let v = raw.trim().to_lowercase();
    if v.is_empty() || tags.contains(&v) {
        None
    } else {
        let mut out = tags.to_vec();
        out.push(v);
        Some(out)
    }
}

fn parse_opt_f64(txt: &str) -> Option<f64> {
    let t = txt.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

fn parse_req_f64(txt: &str) -> f64 {
    parse_opt_f64(txt).unwrap_or(0.0)
}

fn parse_opt_i64(txt: &str) -> Option<i64> {
    let t = txt.trim();
    if t.is_empty() {
        None
    } else {
        t.parse::<i64>().ok()
    }
}

fn meta_arr(m: &Option<Value>, key: &str) -> Vec<Value> {
    m.as_ref()
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

fn meta_str_list(m: &Option<Value>, key: &str) -> Vec<String> {
    m.as_ref()
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn ops_for_type(m: &Option<Value>, ty: &str) -> Vec<String> {
    let from_meta = m
        .as_ref()
        .and_then(|v| v.get("operators_by_type"))
        .and_then(|v| v.get(ty))
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if from_meta.is_empty() {
        vec![
            "equals".to_string(),
            "not_equals".to_string(),
            "contains".to_string(),
        ]
    } else {
        from_meta
    }
}

type NodeList = RwSignal<Vec<Value>>;

fn node_by(list: NodeList, uid: u64) -> Value {
    list.with(|l| l.iter().find(|n| uid_of(n) == uid).cloned())
        .unwrap_or(Value::Null)
}

fn patch_uid(list: NodeList, uid: u64, f: impl FnOnce(&mut Value)) {
    list.update(|l| {
        if let Some(n) = l.iter_mut().find(|n| uid_of(n) == uid) {
            f(n);
        }
    });
}

fn remove_uid(list: NodeList, uid: u64) {
    list.update(|l| l.retain(|n| uid_of(n) != uid));
}

fn set_str(list: NodeList, uid: u64, key: &str, val: &str) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_opt(list: NodeList, uid: u64, key: &str, val: Option<String>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_opt_f64(list: NodeList, uid: u64, key: &str, val: Option<f64>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_opt_i64(list: NodeList, uid: u64, key: &str, val: Option<i64>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_req_f64(list: NodeList, uid: u64, key: &str, val: f64) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_str_arr(list: NodeList, uid: u64, key: &str, val: &[String]) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val));
        }
    });
}

fn set_opt_i64_arr(list: NodeList, uid: u64, key: &str, val: Option<i64>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val.map(|v| vec![v]).unwrap_or_default()));
        }
    });
}

fn set_str_arr_first(list: NodeList, uid: u64, key: &str, val: Option<&str>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            o.insert(key.into(), json!(val.map(|v| vec![v]).unwrap_or_default()));
        }
    });
}

fn set_fields(list: NodeList, uid: u64, kvs: Vec<(&str, Value)>) {
    patch_uid(list, uid, |n| {
        if let Some(o) = n.as_object_mut() {
            for (k, v) in kvs {
                o.insert(k.into(), v);
            }
        }
    });
}

fn with_custom_fields(list: NodeList, uid: u64, f: impl FnOnce(&mut Vec<Value>)) {
    patch_uid(list, uid, |n| {
        let mut cfs = n
            .get("customFields")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        f(&mut cfs);
        if let Some(o) = n.as_object_mut() {
            o.insert("customFields".into(), Value::Array(cfs));
        }
    });
}

fn cur_s(list: NodeList, uid: u64, key: &str) -> String {
    s(&node_by(list, uid), key)
}
fn cur_os(list: NodeList, uid: u64, key: &str) -> Option<String> {
    os(&node_by(list, uid), key)
}
fn cur_oi(list: NodeList, uid: u64, key: &str) -> Option<i64> {
    oi(&node_by(list, uid), key)
}
fn cur_of(list: NodeList, uid: u64, key: &str) -> Option<f64> {
    of(&node_by(list, uid), key)
}
fn cur_sarr(list: NodeList, uid: u64, key: &str) -> Vec<String> {
    sarr(&node_by(list, uid), key)
}

#[component]
fn FieldRow(label: &'static str, children: Children) -> impl IntoView {
    view! {
        <label class="spp-cond-editor__field">
            <span>{label}</span>
            {children()}
        </label>
    }
}

#[component]
pub fn ConditionEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    let kind_now = move || cur_s(list, uid, "kind");
    view! {
        <div class="spp-cond-editor">
            <div class="spp-cond-editor__head">
                <select
                    class="spp-input spp-cond-editor__kind"
                    value=kind_now
                    on:change=move |ev| {
                        let next = event_target_value(&ev);
                        let m = meta.get();
                        let node = default_node(&next, m.as_ref());
                        patch_uid(list, uid, |n| *n = node);
                    }
                >
                    {move || {
                        KIND_LABELS
                            .iter()
                            .map(|(k, label)| {
                                view! { <option value=*k>{*label}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
                <span class="spp-muted spp-text-xs spp-grow">
                    {move || {
                        if kind_now() == "ticket" {
                            "All filters below apply to the SAME conversation (tag ALL/ANY/NONE is evaluated per ticket)"
                        } else {
                            ""
                        }
                    }}
                </span>
                <button
                    class="spp-button spp-button--ghost spp-button--small"
                    on:click=move |_| remove_uid(list, uid)
                    aria-label="Remove condition"
                >
                    "Remove"
                </button>
            </div>
            {move || {
                let node = node_by(list, uid);
                let kind = s(&node, "kind");
                match kind.as_str() {
                    "customer_property" => view! {
                        <PropertyEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "contact" => view! {
                        <ContactEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "ticket" => view! {
                        <TicketEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "history" => view! { <HistoryEditor list=list uid=uid /> }.into_view(),
                    "history_tag" => view! { <HistoryTagEditor list=list uid=uid meta=meta /> }.into_view(),
                    "organization_property" => view! {
                        <OrganizationPropertyEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "history_issue" => view! {
                        <HistoryIssueEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "incident_exposure" => view! {
                        <IncidentExposureEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "campaign_history" => view! {
                        <CampaignHistoryEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "support_health" => view! { <SupportHealthEditor list=list uid=uid /> }
                        .into_view(),
                    "custom_object_link" => view! {
                        <CustomObjectLinkEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    "customer_event" => view! {
                        <CustomerEventEditor list=list uid=uid meta=meta />
                    }
                    .into_view(),
                    _ => ().into_view(),
                }
            }}
        </div>
    }
}

#[component]
fn PropertyEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    let values_input = create_rw_signal(String::new());
    let op_now = move || cur_s(list, uid, "op");
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Property">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_oi(list, uid, "definitionId").map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| {
                        let raw = event_target_value(&ev);
                        let m = meta.get();
                        let defs = meta_arr(&m, "property_definitions");
                        let next = defs.iter().find(|d| {
                            d.get("id").and_then(|v| v.as_i64()).map(|i| i.to_string()) == Some(raw.clone())
                        });
                        if let Some(next) = next {
                            let ty = next.get("type").and_then(|v| v.as_str()).unwrap_or("text");
                            let first_op = ops_for_type(&m, ty).first().cloned().unwrap_or_else(|| "equals".to_string());
                            set_fields(list, uid, vec![
                                ("definitionId", json!(next.get("id").and_then(|v| v.as_i64()).unwrap_or(0))),
                                ("name", json!(next.get("name").and_then(|v| v.as_str()).unwrap_or(""))),
                                ("type", json!(ty)),
                                ("op", json!(first_op)),
                                ("value", json!("")),
                                ("values", json!([])),
                            ]);
                        }
                    }
                >
                    {move || {
                        let m = meta.get();
                        let defs = meta_arr(&m, "property_definitions");
                        if defs.is_empty() {
                            vec![view! { <option value="0">"(no properties synced)"</option> }]
                        } else {
                            defs.iter()
                                .map(|d| {
                                    let id = d.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let ty = d.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                    let populated = d.get("populated").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let label = if populated == 0 {
                                        format!("{name} \u{b7} {ty} (no values locally)")
                                    } else {
                                        format!("{name} \u{b7} {ty}")
                                    };
                                    view! { <option value=id.to_string()>{label}</option> }
                                })
                                .collect::<Vec<_>>()
                        }
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Operator">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=op_now
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    {move || {
                        let m = meta.get();
                        let node = node_by(list, uid);
                        let ty = find_def_type(&m, &node).unwrap_or_else(|| s(&node, "type"));
                        ops_for_type(&m, &ty)
                            .iter()
                            .map(|op| view! { <option value=op.clone()>{op_label(op)}</option> })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            {move || {
                let op = op_now();
                if op == "is_empty" || op == "is_not_empty" || op == "is_any_of" || op == "is_none_of" {
                    Vec::<View>::new().into_view()
                } else {
                    let m = meta.get();
                    let node = node_by(list, uid);
                    let ty = find_def_type(&m, &node).unwrap_or_else(|| s(&node, "type"));
                    let placeholder = match ty.as_str() {
                        "number" => "number",
                        "date" => "YYYY-MM-DD",
                        _ => "value",
                    };
                    view! {
                        <FieldRow label="Value">
                            <input
                                class="spp-input spp-cond-editor__control"
                                value=move || cur_os(list, uid, "value").unwrap_or_default()
                                placeholder=placeholder
                                on:change=move |ev| set_opt(list, uid, "value", non_empty(event_target_value(&ev)))
                            />
                        </FieldRow>
                    }
                    .into_view()
                }
            }}
            {move || {
                if op_now() == "between" {
                    view! {
                        <FieldRow label="And">
                            <input
                                class="spp-input spp-cond-editor__control"
                                value=move || cur_os(list, uid, "value2").unwrap_or_default()
                                placeholder="number or YYYY-MM-DD"
                                on:change=move |ev| set_opt(list, uid, "value2", non_empty(event_target_value(&ev)))
                            />
                        </FieldRow>
                    }
                    .into_view()
                } else {
                    ().into_view()
                }
            }}
            {move || {
                let op = op_now();
                if op == "is_any_of" || op == "is_none_of" {
                    let list2 = list;
                    view! {
                        <FieldRow label="Values">
                            <div class="spp-flex spp-flex--wrap spp-gap-4">
                                {move || {
                                    let m = meta.get();
                                    let node = node_by(list2, uid);
                                    let def_id = oi(&node, "definitionId");
                                    let observed = m
                                        .as_ref()
                                        .and_then(|v| v.get("property_definitions"))
                                        .and_then(|v| v.as_array())
                                        .and_then(|a| {
                                            a.iter().find(|d| d.get("id").and_then(|x| x.as_i64()) == def_id)
                                        })
                                        .and_then(|d| d.get("observed_values"))
                                        .and_then(|v| v.as_array())
                                        .map(|a| {
                                            a.iter()
                                                .filter_map(|x| x.as_str().map(str::to_string))
                                                .collect::<Vec<_>>()
                                        })
                                        .unwrap_or_default();
                                    observed
                                        .into_iter()
                                        .map(|v| {
                                            let list3 = list2;
                                            let v_class = v.clone();
                                            let v_click = v.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    class="spp-chip"
                                                    class:is-on=move || cur_sarr(list3, uid, "values").contains(&v_class)
                                                    on:click=move |_| {
                                                        let next = toggle_value(&cur_sarr(list3, uid, "values"), &v_click);
                                                        set_str_arr(list3, uid, "values", &next);
                                                    }
                                                >
                                                    {v}
                                                </button>
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                }}
                                <input
                                    class="spp-input spp-cond-editor__add"
                                    placeholder="add + Enter"
                                    value=move || values_input.get()
                                    on:input=move |ev| values_input.set(event_target_value(&ev))
                                    on:keydown=move |ev| {
                                        if ev.key().as_str() == "Enter" {
                                            let v = values_input.get_untracked();
                                            let next = toggle_value(&cur_sarr(list, uid, "values"), v.trim());
                                            set_str_arr(list, uid, "values", &next);
                                            values_input.set(String::new());
                                        }
                                    }
                                />
                            </div>
                        </FieldRow>
                    }
                    .into_view()
                } else {
                    ().into_view()
                }
            }}
        </div>
    }
}

fn find_def_type(m: &Option<Value>, node: &Value) -> Option<String> {
    let def_id = oi(node, "definitionId");
    m.as_ref()
        .and_then(|v| v.get("property_definitions"))
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .find(|d| d.get("id").and_then(|x| x.as_i64()) == def_id)
        })
        .and_then(|d| d.get("type"))
        .and_then(|t| t.as_str())
        .map(str::to_string)
}

fn non_empty(v: String) -> Option<String> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[component]
fn ContactEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Field">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "field")
                    on:change=move |ev| set_str(list, uid, "field", &event_target_value(&ev))
                >
                    {move || {
                        let m = meta.get();
                        let fields = meta_str_list(&m, "contact_fields");
                        let fields = if fields.is_empty() {
                            vec!["email".to_string()]
                        } else {
                            fields
                        };
                        fields
                            .iter()
                            .map(|f| view! { <option value=f.clone()>{contact_field_label(f)}</option> })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Operator">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "op")
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    {CONTACT_OPS
                        .iter()
                        .map(|op| view! { <option value=*op>{op_label(op)}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </FieldRow>
            {move || {
                let op = cur_s(list, uid, "op");
                if op == "is_empty" || op == "is_not_empty" {
                    ().into_view()
                } else {
                    view! {
                        <FieldRow label="Value">
                            <input
                                class="spp-input spp-cond-editor__control"
                                value=move || cur_os(list, uid, "value").unwrap_or_default()
                                placeholder=move || {
                                    if cur_s(list, uid, "field") == "email_domain" { "company.com".to_string() } else { "value".to_string() }
                                }
                                on:change=move |ev| set_opt(list, uid, "value", non_empty(event_target_value(&ev)))
                            />
                        </FieldRow>
                    }
                    .into_view()
                }
            }}
        </div>
    }
}

const CONTACT_OPS: [&str; 7] = [
    "equals",
    "not_equals",
    "contains",
    "starts_with",
    "ends_with",
    "is_empty",
    "is_not_empty",
];

#[component]
fn TicketEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    let tag_input = create_rw_signal(String::new());
    let datalist_id = format!("outreach-tag-list-{uid}");
    let datalist_id_input = datalist_id.clone();
    let datalist_id_list = datalist_id.clone();
    view! {
        <div>
            <div class="spp-flex spp-flex--wrap">
                <FieldRow label="Tag mode">
                    <select
                        class="spp-input spp-cond-editor__control"
                        value=move || cur_s(list, uid, "tagMode")
                        on:change=move |ev| set_str(list, uid, "tagMode", &event_target_value(&ev))
                    >
                        <option value="any">"has ANY of"</option>
                        <option value="all">"has ALL of"</option>
                        <option value="none">"has NONE of"</option>
                    </select>
                </FieldRow>
                <FieldRow label="Tags">
                    <div class="spp-flex spp-flex--wrap spp-gap-4">
                        {move || {
                            cur_sarr(list, uid, "tags")
                                .into_iter()
                                .map(|t| {
                                    let t2 = t.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class="spp-chip spp-chip--tag"
                                            title="Click to remove"
                                            on:click=move |_| {
                                                let next: Vec<String> = cur_sarr(list, uid, "tags")
                                                    .into_iter()
                                                    .filter(|x| *x != t2)
                                                    .collect();
                                                set_str_arr(list, uid, "tags", &next);
                                            }
                                        >
                                            {t.clone()}" \u{d7}"
                                        </button>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                        <input
                            class="spp-input spp-cond-editor__add"
                            placeholder="add tag + Enter"
                            attr:list=datalist_id_input.clone()
                            value=move || tag_input.get()
                            on:input=move |ev| tag_input.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key().as_str() == "Enter" {
                                    if let Some(next) = add_tag(&cur_sarr(list, uid, "tags"), &tag_input.get_untracked()) {
                                        set_str_arr(list, uid, "tags", &next);
                                    }
                                    tag_input.set(String::new());
                                }
                            }
                        />
                        <datalist id=datalist_id_list.clone()>
                            {move || {
                                let m = meta.get();
                                meta_str_list(&m, "tags")
                                    .into_iter()
                                    .map(|t| view! { <option value=t.clone()>{t}</option> })
                                    .collect::<Vec<_>>()
                            }}
                        </datalist>
                    </div>
                </FieldRow>
                <FieldRow label="Status">
                    <select
                        class="spp-input spp-cond-editor__control"
                        value=move || cur_sarr(list, uid, "statuses").first().cloned().unwrap_or_default()
                        on:change=move |ev| set_str_arr_first(list, uid, "statuses", non_empty(event_target_value(&ev)).as_deref())
                    >
                        <option value="">"any status"</option>
                        {move || {
                            let m = meta.get();
                            let statuses = meta_str_list(&m, "ticket_statuses");
                            let statuses = if statuses.is_empty() {
                                vec!["active".to_string(), "pending".to_string(), "closed".to_string()]
                            } else {
                                statuses
                            };
                            statuses
                                .into_iter()
                                .map(|st| view! { <option value=st.clone()>{st}</option> })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </FieldRow>
                <FieldRow label="Inbox">
                    <select
                        class="spp-input spp-cond-editor__control"
                        value=move || cur_i64_first(list, uid, "mailboxLocalIds")
                        on:change=move |ev| set_opt_i64_arr(list, uid, "mailboxLocalIds", parse_opt_i64(&event_target_value(&ev)))
                    >
                        <option value="">"any inbox"</option>
                        {move || {
                            let m = meta.get();
                            meta_arr(&m, "mailboxes")
                                .into_iter()
                                .map(|mb| {
                                    let id = mb.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = mb.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    view! { <option value=id.to_string()>{name}</option> }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </FieldRow>
            </div>
            <div class="spp-flex spp-flex--wrap spp-mt-8">
                <FieldRow label="Assignee">
                    <select
                        class="spp-input spp-cond-editor__control"
                        value=move || cur_i64_first(list, uid, "assigneeLocalIds")
                        on:change=move |ev| set_opt_i64_arr(list, uid, "assigneeLocalIds", parse_opt_i64(&event_target_value(&ev)))
                    >
                        <option value="">"any assignee"</option>
                        <option value="-1">"unassigned"</option>
                        {move || {
                            let m = meta.get();
                            meta_arr(&m, "assignees")
                                .into_iter()
                                .map(|a| {
                                    let id = a.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    view! { <option value=id.to_string()>{name}</option> }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </FieldRow>
                <FieldRow label="Created \u{2264} days">
                    <input
                        class="spp-input spp-cond-editor__num"
                        type="number"
                        min="1"
                        placeholder="any"
                        value=move || opt_f64_display(cur_of(list, uid, "createdWithinDays"))
                        on:change=move |ev| set_opt_f64(list, uid, "createdWithinDays", parse_opt_f64(&event_target_value(&ev)))
                    />
                </FieldRow>
                <FieldRow label="Modified \u{2264} days">
                    <input
                        class="spp-input spp-cond-editor__num"
                        type="number"
                        min="1"
                        placeholder="any"
                        value=move || opt_f64_display(cur_of(list, uid, "modifiedWithinDays"))
                        on:change=move |ev| set_opt_f64(list, uid, "modifiedWithinDays", parse_opt_f64(&event_target_value(&ev)))
                    />
                </FieldRow>
                <FieldRow label="Number \u{2265}">
                    <input
                        class="spp-input spp-cond-editor__num"
                        type="number"
                        placeholder="any"
                        value=move || opt_f64_display(cur_of(list, uid, "numberMin"))
                        on:change=move |ev| set_opt_f64(list, uid, "numberMin", parse_opt_f64(&event_target_value(&ev)))
                    />
                </FieldRow>
                <FieldRow label="Number \u{2264}">
                    <input
                        class="spp-input spp-cond-editor__num"
                        type="number"
                        placeholder="any"
                        value=move || opt_f64_display(cur_of(list, uid, "numberMax"))
                        on:change=move |ev| set_opt_f64(list, uid, "numberMax", parse_opt_f64(&event_target_value(&ev)))
                    />
                </FieldRow>
                <FieldRow label="Channel">
                    <select
                        class="spp-input spp-cond-editor__control"
                        value=move || cur_os(list, uid, "channel").unwrap_or_default()
                        on:change=move |ev| set_opt(list, uid, "channel", non_empty(event_target_value(&ev)))
                    >
                        <option value="">"any channel"</option>
                        {move || {
                            let m = meta.get();
                            meta_str_list(&m, "channels")
                                .into_iter()
                                .map(|ch| view! { <option value=ch.clone()>{ch}</option> })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </FieldRow>
            </div>
            {move || {
                let m = meta.get();
                let fields = meta_arr(&m, "ticket_custom_fields");
                if fields.is_empty() {
                    ().into_view()
                } else {
                    let fields: Vec<Value> = fields.into_iter().take(12).collect();
                    view! {
                        <div class="spp-flex spp-flex--wrap spp-mt-8">
                            {fields
                                .into_iter()
                                .map(|f| {
                                    let fid = f.get("local_id").and_then(|v| v.as_i64()).unwrap_or(0);
                                    let fname = f.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                    let label = format!("field: {fname}");
                                    view! {
                                        <button
                                            type="button"
                                            class="spp-chip"
                                            class:is-on=move || custom_fields_active(list, uid, fid)
                                            title=move || {
                                                if custom_fields_active(list, uid, fid) {
                                                    "Click to remove this custom-field filter"
                                                } else {
                                                    "Click to filter on this custom field (same conversation)"
                                                }
                                            }
                                            on:click=move |_| {
                                                with_custom_fields(list, uid, |cfs| {
                                                    if custom_fields_active(list, uid, fid) {
                                                        cfs.retain(|cf| {
                                                            cf.get("fieldLocalId").and_then(|v| v.as_i64()) != Some(fid)
                                                        });
                                                    } else {
                                                        cfs.push(json!({
                                                            "fieldLocalId": fid,
                                                            "op": "is_not_empty",
                                                            "value": null,
                                                        }));
                                                    }
                                                });
                                            }
                                        >
                                            {label}
                                        </button>
                                    }
                                })
                                .collect::<Vec<_>>()}
                            {move || {
                                let node = node_by(list, uid);
                                let cfs = node
                                    .get("customFields")
                                    .and_then(|v| v.as_array())
                                    .cloned()
                                    .unwrap_or_default();
                                cfs.iter()
                                    .enumerate()
                                    .map(|(idx, cf)| {
                                        let f_local = cf.get("fieldLocalId").and_then(|v| v.as_i64()).unwrap_or(0);
                                        let m2 = meta.get();
                                        let fmeta = meta_arr(&m2, "ticket_custom_fields")
                                            .into_iter()
                                            .find(|f| f.get("local_id").and_then(|v| v.as_i64()) == Some(f_local));
                                        let ftype = fmeta
                                            .as_ref()
                                            .and_then(|f| f.get("type"))
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("value")
                                            .to_string();
                                        view! {
                                            <div class="spp-flex spp-gap-4">
                                                <select
                                                    class="spp-input spp-cond-editor__control"
                                                    value=cf.get("op").and_then(|v| v.as_str()).unwrap_or("is_not_empty").to_string()
                                                    on:change=move |ev| {
                                                        let new_op = event_target_value(&ev);
                                                        with_custom_fields(list, uid, |cfs2| {
                                                            if let Some(row) = cfs2.get_mut(idx) {
                                                                if let Some(o) = row.as_object_mut() {
                                                                    o.insert("op".into(), json!(new_op));
                                                                }
                                                            }
                                                        });
                                                    }
                                                >
                                                    <option value="is_not_empty">"is set"</option>
                                                    <option value="is_empty">"is empty"</option>
                                                    <option value="equals">"="</option>
                                                    <option value="not_equals">"\u{2260}"</option>
                                                    <option value="contains">"contains"</option>
                                                </select>
                                                {move || {
                                                    let node2 = node_by(list, uid);
                                                    let op = node2
                                                        .get("customFields")
                                                        .and_then(|v| v.as_array())
                                                        .and_then(|a| a.get(idx).cloned())
                                                        .and_then(|cf2| cf2.get("op").and_then(|v| v.as_str()).map(str::to_string))
                                                        .unwrap_or_default();
                                                    if op == "is_empty" || op == "is_not_empty" {
                                                        ().into_view()
                                                    } else {
                                                        view! {
                                                            <input
                                                                class="spp-input spp-cond-editor__control"
                                                                placeholder=ftype.clone()
                                                                value=move || {
                                                                    node_by(list, uid)
                                                                        .get("customFields")
                                                                        .and_then(|v| v.as_array())
                                                                        .and_then(|a| a.get(idx).cloned())
                                                                        .and_then(|cf2| {
                                                                            cf2.get("value").and_then(|v| v.as_str()).map(str::to_string)
                                                                        })
                                                                        .unwrap_or_default()
                                                                }
                                                                on:change=move |ev| {
                                                                    let new_val = event_target_value(&ev);
                                                                    with_custom_fields(list, uid, |cfs2| {
                                                                        if let Some(row) = cfs2.get_mut(idx) {
                                                                            if let Some(o) = row.as_object_mut() {
                                                                                o.insert(
                                                                                    "value".into(),
                                                                                    if new_val.is_empty() { json!(null) } else { json!(new_val.clone()) },
                                                                                );
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                            />
                                                        }
                                                        .into_view()
                                                    }
                                                }}
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </div>
                    }
                    .into_view()
                }
            }}
        </div>
    }
}

fn custom_fields_active(list: NodeList, uid: u64, fid: i64) -> bool {
    node_by(list, uid)
        .get("customFields")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .any(|cf| cf.get("fieldLocalId").and_then(|v| v.as_i64()) == Some(fid))
        })
        .unwrap_or(false)
}

fn cur_i64_first(list: NodeList, uid: u64, key: &str) -> String {
    node_by(list, uid)
        .get(key)
        .and_then(|v| v.as_array())
        .and_then(|a| a.first().cloned())
        .and_then(|v| v.as_i64())
        .map(|i| i.to_string())
        .unwrap_or_default()
}

fn opt_f64_display(x: Option<f64>) -> String {
    x.map(num_str).unwrap_or_default()
}

#[component]
fn HistoryEditor(list: NodeList, uid: u64) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Metric">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_s(list, uid, "metric")
                    on:change=move |ev| set_str(list, uid, "metric", &event_target_value(&ev))
                >
                    <option value="ticket_count">"total tickets"</option>
                    <option value="open_count">"open tickets"</option>
                    <option value="closed_count">"closed tickets"</option>
                    <option value="last_contact_within_days">"last contact within (days)"</option>
                    <option value="first_contact_before_days">"first contact older than (days)"</option>
                    <option value="waited_over_hours_count">"waited over (hours) at least once"</option>
                </select>
            </FieldRow>
            <FieldRow label="Comparison">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "op")
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    <option value="gte">"is at least"</option>
                    <option value="lte">"is at most"</option>
                    <option value="eq">"equals"</option>
                </select>
            </FieldRow>
            <FieldRow label="Value">
                <input
                    class="spp-input spp-cond-editor__num"
                    type="number"
                    value=move || num_str(cur_of(list, uid, "value").unwrap_or(0.0))
                    on:change=move |ev| set_req_f64(list, uid, "value", parse_req_f64(&event_target_value(&ev)))
                />
            </FieldRow>
        </div>
    }
}

#[component]
fn HistoryTagEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    let datalist_id = format!("outreach-tag-list-ht-{uid}");
    let datalist_id_input = datalist_id.clone();
    let datalist_id_list = datalist_id.clone();
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Tag">
                <input
                    class="spp-input spp-cond-editor__control"
                    placeholder="timezone"
                    attr:list=datalist_id_input.clone()
                    value=move || cur_s(list, uid, "tag")
                    on:change=move |ev| set_str(list, uid, "tag", &event_target_value(&ev))
                />
                <datalist id=datalist_id_list.clone()>
                    {move || {
                        let m = meta.get();
                        meta_str_list(&m, "tags")
                            .into_iter()
                            .map(|t| view! { <option value=t.clone()>{t}</option> })
                            .collect::<Vec<_>>()
                    }}
                </datalist>
            </FieldRow>
            <FieldRow label="Within (days)">
                <input
                    class="spp-input spp-cond-editor__num"
                    type="number"
                    min="1"
                    placeholder="any time"
                    value=move || opt_f64_display(cur_of(list, uid, "withinDays"))
                    on:change=move |ev| set_opt_f64(list, uid, "withinDays", parse_opt_f64(&event_target_value(&ev)))
                />
            </FieldRow>
            <span class="spp-muted spp-text-xs spp-cond-editor__hint">
                "Customer has at least one ticket with this tag (across their whole history)."
            </span>
        </div>
    }
}

#[component]
fn OrganizationPropertyEditor(
    list: NodeList,
    uid: u64,
    meta: RwSignal<Option<Value>>,
) -> impl IntoView {
    let using_standard = move || {
        let f = cur_s(list, uid, "field");
        f == "name" || f == "domains"
    };
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Field">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || {
                        if using_standard() {
                            cur_s(list, uid, "field")
                        } else {
                            cur_oi(list, uid, "definitionId").map(|i| i.to_string()).unwrap_or_default()
                        }
                    }
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        if v == "name" || v == "domains" {
                            set_fields(list, uid, vec![
                                ("field", json!(v.clone())),
                                ("definitionId", json!(null)),
                                ("op", json!("contains")),
                                ("value", json!("")),
                            ]);
                        } else {
                            let m = meta.get();
                            let def = meta_arr(&m, "organization_property_definitions")
                                .into_iter()
                                .find(|d| d.get("id").and_then(|x| x.as_i64()).map(|i| i.to_string()) == Some(v.clone()));
                            set_fields(list, uid, vec![
                                ("field", json!(null)),
                                ("definitionId", json!(def.as_ref().and_then(|d| d.get("id")).and_then(|x| x.as_i64()))),
                                ("name", json!(def.as_ref().and_then(|d| d.get("name")).and_then(|x| x.as_str()))),
                                ("type", json!(def.as_ref().and_then(|d| d.get("type")).and_then(|x| x.as_str()).unwrap_or("text"))),
                                ("op", json!("equals")),
                                ("value", json!("")),
                            ]);
                        }
                    }
                >
                    <option value="name">"Organization name"</option>
                    <option value="domains">"Organization domains"</option>
                    {move || {
                        let m = meta.get();
                        meta_arr(&m, "organization_property_definitions")
                            .into_iter()
                            .map(|d| {
                                let id = d.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let populated = d.get("populated").and_then(|v| v.as_i64()).unwrap_or(0);
                                let label = if populated == 0 {
                                    format!("{name} \u{b7} org property (no values locally)")
                                } else {
                                    format!("{name} \u{b7} org property")
                                };
                                view! { <option value=id.to_string()>{label}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Operator">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "op")
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    {move || {
                        let ops = if using_standard() {
                            STANDARD_ORG_OPS.iter().map(|o| o.to_string()).collect::<Vec<_>>()
                        } else {
                            let m = meta.get();
                            ops_for_type(&m, "text")
                        };
                        ops.iter()
                            .map(|op| view! { <option value=op.clone()>{op_label(op)}</option> })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            {move || {
                let op = cur_s(list, uid, "op");
                if op == "is_empty" || op == "is_not_empty" {
                    ().into_view()
                } else {
                    view! {
                        <FieldRow label="Value">
                            <input
                                class="spp-input spp-cond-editor__control"
                                placeholder=move || {
                                    if using_standard() && cur_s(list, uid, "field") == "domains" {
                                        "company.com".to_string()
                                    } else {
                                        "value".to_string()
                                    }
                                }
                                value=move || cur_os(list, uid, "value").unwrap_or_default()
                                on:change=move |ev| set_opt(list, uid, "value", non_empty(event_target_value(&ev)))
                            />
                        </FieldRow>
                    }
                    .into_view()
                }
            }}
        </div>
    }
}

const STANDARD_ORG_OPS: [&str; 7] = [
    "equals",
    "not_equals",
    "contains",
    "starts_with",
    "ends_with",
    "is_empty",
    "is_not_empty",
];

#[component]
fn HistoryIssueEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Issue kind">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "issueKind")
                    on:change=move |ev| {
                        set_fields(list, uid, vec![
                            ("issueKind", json!(event_target_value(&ev))),
                            ("issueLocalId", json!(null)),
                        ]);
                    }
                >
                    <option value="known_issue">"Known issues"</option>
                    <option value="cluster">"Issue clusters"</option>
                </select>
            </FieldRow>
            <FieldRow label="Issue">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_oi(list, uid, "issueLocalId").map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| set_opt_i64(list, uid, "issueLocalId", parse_opt_i64(&event_target_value(&ev)))
                >
                    {move || {
                        let m = meta.get();
                        let kind = cur_s(list, uid, "issueKind");
                        let want = if kind == "cluster" { "cluster" } else { "known_issue" };
                        let any_label = if want == "cluster" { "any cluster" } else { "any known issue" };
                        let mut out = vec![view! { <option value="">{any_label}</option> }];
                        out.extend(meta_arr(&m, "issues").into_iter().filter(|i| {
                            i.get("kind").and_then(|v| v.as_str()) == Some(want)
                        }).map(|i| {
                            let id = i.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let label: String = i
                                .get("label")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default()
                                .chars()
                                .take(60)
                                .collect();
                            view! { <option value=id.to_string()>{label}</option> }
                        }));
                        out
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Linked conversations">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "op")
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    <option value="gte">"at least"</option>
                    <option value="eq">"none (complement)"</option>
                </select>
            </FieldRow>
            {move || {
                if cur_s(list, uid, "op") == "gte" {
                    view! {
                        <FieldRow label="Count">
                            <input
                                class="spp-input spp-cond-editor__num"
                                type="number"
                                min="1"
                                value=move || num_str(cur_of(list, uid, "value").unwrap_or(0.0))
                                on:change=move |ev| set_req_f64(list, uid, "value", parse_req_f64(&event_target_value(&ev)))
                            />
                        </FieldRow>
                    }
                    .into_view()
                } else {
                    ().into_view()
                }
            }}
        </div>
    }
}

#[component]
fn IncidentExposureEditor(
    list: NodeList,
    uid: u64,
    meta: RwSignal<Option<Value>>,
) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Incident">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_oi(list, uid, "incidentId").map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| set_opt_i64(list, uid, "incidentId", parse_opt_i64(&event_target_value(&ev)))
                >
                    <option value="">"any ACTIVE incident"</option>
                    {move || {
                        let m = meta.get();
                        meta_arr(&m, "incidents")
                            .into_iter()
                            .map(|i| {
                                let id = i.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let code = i.get("code").and_then(|v| v.as_str()).unwrap_or("");
                                let title: String = i
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .chars()
                                    .take(50)
                                    .collect();
                                let status = i.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let label = format!("{code} \u{b7} {title} ({status})");
                                view! { <option value=id.to_string()>{label}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Within (days)">
                <input
                    class="spp-input spp-cond-editor__num"
                    type="number"
                    min="1"
                    placeholder="any time"
                    value=move || opt_f64_display(cur_of(list, uid, "withinDays"))
                    on:change=move |ev| set_opt_f64(list, uid, "withinDays", parse_opt_f64(&event_target_value(&ev)))
                />
            </FieldRow>
            <span class="spp-muted spp-text-xs spp-cond-editor__hint">
                "Customers whose conversations are linked to the incident."
            </span>
        </div>
    }
}

#[component]
fn CampaignHistoryEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Relation">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "relation")
                    on:change=move |ev| set_str(list, uid, "relation", &event_target_value(&ev))
                >
                    <option value="received">"received a campaign"</option>
                    <option value="replied">"replied to a campaign"</option>
                    <option value="not_received">"never received a campaign"</option>
                </select>
            </FieldRow>
            <FieldRow label="Campaign">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_oi(list, uid, "campaignId").map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| set_opt_i64(list, uid, "campaignId", parse_opt_i64(&event_target_value(&ev)))
                >
                    <option value="">"any campaign"</option>
                    {move || {
                        let m = meta.get();
                        meta_arr(&m, "campaigns")
                            .into_iter()
                            .map(|cm| {
                                let id = cm.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let name = cm.get("name").and_then(|v| v.as_str()).unwrap_or("");
                                let status = cm.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                let label = format!("{name} ({status})");
                                view! { <option value=id.to_string()>{label}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
        </div>
    }
}

#[component]
fn SupportHealthEditor(list: NodeList, uid: u64) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Metric">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_s(list, uid, "metric")
                    on:change=move |ev| set_str(list, uid, "metric", &event_target_value(&ev))
                >
                    {HEALTH_METRICS
                        .iter()
                        .map(|(k, v)| view! { <option value=*k>{*v}</option> })
                        .collect::<Vec<_>>()}
                </select>
            </FieldRow>
            <FieldRow label="Comparison">
                <select
                    class="spp-input spp-cond-editor__control"
                    value=move || cur_s(list, uid, "op")
                    on:change=move |ev| set_str(list, uid, "op", &event_target_value(&ev))
                >
                    <option value="gte">"is at least"</option>
                    <option value="lte">"is at most"</option>
                </select>
            </FieldRow>
            <FieldRow label="Value">
                <input
                    class="spp-input spp-cond-editor__num"
                    type="number"
                    step="0.1"
                    value=move || num_str(cur_of(list, uid, "value").unwrap_or(0.0))
                    on:change=move |ev| set_req_f64(list, uid, "value", parse_req_f64(&event_target_value(&ev)))
                />
            </FieldRow>
            <span class="spp-muted spp-text-xs spp-cond-editor__hint">
                "Deterministic aggregates over the local mirror (ratings, effort scores)."
            </span>
        </div>
    }
}

const HEALTH_METRICS: [(&str, &str); 4] = [
    ("avg_rating", "average rating (1-5)"),
    ("avg_effort_score", "average effort score (0-10)"),
    (
        "first_response_resolution_rate",
        "first-response resolution rate (0-1)",
    ),
    ("high_friction_rate", "high-friction rate (0-1)"),
];

#[component]
fn CustomObjectLinkEditor(
    list: NodeList,
    uid: u64,
    meta: RwSignal<Option<Value>>,
) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Object type">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_oi(list, uid, "typeId").map(|i| i.to_string()).unwrap_or_default()
                    on:change=move |ev| set_opt_i64(list, uid, "typeId", parse_opt_i64(&event_target_value(&ev)))
                >
                    <option value="">"any custom object type"</option>
                    {move || {
                        let m = meta.get();
                        meta_arr(&m, "custom_object_types")
                            .into_iter()
                            .map(|t| {
                                let id = t.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                view! { <option value=id.to_string()>{name}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            <span class="spp-muted spp-text-xs spp-cond-editor__hint">
                "Customers linked to at least one object of this type (locally defined records)."
            </span>
        </div>
    }
}

#[component]
fn CustomerEventEditor(list: NodeList, uid: u64, meta: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        <div class="spp-flex spp-flex--wrap">
            <FieldRow label="Event kind">
                <select
                    class="spp-input spp-cond-editor__control--wide"
                    value=move || cur_s(list, uid, "eventKind")
                    on:change=move |ev| set_str(list, uid, "eventKind", &event_target_value(&ev))
                >
                    {move || {
                        let m = meta.get();
                        let kinds = meta_str_list(&m, "customer_event_kinds");
                        let kinds = if kinds.is_empty() {
                            vec!["campaign_reply".to_string()]
                        } else {
                            kinds
                        };
                        kinds
                            .into_iter()
                            .map(|k| {
                                let label = k.replace('_', " ");
                                view! { <option value=k.clone()>{label}</option> }
                            })
                            .collect::<Vec<_>>()
                    }}
                </select>
            </FieldRow>
            <FieldRow label="Within (days)">
                <input
                    class="spp-input spp-cond-editor__num"
                    type="number"
                    min="1"
                    placeholder="any time"
                    value=move || opt_f64_display(cur_of(list, uid, "withinDays"))
                    on:change=move |ev| set_opt_f64(list, uid, "withinDays", parse_opt_f64(&event_target_value(&ev)))
                />
            </FieldRow>
            <span class="spp-muted spp-text-xs spp-cond-editor__hint">
                "Customer timeline includes at least one event of this kind."
            </span>
        </div>
    }
}

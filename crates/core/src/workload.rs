use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const CAPACITY_WINDOW_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkload {
    pub agent_remote_id: i64,
    pub assigned: u32,
    pub active: u32,
    pub resolved_today: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamWorkload {
    pub team_remote_id: i64,
    pub agent_count: u32,
    pub assigned: u32,
    pub active: u32,
    pub resolved_today: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CapacityMetrics {
    pub incoming_7d: u32,
    pub closing_7d: u32,
    pub window_days: i64,
    pub incoming_rate_per_day: f64,
    pub closing_rate_per_day: f64,
}

pub fn agent_workload(conn: &Connection, agent_remote_id: i64) -> Result<AgentWorkload> {
    let assigned: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE assignee_local_id = ?1 AND status != 'closed'",
        params![agent_remote_id],
        |r| r.get(0),
    )?;

    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE assignee_local_id = ?1 AND status = 'active'",
        params![agent_remote_id],
        |r| r.get(0),
    )?;

    let resolved_today: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE assignee_local_id = ?1
           AND status = 'closed'
           AND closed_at IS NOT NULL
           AND julianday(closed_at) >= julianday('now', 'start of day')
           AND julianday(closed_at) < julianday('now', '+1 day', 'start of day')",
        params![agent_remote_id],
        |r| r.get(0),
    )?;

    Ok(AgentWorkload {
        agent_remote_id,
        assigned: u32::try_from(assigned).unwrap_or(0),
        active: u32::try_from(active).unwrap_or(0),
        resolved_today: u32::try_from(resolved_today).unwrap_or(0),
    })
}

pub fn team_workload(
    conn: &Connection,
    team_remote_id: i64,
    member_remote_ids: &[i64],
) -> Result<TeamWorkload> {
    let mut rollup = TeamWorkload {
        team_remote_id,
        agent_count: u32::try_from(member_remote_ids.len()).unwrap_or(0),
        ..Default::default()
    };
    for &agent_remote_id in member_remote_ids {
        let w = agent_workload(conn, agent_remote_id)?;
        rollup.assigned += w.assigned;
        rollup.active += w.active;
        rollup.resolved_today += w.resolved_today;
    }
    Ok(rollup)
}

pub fn capacity_metrics(conn: &Connection) -> Result<CapacityMetrics> {
    let incoming_7d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE julianday(local_created_at) >= julianday('now', ?1)",
        params![format!("-{CAPACITY_WINDOW_DAYS} days")],
        |r| r.get(0),
    )?;

    let closing_7d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations
         WHERE closed_at IS NOT NULL
           AND status = 'closed'
           AND julianday(closed_at) >= julianday('now', ?1)",
        params![format!("-{CAPACITY_WINDOW_DAYS} days")],
        |r| r.get(0),
    )?;

    let incoming = u32::try_from(incoming_7d).unwrap_or(0);
    let closing = u32::try_from(closing_7d).unwrap_or(0);
    let window = CAPACITY_WINDOW_DAYS as f64;

    Ok(CapacityMetrics {
        incoming_7d: incoming,
        closing_7d: closing,
        window_days: CAPACITY_WINDOW_DAYS,
        incoming_rate_per_day: incoming as f64 / window,
        closing_rate_per_day: closing as f64 / window,
    })
}

pub const CAPACITY_SETTING_KEY: &str = "capacity_model";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CapacityWeights {
    pub urgent: f64,
    pub sla: f64,
    pub waiting: f64,
    pub open: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapacityModel {
    pub default_max_open: i64,
    pub per_user_max: std::collections::BTreeMap<String, i64>,
    pub weights: CapacityWeights,
}

pub fn default_capacity_model() -> CapacityModel {
    CapacityModel {
        default_max_open: 25,
        per_user_max: std::collections::BTreeMap::new(),
        weights: CapacityWeights {
            urgent: 3.0,
            sla: 2.0,
            waiting: 1.5,
            open: 1.0,
        },
    }
}

impl CapacityModel {
    #[must_use]
    pub fn capacity_for(&self, user_id: i64) -> i64 {
        self.per_user_max
            .get(&user_id.to_string())
            .copied()
            .unwrap_or(self.default_max_open)
    }
}

pub fn parse_capacity_model(value: &serde_json::Value) -> Option<CapacityModel> {
    validate_capacity_model(value).ok()
}

pub fn validate_capacity_model(
    value: &serde_json::Value,
) -> std::result::Result<CapacityModel, Vec<String>> {
    let mut issues = Vec::new();
    let Some(obj) = value.as_object() else {
        return Err(vec!["capacity model must be an object.".to_string()]);
    };
    let default_max_open = match obj.get("default_max_open") {
        Some(v) => match v.as_i64() {
            Some(n) if (1..=500).contains(&n) => n,
            _ => {
                issues.push("default_max_open: must be an integer between 1 and 500.".to_string());
                25
            }
        },
        None => {
            issues.push("default_max_open: Required".to_string());
            25
        }
    };
    let mut per_user_max = std::collections::BTreeMap::new();
    match obj.get("per_user_max") {
        Some(serde_json::Value::Object(per_raw)) => {
            for (k, v) in per_raw {
                if k.is_empty() || !k.chars().all(|c| c.is_ascii_digit()) {
                    issues.push(format!(
                        "per_user_max.{k}: keys must be local user ids (digits)."
                    ));
                    continue;
                }
                match v.as_i64() {
                    Some(n) if (1..=500).contains(&n) => {
                        per_user_max.insert(k.clone(), n);
                    }
                    _ => issues.push(format!(
                        "per_user_max.{k}: must be an integer between 1 and 500."
                    )),
                }
            }
        }
        _ => issues.push("per_user_max: Required".to_string()),
    }
    let mut weights = CapacityWeights {
        urgent: 3.0,
        sla: 2.0,
        waiting: 1.5,
        open: 1.0,
    };
    match obj.get("weights") {
        Some(serde_json::Value::Object(w)) => {
            for key in ["urgent", "sla", "waiting", "open"] {
                match w.get(key).and_then(|v| v.as_f64()) {
                    Some(n) if n.is_finite() && (0.0..=100.0).contains(&n) => match key {
                        "urgent" => weights.urgent = n,
                        "sla" => weights.sla = n,
                        "waiting" => weights.waiting = n,
                        _ => weights.open = n,
                    },
                    _ => issues.push(format!(
                        "weights.{key}: must be a number between 0 and 100."
                    )),
                }
            }
        }
        _ => issues.push("weights: Required".to_string()),
    }
    if issues.is_empty() {
        Ok(CapacityModel {
            default_max_open,
            per_user_max,
            weights,
        })
    } else {
        Err(issues)
    }
}

pub fn get_capacity_model(conn: &Connection) -> CapacityModel {
    crate::settings::get_json::<serde_json::Value>(conn, CAPACITY_SETTING_KEY)
        .ok()
        .flatten()
        .and_then(|v| parse_capacity_model(&v))
        .unwrap_or_else(default_capacity_model)
}

pub fn set_capacity_model(
    conn: &Connection,
    model: &CapacityModel,
) -> std::result::Result<(), String> {
    let value = serde_json::to_value(model)
        .map_err(|e| format!("capacity model serialization failed: {e}"))?;
    match parse_capacity_model(&value) {
        Some(_) => {
            crate::settings::set_json(conn, CAPACITY_SETTING_KEY, model).map_err(|e| e.to_string())
        }
        None => Err("capacity model failed validation".to_string()),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSnapshot {
    pub user_local_id: i64,
    pub display_name: String,
    pub mention: Option<String>,
    pub role: Option<String>,
    pub availability: Availability,
    pub open_workload: i64,
    pub pending_workload: i64,
    pub customer_waiting_workload: i64,
    pub urgent_workload: i64,
    pub sla_risk_workload: i64,
    pub weighted_load: f64,
    pub capacity: i64,
    pub pressure: f64,
    pub avg_active_load_7d: Option<f64>,
    pub recent_closed_7d: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Availability {
    pub email_status: Option<String>,
    pub chat_status: Option<String>,
    pub source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeamSnapshot {
    pub team_local_id: i64,
    pub name: String,
    pub member_user_local_ids: Vec<i64>,
    pub open_workload: i64,
    pub weighted_load: f64,
    pub capacity: i64,
    pub pressure: f64,
    pub recent_closed_7d: i64,
    pub available_members: i64,
    pub total_members: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadSnapshot {
    pub generated_at: String,
    pub unassigned_work: i64,
    pub agents: Vec<AgentSnapshot>,
    pub teams: Vec<TeamSnapshot>,
    pub capacity_model: CapacityModel,
    pub method_notes: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestedAssignee {
    pub conversation_id: i64,
    pub conversation_number: Option<i64>,
    pub subject: Option<String>,
    pub supportos_priority: Option<String>,
    pub waiting_minutes: Option<i64>,
    pub suggested_user_local_id: Option<i64>,
    pub suggested_display_name: Option<String>,
    pub suggested_pressure_after: Option<f64>,
    pub suggested_availability: Option<String>,
    pub all_away: bool,
    pub reason: String,
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

struct UserRow {
    id: i64,
    first_name: Option<String>,
    last_name: Option<String>,
    mention: Option<String>,
    role: Option<String>,
}

pub fn snapshot(conn: &Connection) -> Result<WorkloadSnapshot> {
    let model = get_capacity_model(conn);
    let mut users_stmt = conn.prepare(
        "SELECT id, first_name, last_name, mention, role
           FROM users WHERE deleted_at IS NULL ORDER BY id",
    )?;
    let users: Vec<UserRow> = users_stmt
        .query_map([], |r| {
            Ok(UserRow {
                id: r.get(0)?,
                first_name: r.get(1)?,
                last_name: r.get(2)?,
                mention: r.get(3)?,
                role: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(users_stmt);

    let alerts = crate::sla::sla_alerts(conn)?;
    let mut sla_by_assignee: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    for a in &alerts.alerts {
        if let Some(assignee) = a.assignee_local_id {
            sla_by_assignee
                .entry(assignee)
                .or_default()
                .push(a.conversation_id);
        }
    }

    let mut agents = Vec::with_capacity(users.len());
    for user in users {
        let user_id = user.id;
        let display = {
            let name = [user.first_name.as_deref(), user.last_name.as_deref()]
                .iter()
                .filter_map(|p| p.filter(|s| !s.is_empty()))
                .collect::<Vec<_>>()
                .join(" ");
            if name.is_empty() {
                conn.query_row(
                    "SELECT email FROM users WHERE id = ?1",
                    params![user_id],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| format!("user #{user_id}"))
            } else {
                name
            }
        };
        let availability = availability_of(conn, user_id);
        let counts = agent_counts(conn, user_id)?;
        let capacity = model.capacity_for(user_id);
        let weighted = weighted_load(
            conn,
            user_id,
            &model.weights,
            sla_by_assignee
                .get(&user_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
        agents.push(AgentSnapshot {
            user_local_id: user_id,
            display_name: display,
            mention: user.mention,
            role: user.role,
            availability,
            open_workload: counts.open,
            pending_workload: counts.pending,
            customer_waiting_workload: counts.waiting,
            urgent_workload: counts.urgent,
            sla_risk_workload: sla_by_assignee.get(&user_id).map(Vec::len).unwrap_or(0) as i64,
            weighted_load: round2(weighted),
            capacity,
            pressure: if capacity > 0 {
                round2(weighted / capacity as f64)
            } else {
                0.0
            },
            avg_active_load_7d: avg_active_load_7d(conn, user_id),
            recent_closed_7d: counts.closed7d,
        });
    }

    let teams = team_snapshots(conn, &agents, &model);

    let unassigned: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations c
          WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL
            AND c.status IN ('active','pending') AND c.assignee_local_id IS NULL",
        [],
        |r| r.get(0),
    )?;

    Ok(WorkloadSnapshot {
        generated_at: chrono::Utc::now().to_rfc3339(),
        unassigned_work: unassigned,
        agents,
        teams,
        capacity_model: model,
        method_notes: vec![
            "Pressure = weighted open load / capacity. Each open conversation counts once at its highest tier: urgent, SLA risk, customer-waiting, or plain open.",
            "Average active load is an approximation from created/closed timestamps using the CURRENT assignee; assignment changes mid-conversation are not historically reconstructable.",
            "Availability is the Help Scout user status we already sync (email/chat). Agents with no synced status are shown as unknown, never guessed.",
            "Suggested assignee is a read-only recommendation. SupportOS never reassigns automatically unless you explicitly enable an approved automation.",
        ],
    })
}

struct AgentCounts {
    open: i64,
    pending: i64,
    waiting: i64,
    urgent: i64,
    closed7d: i64,
}

fn agent_counts(conn: &Connection, user_id: i64) -> Result<AgentCounts> {
    let (open, pending, waiting, urgent): (i64, i64, i64, i64) = conn.query_row(
        "SELECT
             SUM(CASE WHEN c.status IN ('active','pending') THEN 1 ELSE 0 END),
             SUM(CASE WHEN c.status = 'pending' THEN 1 ELSE 0 END),
             SUM(CASE WHEN c.status = 'active' AND c.customer_waiting_since IS NOT NULL THEN 1 ELSE 0 END),
             SUM(CASE WHEN c.status IN ('active','pending') AND c.supportos_priority IN ('high','urgent') THEN 1 ELSE 0 END)
           FROM conversations c
          WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL
            AND c.assignee_local_id = ?1",
        params![user_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .unwrap_or((0, 0, 0, 0));
    let closed7d: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversations c
          WHERE c.deleted_at IS NULL AND c.assignee_local_id = ?1 AND c.status = 'closed'
            AND c.closed_at IS NOT NULL
            AND julianday(c.closed_at) >= julianday('now', '-7 days')",
        params![user_id],
        |r| r.get(0),
    )?;
    Ok(AgentCounts {
        open,
        pending,
        waiting,
        urgent,
        closed7d,
    })
}

fn weighted_load(
    conn: &Connection,
    user_id: i64,
    weights: &CapacityWeights,
    sla_conversation_ids: &[i64],
) -> f64 {
    let sla: std::collections::HashSet<i64> = sla_conversation_ids.iter().copied().collect();
    let mut stmt = match conn.prepare(
        "SELECT c.id, c.supportos_priority, c.status, c.customer_waiting_since
           FROM conversations c
          WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL
            AND c.assignee_local_id = ?1 AND c.status IN ('active','pending')",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return 0.0,
    };
    let rows: Vec<(i64, Option<String>, String, Option<String>)> = match stmt
        .query_map(params![user_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => return 0.0,
    };
    let total: f64 = rows
        .into_iter()
        .map(|(id, priority, status, waiting)| {
            if priority
                .as_deref()
                .is_some_and(|p| p == "high" || p == "urgent")
            {
                weights.urgent
            } else if sla.contains(&id) {
                weights.sla
            } else if status == "active" && waiting.is_some() {
                weights.waiting
            } else {
                weights.open
            }
        })
        .sum();
    total
}

fn avg_active_load_7d(conn: &Connection, user_id: i64) -> Option<f64> {
    let mut total = 0i64;
    for i in 0..7 {
        let day = chrono::Utc::now() - chrono::Duration::days(i);
        let day = day.format("%Y-%m-%d").to_string();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversations c
                  WHERE c.deleted_at IS NULL AND c.assignee_local_id = ?1
                    AND date(COALESCE(c.remote_created_at, c.created_at, c.local_created_at)) <= date(?2)
                    AND (c.closed_at IS NULL OR date(c.closed_at) > date(?2))",
                params![user_id, day],
                |r| r.get(0),
            )
            .unwrap_or(0);
        total += n;
    }
    Some(round2(total as f64 / 7.0))
}

fn availability_of(conn: &Connection, user_id: i64) -> Availability {
    let row = conn
        .query_row(
            "SELECT email_status, chat_status FROM user_statuses WHERE user_local_id = ?1",
            params![user_id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .ok();
    match row {
        Some((email_status, chat_status)) => Availability {
            email_status,
            chat_status,
            source: "user_statuses",
        },
        None => Availability {
            email_status: None,
            chat_status: None,
            source: "unknown",
        },
    }
}

fn team_snapshots(
    conn: &Connection,
    agents: &[AgentSnapshot],
    model: &CapacityModel,
) -> Vec<TeamSnapshot> {
    let by_user: std::collections::HashMap<i64, &AgentSnapshot> =
        agents.iter().map(|a| (a.user_local_id, a)).collect();
    let teams: Vec<(i64, String)> = {
        let Ok(mut stmt) =
            conn.prepare("SELECT id, name FROM teams WHERE deleted_at IS NULL ORDER BY name")
        else {
            return Vec::new();
        };
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();
        rows
    };
    let mut out = Vec::new();
    for (team_id, name) in teams {
        let member_ids: Vec<i64> = {
            let Ok(mut stmt) = conn
                .prepare("SELECT user_id FROM team_members WHERE team_id = ?1 ORDER BY user_id")
            else {
                continue;
            };
            let rows: Vec<i64> = stmt
                .query_map(params![team_id], |r| r.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
            rows
        };
        let members: Vec<&AgentSnapshot> = member_ids
            .iter()
            .filter_map(|id| by_user.get(id).copied())
            .collect();
        if members.is_empty() {
            continue;
        }
        let open: i64 = members.iter().map(|m| m.open_workload).sum();
        let weighted: f64 = members.iter().map(|m| m.weighted_load).sum();
        let capacity: i64 = members
            .iter()
            .map(|m| model.capacity_for(m.user_local_id))
            .sum();
        out.push(TeamSnapshot {
            team_local_id: team_id,
            name,
            member_user_local_ids: member_ids,
            open_workload: open,
            weighted_load: round2(weighted),
            capacity,
            pressure: if capacity > 0 {
                round2(weighted / capacity as f64)
            } else {
                0.0
            },
            recent_closed_7d: members.iter().map(|m| m.recent_closed_7d).sum(),
            available_members: members
                .iter()
                .filter(|m| {
                    m.availability.email_status.as_deref() == Some("active")
                        || m.availability.chat_status.as_deref() == Some("active")
                })
                .count() as i64,
            total_members: members.len() as i64,
        });
    }
    out
}

struct UnassignedRow {
    id: i64,
    number: Option<i64>,
    subject: Option<String>,
    priority: Option<String>,
    waiting_since: Option<String>,
}

pub fn suggested_assignees(conn: &Connection, limit: u32) -> Result<Vec<SuggestedAssignee>> {
    let model = get_capacity_model(conn);
    let snap = snapshot(conn)?;
    let agents = snap.agents;
    let limit = limit.clamp(1, 50) as i64;
    let mut rows_stmt = conn.prepare(
        "SELECT c.id, c.number, c.subject, c.supportos_priority, c.customer_waiting_since
           FROM conversations c
          WHERE c.deleted_at IS NULL AND c.merged_into_conversation_id IS NULL
            AND c.status IN ('active','pending') AND c.assignee_local_id IS NULL
          ORDER BY CASE c.supportos_priority
                     WHEN 'urgent' THEN 0 WHEN 'high' THEN 1
                     WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END,
                   c.customer_waiting_since IS NULL, c.customer_waiting_since
          LIMIT ?1",
    )?;
    let rows: Vec<UnassignedRow> = rows_stmt
        .query_map(params![limit], |r| {
            Ok(UnassignedRow {
                id: r.get(0)?,
                number: r.get(1)?,
                subject: r.get(2)?,
                priority: r.get(3)?,
                waiting_since: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(rows_stmt);

    Ok(rows
        .into_iter()
        .map(|row| {
            let conv_id = row.id;
            let priority = row.priority;
            let is_urgent = matches!(priority.as_deref(), Some("urgent") | Some("high"));
            let convo_weight = if is_urgent {
                model.weights.urgent
            } else {
                model.weights.open
            };
            let mut best: Option<(&AgentSnapshot, f64)> = None;
            let mut best_available: Option<(&AgentSnapshot, f64)> = None;
            for a in &agents {
                let after =
                    round2((a.weighted_load + convo_weight) / f64::max(1.0, a.capacity as f64));
                let is_away = a.availability.email_status.as_deref() == Some("away")
                    && a.availability.chat_status.as_deref().unwrap_or("away") == "away";
                let better = |cur: Option<(&AgentSnapshot, f64)>| match cur {
                    Some((agent, value)) => {
                        after < value || (after == value && a.user_local_id < agent.user_local_id)
                    }
                    None => true,
                };
                if better(best) {
                    best = Some((a, after));
                }
                if !is_away && better(best_available) {
                    best_available = Some((a, after));
                }
            }
            let chosen = best_available.or(best);
            let all_away = best_available.is_none();
            let waiting_minutes = row.waiting_since.as_deref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s).ok().map(|t| {
                    (chrono::Utc::now() - t.with_timezone(&chrono::Utc))
                        .num_minutes()
                        .max(0)
                })
            });
            let reason = match chosen {
                Some((_agent, after)) => format!(
                    "Lowest resulting pressure ({after}){}.",
                    if all_away {
                        " - all agents are away, least-loaded picked"
                    } else {
                        " among available agents"
                    }
                ),
                None => "No agents synced.".to_string(),
            };
            SuggestedAssignee {
                conversation_id: conv_id,
                conversation_number: row.number,
                subject: row.subject,
                supportos_priority: priority,
                waiting_minutes,
                suggested_user_local_id: chosen.map(|(a, _)| a.user_local_id),
                suggested_display_name: chosen.map(|(a, _)| a.display_name.clone()),
                suggested_pressure_after: chosen.map(|(_, after)| after),
                suggested_availability: chosen.map(|(a, _)| {
                    format!(
                        "{} / {}",
                        a.availability.email_status.as_deref().unwrap_or("unknown"),
                        a.availability.chat_status.as_deref().unwrap_or("unknown")
                    )
                }),
                all_away,
                reason,
            }
        })
        .collect())
}

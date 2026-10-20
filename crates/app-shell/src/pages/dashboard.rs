//! Dashboard page (port of the web `pages/dashboard.rs`).
//!
//! The model re-implements the `/api/analytics/dashboard` SQL from
//! `core::http::routes::analytics` as typed queries over the local
//! connection — same predicates, same rounding, no JSON layer and no HTTP
//! round-trip. Reads are pure: the HTTP route's `daily_metrics` cache
//! upsert is a write-on-read side effect of the request path and is
//! intentionally not reproduced here.
//!
//! The view keeps the web page's information architecture: date-range
//! selector, mailbox chips, channel filter, metric cards, per-day line
//! chart, breakdown tables, mailbox comparison, and the issue
//! radar panel (via the promoted `issue_radar_impl`).

use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;

use crate::chart;
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Date ranges offered by the web page.
pub const RANGES: [(i64, &str); 4] = [
    (7, "7 days"),
    (30, "30 days"),
    (90, "90 days"),
    (365, "1 year"),
];

/// Channel filter values ("" = all).
const CHANNELS: [(&str, &str); 3] = [
    ("", "All channels"),
    ("email", "Email"),
    ("chat", "Chat (Beacon)"),
];

/// (name, count) breakdown row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameCount {
    pub name: String,
    pub count: i64,
}

/// One point of the per-day series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyPoint {
    pub date: String,
    pub value: i64,
}

/// Per-channel metric row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelMetric {
    pub channel: String,
    pub count: i64,
    pub first_response_avg_min: Option<i64>,
    pub resolution_avg_min: Option<i64>,
}

/// Mailbox comparison row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailboxComparison {
    pub mailbox_id: i64,
    pub name: String,
    pub new_conversations: i64,
    pub active_conversations: i64,
    pub closed_conversations: i64,
    pub backlog: i64,
    pub first_response_avg_min: Option<i64>,
    pub resolution_avg_min: Option<i64>,
    pub great_ratings: i64,
    pub total_ratings: i64,
}

/// Typed dashboard payload (the analytics route's JSON shape).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DashboardData {
    pub range_from: String,
    pub range_to: String,
    pub new_conversations: i64,
    pub active_conversations: i64,
    pub pending_conversations: i64,
    pub closed_conversations: i64,
    pub unassigned: i64,
    pub backlog: i64,
    pub first_response_avg_min: Option<i64>,
    pub resolution_avg_min: Option<i64>,
    pub replies_sent: i64,
    pub ratings_great: i64,
    pub ratings_okay: i64,
    pub ratings_not_good: i64,
    pub by_mailbox: Vec<NameCount>,
    pub by_tag: Vec<NameCount>,
    pub by_agent: Vec<NameCount>,
    pub by_team: Vec<NameCount>,
    pub daily_new: Vec<DailyPoint>,
    pub by_channel: Vec<NameCount>,
    pub channel_metrics: Vec<ChannelMetric>,
    pub mailbox_comparison: Vec<MailboxComparison>,
}

/// Filters the dashboard is computed over.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardScope {
    pub days: i64,
    pub mailbox_ids: Vec<i64>,
    /// `None` = all channels; `Some("email" | "chat")` otherwise.
    pub channel: Option<String>,
}

impl Default for DashboardScope {
    fn default() -> Self {
        Self {
            days: 30,
            mailbox_ids: Vec::new(),
            channel: None,
        }
    }
}

fn round_min(v: f64) -> i64 {
    v.round() as i64
}

fn name_counts(
    conn: &Connection,
    sql: &str,
    params: Vec<rusqlite::types::Value>,
) -> Vec<NameCount> {
    conn.prepare(sql)
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(params), |r| {
                Ok(NameCount {
                    name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    count: r.get(1)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Load the full dashboard payload for `scope`.
///
/// Read-only; safe to call on every invalidation tick.
#[must_use]
pub fn load_dashboard(conn: &Connection, scope: &DashboardScope) -> DashboardData {
    let from = spp_core::business_hours::iso_days_ago(scope.days);
    let to = spp_core::business_hours::now_iso_millis();

    let mailbox_in = if scope.mailbox_ids.is_empty() {
        String::new()
    } else {
        format!(
            " AND c.mailbox_local_id IN ({})",
            scope
                .mailbox_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let conv_channel = scope
        .channel
        .as_ref()
        .map(|_| " AND c.type = ?3".to_string())
        .unwrap_or_default();
    let rating_channel = scope
        .channel
        .as_ref()
        .map(|_| " AND c.type = ?3".to_string())
        .unwrap_or_default();

    let range_params = |with_channel: bool| -> Vec<rusqlite::types::Value> {
        let mut p = vec![from.clone().into(), to.clone().into()];
        if with_channel && let Some(c) = &scope.channel {
            p.push(c.clone().into());
        }
        p
    };

    let counts_sql = format!(
        "SELECT
          COALESCE(SUM(CASE WHEN c.created_at >= ?1 AND c.created_at <= ?2 THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status = 'active' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status = 'pending' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND c.assignee_local_id IS NULL AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday('now') - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0)
         FROM conversations c WHERE c.deleted_at IS NULL{mailbox_in}{conv_channel}"
    );
    let (
        new_conversations,
        active_conversations,
        pending_conversations,
        closed_conversations,
        unassigned,
        backlog,
    ): (i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            &counts_sql,
            rusqlite::params_from_iter(range_params(scope.channel.is_some())),
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0, 0, 0));

    let replies_sent: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM conversation_threads t
                 JOIN conversations c ON c.id = t.conversation_id
                 WHERE t.type = 'reply' AND t.state = 'published' AND t.deleted_at IS NULL
                   AND t.created_at >= ?1 AND t.created_at <= ?2{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(scope.channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(0);

    let first_reply_join = "LEFT JOIN (SELECT conversation_id, MIN(created_at) AS first_reply FROM conversation_threads WHERE type = 'reply' AND state = 'published' AND deleted_at IS NULL GROUP BY conversation_id) fr ON fr.conversation_id = c.id";
    let first_response_avg_min: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440)
                 FROM conversations c
                 {first_reply_join}
                 WHERE c.created_at >= ?1 AND c.created_at <= ?2 AND c.deleted_at IS NULL{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(scope.channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(None);
    let resolution_avg_min: Option<f64> = conn
        .query_row(
            &format!(
                "SELECT AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440)
                 FROM conversations c
                 WHERE c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 AND c.deleted_at IS NULL{mailbox_in}{conv_channel}"
            ),
            rusqlite::params_from_iter(range_params(scope.channel.is_some())),
            |r| r.get(0),
        )
        .unwrap_or(None);

    let (great, okay, not_good): (i64, i64, i64) = conn
        .query_row(
            &format!(
                "SELECT
                   COALESCE(SUM(CASE WHEN r.rating = 'great' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN r.rating = 'okay' THEN 1 ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN r.rating = 'not-good' THEN 1 ELSE 0 END), 0)
                 FROM ratings r LEFT JOIN conversations c ON c.id = r.conversation_id
                 WHERE r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{mailbox_in}{rating_channel}"
            ),
            rusqlite::params_from_iter(range_params(scope.channel.is_some())),
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let by_mailbox = name_counts(
        conn,
        &format!(
            "SELECT m.name AS name, COUNT(*) AS count FROM conversations c
             JOIN mailboxes m ON m.id = c.mailbox_local_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{conv_channel}
             GROUP BY m.name ORDER BY count DESC"
        ),
        range_params(scope.channel.is_some()),
    );
    let by_tag = name_counts(
        conn,
        &format!(
            "SELECT t.name AS name, COUNT(*) AS count
             FROM conversation_tags ct
             JOIN tags t ON t.id = ct.tag_id
             JOIN conversations c ON c.id = ct.conversation_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY t.name ORDER BY count DESC LIMIT 12"
        ),
        range_params(scope.channel.is_some()),
    );
    let by_agent = name_counts(
        conn,
        &format!(
            "SELECT TRIM(COALESCE(u.first_name, '') || ' ' || COALESCE(u.last_name, '')) AS name, COUNT(*) AS count
             FROM conversations c JOIN users u ON u.id = c.assignee_local_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY u.id ORDER BY count DESC"
        ),
        range_params(scope.channel.is_some()),
    );
    let by_team = name_counts(
        conn,
        &format!(
            "SELECT tm.name AS name, COUNT(*) AS count
             FROM conversations c
             JOIN users u ON u.id = c.assignee_local_id
             JOIN team_members tem ON tem.user_id = u.id
             JOIN teams tm ON tm.id = tem.team_id
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY tm.id ORDER BY count DESC"
        ),
        range_params(scope.channel.is_some()),
    );

    let daily_new: Vec<DailyPoint> = conn
        .prepare(&format!(
            "SELECT date(c.created_at) AS date, COUNT(*) AS value FROM conversations c
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}{conv_channel}
             GROUP BY date(c.created_at) ORDER BY date"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(
                rusqlite::params_from_iter(range_params(scope.channel.is_some())),
                |r| {
                    Ok(DailyPoint {
                        date: r.get(0)?,
                        value: r.get(1)?,
                    })
                },
            )
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let by_channel = name_counts(
        conn,
        &format!(
            "SELECT COALESCE(c.type, 'unknown') AS channel, COUNT(*) AS count FROM conversations c
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}
             GROUP BY COALESCE(c.type, 'unknown') ORDER BY count DESC"
        ),
        range_params(false),
    );

    let channel_metrics: Vec<ChannelMetric> = conn
        .prepare(&format!(
            "SELECT COALESCE(c.type, 'unknown') AS channel, COUNT(*) AS count,
                    AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS first_response_avg_min,
                    AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS resolution_avg_min
             FROM conversations c
             {first_reply_join}
             WHERE c.deleted_at IS NULL AND c.created_at >= ?1 AND c.created_at <= ?2{mailbox_in}
             GROUP BY COALESCE(c.type, 'unknown') ORDER BY count DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(range_params(false)), |r| {
                Ok(ChannelMetric {
                    channel: r.get::<_, Option<String>>(0)?.unwrap_or_else(|| "unknown".into()),
                    count: r.get(1)?,
                    first_response_avg_min: r.get::<_, Option<f64>>(2)?.map(round_min),
                    resolution_avg_min: r.get::<_, Option<f64>>(3)?.map(round_min),
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    let cmp_channel_join = scope
        .channel
        .as_ref()
        .map(|_| " AND c.type = ?3")
        .unwrap_or("");
    let cmp_channel_sub = scope
        .channel
        .as_ref()
        .map(|_| " AND c2.type = ?3")
        .unwrap_or("");
    let mailbox_comparison: Vec<MailboxComparison> = conn
        .prepare(&format!(
            "SELECT m.id AS mailbox_id, m.name AS name,
                    COALESCE(SUM(CASE WHEN c.created_at >= ?1 AND c.created_at <= ?2 THEN 1 ELSE 0 END), 0) AS new_conversations,
                    COALESCE(SUM(CASE WHEN c.status = 'active' AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0) AS active_conversations,
                    COALESCE(SUM(CASE WHEN c.closed_at IS NOT NULL AND c.closed_at >= ?1 AND c.closed_at <= ?2 THEN 1 ELSE 0 END), 0) AS closed_conversations,
                    COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday('now') - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0) AS backlog,
                    AVG((julianday(fr.first_reply) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS first_response_avg_min,
                    AVG((julianday(c.closed_at) - julianday(COALESCE(c.first_customer_message_at, c.created_at))) * 1440) AS resolution_avg_min,
                    (SELECT COUNT(*) FROM ratings r JOIN conversations c2 ON c2.id = r.conversation_id
                      WHERE c2.mailbox_local_id = m.id AND r.rating = 'great' AND r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{cmp_channel_sub}) AS great_ratings,
                    (SELECT COUNT(*) FROM ratings r JOIN conversations c2 ON c2.id = r.conversation_id
                      WHERE c2.mailbox_local_id = m.id AND r.remote_created_at >= ?1 AND r.remote_created_at <= ?2{cmp_channel_sub}) AS total_ratings
             FROM mailboxes m
             LEFT JOIN conversations c ON c.mailbox_local_id = m.id AND c.deleted_at IS NULL{cmp_channel_join}
             {first_reply_join}
             WHERE 1 = 1{mailbox_in}
             GROUP BY m.id, m.name ORDER BY new_conversations DESC"
        ))
        .and_then(|mut stmt| {
            stmt.query_map(rusqlite::params_from_iter(range_params(scope.channel.is_some())), |r| {
                Ok(MailboxComparison {
                    mailbox_id: r.get(0)?,
                    name: r.get(1)?,
                    new_conversations: r.get(2)?,
                    active_conversations: r.get(3)?,
                    closed_conversations: r.get(4)?,
                    backlog: r.get(5)?,
                    first_response_avg_min: r.get::<_, Option<f64>>(6)?.map(round_min),
                    resolution_avg_min: r.get::<_, Option<f64>>(7)?.map(round_min),
                    great_ratings: r.get(8)?,
                    total_ratings: r.get(9)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();

    DashboardData {
        range_from: from,
        range_to: to,
        new_conversations,
        active_conversations,
        pending_conversations,
        closed_conversations,
        unassigned,
        backlog,
        first_response_avg_min: first_response_avg_min.map(round_min),
        resolution_avg_min: resolution_avg_min.map(round_min),
        replies_sent,
        ratings_great: great,
        ratings_okay: okay,
        ratings_not_good: not_good,
        by_mailbox,
        by_tag,
        by_agent,
        by_team,
        daily_new,
        by_channel,
        channel_metrics,
        mailbox_comparison,
    }
}

/// Mailbox chips row: `(id, name)`.
#[must_use]
pub fn mailboxes(conn: &Connection) -> Vec<(i64, String)> {
    conn.prepare("SELECT id, name FROM mailboxes ORDER BY id")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Issue-radar alerts (same data the web page polled every 2 minutes).
#[must_use]
pub fn radar_alerts(conn: &Connection) -> Vec<serde_json::Value> {
    spp_core::analytics::issue_radar_impl(conn)
}

/// Chart-ready `[[x, y]]` pairs: x = day index, y = new conversations.
#[must_use]
pub fn daily_new_points(data: &DashboardData) -> Vec<[f64; 2]> {
    data.daily_new
        .iter()
        .enumerate()
        .map(|(i, p)| [i as f64, p.value as f64])
        .collect()
}

/// The interactive page state.
pub struct DashboardPage {
    scope: DashboardScope,
    mailboxes: Vec<(i64, String)>,
    data: Option<DashboardData>,
    radar: Vec<serde_json::Value>,
    error: Option<String>,
}

impl DashboardPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scope: DashboardScope::default(),
            mailboxes: Vec::new(),
            data: None,
            radar: Vec::new(),
            error: None,
        }
    }

    /// Mark dirty: the next frame reloads everything (scope, data, radar).
    pub fn invalidate(&mut self) {
        self.data = None;
    }

    fn reload(&mut self, db: &DbHandle) {
        let (mailboxes, data, radar) = db.with_read(|conn| {
            (
                mailboxes(conn),
                load_dashboard(conn, &self.scope),
                radar_alerts(conn),
            )
        });
        self.mailboxes = mailboxes;
        self.data = Some(data);
        self.radar = radar;
        self.error = None;
    }

    /// Render the page; reloads lazily when data is missing (first frame or
    /// after an invalidation).
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        if self.data.is_none() {
            self.reload(db);
        }

        design::page_header(
            ui,
            "Dashboard",
            Some("Local metrics · sources labeled Help Scout / local / AI-derived everywhere"),
            |ui| {
                if design::button_with_icon(ui, Icon::Refresh, "Reload", ButtonVariant::Outline)
                    .on_hover_text("re-read the local mirror (auto-refreshes on every sync)")
                    .clicked()
                {
                    self.invalidate();
                }
            },
        );

        // Scope filters.
        design::card(ui, |ui| {
            ui.horizontal(|ui| {
                design::field_label(ui, "Range");
                for (d, label) in RANGES {
                    if design::chip(ui, self.scope.days == d, label).clicked() {
                        self.scope.days = d;
                        self.reload(db);
                    }
                }
            });
            ui.horizontal(|ui| {
                design::field_label(ui, "Channel");
                for (value, label) in CHANNELS {
                    let selected = self.scope.channel.as_deref() == Some(value)
                        || (value.is_empty() && self.scope.channel.is_none());
                    if design::chip(ui, selected, label).clicked() {
                        self.scope.channel = if value.is_empty() {
                            None
                        } else {
                            Some(value.to_string())
                        };
                        self.reload(db);
                    }
                }
            });
            ui.horizontal(|ui| {
                design::field_label(ui, "Mailboxes");
                let all = self.scope.mailbox_ids.is_empty();
                if design::chip(ui, all, "All").clicked() {
                    self.scope.mailbox_ids.clear();
                    self.reload(db);
                }
                for (id, name) in self.mailboxes.clone() {
                    let selected = self.scope.mailbox_ids.contains(&id);
                    if design::chip(ui, selected, name.as_str()).clicked() {
                        if selected {
                            self.scope.mailbox_ids.retain(|m| m != &id);
                        } else {
                            self.scope.mailbox_ids.push(id);
                        }
                        self.reload(db);
                    }
                }
            });
        });
        ui.add_space(10.0);

        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }

        let data = match &self.data {
            Some(d) => d,
            None => {
                ui.spinner();
                return;
            }
        };

        // Metric cards — responsive column count.
        let width = ui.available_width();
        let cols = if width < 640.0 {
            2
        } else if width < 980.0 {
            3
        } else {
            4
        };
        let metrics: [(&str, i64); 12] = [
            ("New", data.new_conversations),
            ("Active", data.active_conversations),
            ("Pending", data.pending_conversations),
            ("Closed", data.closed_conversations),
            ("Unassigned", data.unassigned),
            ("Backlog (7d stale)", data.backlog),
            ("Replies sent", data.replies_sent),
            (
                "First response avg",
                data.first_response_avg_min.unwrap_or(0),
            ),
            ("Great ratings", data.ratings_great),
            ("Okay ratings", data.ratings_okay),
            ("Not-good ratings", data.ratings_not_good),
            ("Resolution avg", data.resolution_avg_min.unwrap_or(0)),
        ];
        egui::Grid::new("dashboard-metrics")
            .num_columns(cols)
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                for row in metrics.chunks(cols) {
                    for (label, value) in row {
                        let unit = if label.contains("avg") { "min" } else { "" };
                        design::metric_card(
                            ui,
                            label,
                            &value.to_string(),
                            if unit.is_empty() { None } else { Some(unit) },
                        );
                    }
                    ui.end_row();
                }
            });
        ui.add_space(10.0);

        // Per-day chart.
        let points = daily_new_points(data);
        let range_hint = format!(
            "{} → {} · {} points",
            iso_date_only(&data.range_from),
            iso_date_only(&data.range_to),
            points.len()
        );
        design::card_titled(ui, "New conversations per day", Some(&range_hint), |ui| {
            if points.is_empty() {
                design::empty_state(
                    ui,
                    Icon::BarChart,
                    "no conversations in range",
                    "Nothing was created in the selected period.",
                );
            } else {
                chart::line_chart(ui, "dashboard-daily-new", &points, "new conversations");
            }
        });
        ui.add_space(10.0);

        // Breakdown panels — two columns, one on narrow windows.
        let breakdown_cols = if width < 760.0 { 1 } else { 2 };
        egui::Grid::new("dashboard-breakdowns")
            .num_columns(breakdown_cols)
            .spacing(egui::vec2(10.0, 10.0))
            .show(ui, |ui| {
                for row in [
                    ("By mailbox", &data.by_mailbox, false),
                    ("By tag", &data.by_tag, false),
                    ("By channel", &data.by_channel, false),
                    ("By agent", &data.by_agent, false),
                    ("By team", &data.by_team, false),
                ]
                .chunks(breakdown_cols)
                {
                    for (title, rows, _) in row {
                        breakdown(ui, title, rows);
                    }
                    ui.end_row();
                }
                // The channel-metrics table pairs with the last breakdown.
                channel_metrics_table(ui, &data.channel_metrics);
                if breakdown_cols == 2 {
                    // Placeholder keeps the grid balanced.
                    ui.label("");
                }
                ui.end_row();
            });
        ui.add_space(10.0);

        comparison_table(ui, &data.mailbox_comparison);
        ui.add_space(10.0);

        radar_panel(ui, &self.radar);
    }
}

impl Default for DashboardPage {
    fn default() -> Self {
        Self::new()
    }
}

/// Trim an ISO timestamp down to its date (`YYYY-MM-DD`).
fn iso_date_only(iso: &str) -> std::borrow::Cow<'_, str> {
    match iso.split_once('T') {
        Some((date, _)) => std::borrow::Cow::Borrowed(date),
        None => std::borrow::Cow::Borrowed(iso),
    }
}

fn breakdown(ui: &mut egui::Ui, title: &str, rows: &[NameCount]) {
    design::card_titled(ui, title, None, |ui| {
        if rows.is_empty() {
            design::hint_text(ui, "(none)");
            return;
        }
        egui::Grid::new(egui::IdSalt::new(format!("breakdown-{title}")))
            .num_columns(2)
            .striped(true)
            .spacing(egui::vec2(12.0, 2.0))
            .min_row_height(20.0)
            .show(ui, |ui| {
                let t = design::tokens(ui);
                for row in rows {
                    ui.label(
                        egui::RichText::new(row.name.as_str())
                            .size(13.0)
                            .color(t.foreground),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.monospace(
                            egui::RichText::new(row.count.to_string()).color(t.foreground),
                        );
                    });
                    ui.end_row();
                }
            });
    });
}

fn channel_metrics_table(ui: &mut egui::Ui, rows: &[ChannelMetric]) {
    design::card_titled(ui, "Channel metrics", None, |ui| {
        if rows.is_empty() {
            design::hint_text(ui, "(none)");
            return;
        }
        design::table(
            ui,
            "channel-metrics",
            &["channel", "count", "first resp (min)", "resolution (min)"],
            26.0,
            |ui| {
                let t = design::tokens(ui);
                for row in rows {
                    ui.label(
                        egui::RichText::new(row.channel.as_str())
                            .size(13.0)
                            .color(t.foreground),
                    );
                    design::td_num(ui, &row.count.to_string());
                    design::td_num(
                        ui,
                        &row.first_response_avg_min
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "—".into()),
                    );
                    design::td_num(
                        ui,
                        &row.resolution_avg_min
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "—".into()),
                    );
                    ui.end_row();
                }
            },
        );
    });
}

fn comparison_table(ui: &mut egui::Ui, rows: &[MailboxComparison]) {
    if rows.is_empty() {
        return;
    }
    design::card_titled(ui, "Mailbox comparison", None, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("mailbox-comparison-scroll")
            .max_height(320.0)
            .show(ui, |ui| {
                design::table(
                    ui,
                    "mailbox-comparison",
                    &[
                        "mailbox",
                        "new",
                        "active",
                        "closed",
                        "backlog",
                        "first resp (min)",
                        "resolution (min)",
                        "great",
                        "ratings",
                    ],
                    26.0,
                    |ui| {
                        let t = design::tokens(ui);
                        for row in rows {
                            ui.label(
                                egui::RichText::new(row.name.as_str())
                                    .size(13.0)
                                    .strong()
                                    .color(t.foreground),
                            );
                            design::td_num(ui, &row.new_conversations.to_string());
                            design::td_num(ui, &row.active_conversations.to_string());
                            design::td_num(ui, &row.closed_conversations.to_string());
                            design::td_num(ui, &row.backlog.to_string());
                            design::td_num(
                                ui,
                                &row.first_response_avg_min
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "—".into()),
                            );
                            design::td_num(
                                ui,
                                &row.resolution_avg_min
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|| "—".into()),
                            );
                            design::td_num(ui, &row.great_ratings.to_string());
                            design::td_num(ui, &row.total_ratings.to_string());
                            ui.end_row();
                        }
                    },
                );
            });
    });
}

fn radar_panel(ui: &mut egui::Ui, alerts: &[serde_json::Value]) {
    design::card_titled(
        ui,
        "Issue radar",
        Some("AI-derived alerts from local conversation data"),
        |ui| {
            if alerts.is_empty() {
                design::empty_state(
                    ui,
                    Icon::Target,
                    "(no alerts)",
                    "No AI-derived alerts for the current data.",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("dashboard-radar-scroll")
                .show(ui, |ui| {
                    for alert in alerts {
                        let title = super::str_field(alert, "title");
                        let severity = super::str_field(alert, "severity");
                        let conversations = alert
                            .get("conversation_ids")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(0);
                        let tone = if severity == "high" {
                            (BadgeTone::Destructive, "high")
                        } else {
                            (BadgeTone::Warning, "medium")
                        };
                        ui.horizontal(|ui| {
                            design::badge(ui, tone.1, tone.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "{title} · {conversations} conversations",
                                ))
                                .size(13.0)
                                .color(design::tokens(ui).foreground),
                            );
                        });
                        ui.add_space(2.0);
                    }
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testsupport::fixture;

    #[test]
    fn counts_breakdowns_and_daily_series() {
        let db = fixture(vec![
            ("conv", 1, "Bug in export", "active", "email", 1, 5, 9),
            ("conv", 2, "Login broken", "closed", "chat", 2, 5, 9),
            ("conv", 3, "Billing question", "pending", "email", 1, 5, 9),
            ("conv", 4, "Old one", "closed", "email", 1, 5, 9),
        ]);
        let data = db.with_read(|conn| {
            crate::testsupport::seed_thread(conn, 1, "customer_message", "help export broken");
            crate::testsupport::seed_thread(conn, 1, "reply", "we are on it");
            crate::testsupport::seed_rating(conn, 1, "great");
            crate::testsupport::seed_rating(conn, 2, "okay");
            crate::testsupport::seed_tag(conn, 1, "bug");
            crate::testsupport::seed_tag(conn, 2, "login");
            load_dashboard(conn, &DashboardScope::default())
        });

        assert_eq!(data.new_conversations, 4);
        assert_eq!(data.active_conversations, 1);
        assert_eq!(data.pending_conversations, 1);
        assert_eq!(data.closed_conversations, 2);
        assert_eq!(data.unassigned, 0); // all assigned to user 5
        assert_eq!(data.replies_sent, 1);
        assert_eq!(data.ratings_great, 1);
        assert_eq!(data.ratings_okay, 1);
        assert_eq!(data.ratings_not_good, 0);

        let mailboxes: Vec<&NameCount> = data.by_mailbox.iter().collect();
        assert_eq!(mailboxes.len(), 2);
        assert_eq!(mailboxes[0].name, "Support"); // id 1 first, more convs
        assert_eq!(mailboxes[0].count, 3);

        assert_eq!(data.by_tag.len(), 2);
        assert_eq!(data.by_channel.len(), 2);
        assert_eq!(data.channel_metrics.len(), 2);
        assert_eq!(data.daily_new.len(), 1); // all seeded today
        assert_eq!(data.daily_new[0].value, 4);
        assert!(data.first_response_avg_min.is_some());
    }

    #[test]
    fn mailbox_scope_and_channel_scope_filter() {
        let db = fixture(vec![
            ("conv", 1, "Email one", "active", "email", 1, 5, 9),
            ("conv", 2, "Chat one", "active", "chat", 2, 5, 9),
        ]);
        let only_mailbox_one = DashboardScope {
            mailbox_ids: vec![1],
            ..DashboardScope::default()
        };
        let chat_only = DashboardScope {
            channel: Some("chat".into()),
            ..DashboardScope::default()
        };
        db.with_read(|conn| {
            let data = load_dashboard(conn, &only_mailbox_one);
            assert_eq!(data.new_conversations, 1);
            // `by_channel` deliberately ignores the channel filter (it shows
            // the distribution — same as the HTTP route), but the mailbox
            // scope applies.
            assert_eq!(data.by_channel.len(), 1);
            assert_eq!(data.by_channel[0].name, "email");

            let data = load_dashboard(conn, &chat_only);
            assert_eq!(data.new_conversations, 1);
            assert_eq!(data.by_channel.len(), 2);
        });
    }

    #[test]
    fn daily_new_points_maps_index_to_value() {
        let data = DashboardData {
            daily_new: vec![
                DailyPoint {
                    date: "2026-01-01".into(),
                    value: 2,
                },
                DailyPoint {
                    date: "2026-01-02".into(),
                    value: 5,
                },
            ],
            ..DashboardData::default()
        };
        assert_eq!(daily_new_points(&data), vec![[0.0, 2.0], [1.0, 5.0]]);
    }

    #[test]
    fn radar_alerts_run_against_seeded_db() {
        let db = fixture(Vec::new());
        let alerts = db.with_read(radar_alerts);
        // No ai_runs rows → no alerts, but the query must not error.
        assert!(alerts.is_empty());
    }

    #[test]
    fn mailboxes_lists_ordered() {
        let db = fixture(Vec::new());
        let list = db.with_read(mailboxes);
        assert_eq!(list, vec![(1, "Support".into()), (2, "Billing".into())]);
    }
}

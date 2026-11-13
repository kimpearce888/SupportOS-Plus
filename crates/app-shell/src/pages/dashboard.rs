use eframe::egui;
use rusqlite::Connection;
use spp_core::db::DbHandle;

use crate::chart;
use crate::design::{self, BadgeTone};
use crate::icons::Icon;

pub(crate) const RANGES: [(i64, &str); 4] = [
    (7, "7 days"),
    (30, "30 days"),
    (90, "90 days"),
    (365, "1 year"),
];

const CHANNELS: [(&str, &str); 3] = [
    ("", "All channels"),
    ("email", "Email"),
    ("chat", "Chat (Beacon)"),
];

const CHANNEL_NOTE: &str =
    "every channel is listed — the Channel filter scopes the other cards, not this one";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyPoint {
    pub date: String,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChannelMetric {
    pub channel: String,
    pub count: i64,
    pub first_response_avg_min: Option<i64>,
    pub resolution_avg_min: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MailboxComparison {
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

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DashboardData {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DashboardScope {
    pub days: i64,
    pub mailbox_ids: Vec<i64>,
    pub channel: Option<String>,
    pub today: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for DashboardScope {
    fn default() -> Self {
        Self {
            days: 30,
            mailbox_ids: Vec::new(),
            channel: None,
            today: None,
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

#[must_use]
pub(crate) fn load_dashboard(conn: &Connection, scope: &DashboardScope) -> DashboardData {
    let now = scope.today.unwrap_or_else(chrono::Utc::now);
    let from = spp_core::business_hours::iso_days_ago_at(scope.days, now);
    let to = spp_core::business_hours::now_iso_millis_at(now);

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
          COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday(?2) - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0)
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
                    COALESCE(SUM(CASE WHEN c.status IN ('active','pending') AND (julianday(?2) - COALESCE(julianday(c.updated_at), julianday(c.created_at))) >= 7 AND c.deleted_at IS NULL THEN 1 ELSE 0 END), 0) AS backlog,
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

#[must_use]
pub fn mailboxes(conn: &Connection) -> Vec<(i64, String)> {
    conn.prepare("SELECT id, name FROM mailboxes ORDER BY id")
        .and_then(|mut stmt| {
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn radar_alerts(conn: &Connection) -> Vec<serde_json::Value> {
    spp_core::analytics::issue_radar_impl(conn)
}

#[must_use]
pub(crate) fn daily_new_points(data: &DashboardData) -> Vec<[f64; 2]> {
    data.daily_new
        .iter()
        .enumerate()
        .map(|(i, p)| [i as f64, p.value as f64])
        .collect()
}

pub struct DashboardPage {
    pub sync_note: String,

    scope: DashboardScope,
    mailboxes: Vec<(i64, String)>,
    data: Option<DashboardData>,
    radar: Vec<serde_json::Value>,
    error: Option<String>,
}

impl DashboardPage {
    pub(crate) fn sync_finished(&mut self, outcome: &str) {
        if !self.sync_note.is_empty() {
            self.sync_note = outcome.to_string();
        }
    }

    #[must_use]
    pub fn new() -> Self {
        Self {
            sync_note: String::new(),
            scope: DashboardScope::default(),
            mailboxes: Vec::new(),
            data: None,
            radar: Vec::new(),
            error: None,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.data = None;
    }

    pub fn pin_today(&mut self, today: chrono::DateTime<chrono::Utc>) {
        self.scope.today = Some(today);
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

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        db: &DbHandle,
        engine: Option<&std::sync::Arc<spp_core::sync_engine::SyncEngine>>,
    ) {
        if self.data.is_none() {
            self.reload(db);
        }

        design::page_header(
            ui,
            "Dashboard",
            Some("Local metrics · sources labeled Help Scout / local / AI-derived everywhere"),
            |ui| {
                if !self.sync_note.is_empty() {
                    design::hint_text(ui, &self.sync_note);
                }
                if crate::pages::sync_health::sync_now_button(ui, db, engine, &mut self.sync_note) {
                    self.invalidate();
                }
            },
        );

        design::card(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                design::field_label(ui, "Range");
                for (d, label) in RANGES {
                    if design::chip(ui, self.scope.days == d, label).clicked() {
                        self.scope.days = d;
                        self.reload(db);
                    }
                }
                ui.add_space(14.0);
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
                ui.add_space(14.0);
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

        let width = ui.available_width();
        let gap = 10.0;

        let kpi_cols = if width >= 1400.0 {
            6
        } else if width >= 900.0 {
            4
        } else if width >= 620.0 {
            3
        } else {
            2
        };
        let card_w = ((width - gap * (kpi_cols - 1) as f32) / kpi_cols as f32).floor();
        let metrics: [(&str, i64); 12] = [
            ("New", data.new_conversations),
            ("Active", data.active_conversations),
            ("Pending", data.pending_conversations),
            ("Closed", data.closed_conversations),
            ("Unassigned", data.unassigned),
            ("Backlog (7d stale)", data.backlog),
            ("Replies sent", data.replies_sent),
            (
                "First response (min)",
                data.first_response_avg_min.unwrap_or(0),
            ),
            ("Resolution avg (min)", data.resolution_avg_min.unwrap_or(0)),
            ("Great ratings", data.ratings_great),
            ("Okay ratings", data.ratings_okay),
            ("Not-good ratings", data.ratings_not_good),
        ];
        egui::Grid::new("dashboard-metrics")
            .num_columns(kpi_cols)
            .spacing(egui::vec2(gap, gap))
            .show(ui, |ui| {
                for row in metrics.chunks(kpi_cols) {
                    for (label, value) in row {
                        design::metric_card_w(ui, label, &value.to_string(), None, card_w);
                    }
                    ui.end_row();
                }
            });
        ui.add_space(gap);

        let points = daily_new_points(data);
        let range_hint = format!(
            "{} → {} · {} points",
            iso_date_only(&data.range_from),
            iso_date_only(&data.range_to),
            points.len()
        );
        let range_from = iso_date_only(&data.range_from);
        let range_to = iso_date_only(&data.range_to);
        if width >= 1000.0 {
            let chart_w = ((width - gap) * 0.62).floor();
            let radar_w = (width - chart_w - gap).floor();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                let chart = design::card_titled_exact(
                    ui,
                    "New conversations per day",
                    Some(&range_hint),
                    chart_w,
                    None,
                    |ui| {
                        chart_body(ui, &points, range_from.as_ref(), range_to.as_ref());
                    },
                );
                let chart_h = chart.response.rect.height();
                radar_panel(ui, &self.radar, radar_w, Some(chart_h));
            });
        } else {
            design::card_titled_exact(
                ui,
                "New conversations per day",
                Some(&range_hint),
                width,
                None,
                |ui| {
                    chart_body(ui, &points, range_from.as_ref(), range_to.as_ref());
                },
            );
            ui.add_space(gap);
            radar_panel(ui, &self.radar, width, None);
        }
        ui.add_space(gap);

        let breakdown_cols = if width >= 900.0 {
            3
        } else if width >= 560.0 {
            2
        } else {
            1
        };
        let breakdown_w =
            ((width - gap * (breakdown_cols - 1) as f32) / breakdown_cols as f32).floor();
        let breakdown_cards: [(&str, &[NameCount], &str); 5] = [
            ("By mailbox", &data.by_mailbox, ""),
            ("By tag", &data.by_tag, ""),
            ("By channel", &data.by_channel, CHANNEL_NOTE),
            ("By agent", &data.by_agent, ""),
            ("By team", &data.by_team, ""),
        ];
        for (i, row) in breakdown_cards.chunks(breakdown_cols).enumerate() {
            if i > 0 {
                ui.add_space(gap);
            }
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (title, rows, note) in row {
                    breakdown(ui, title, note, rows, breakdown_w);
                }
            });
        }
        ui.add_space(gap);

        channel_metrics_table(ui, &data.channel_metrics);
        ui.add_space(gap);

        comparison_table(ui, &data.mailbox_comparison);
    }
}

impl Default for DashboardPage {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn iso_date_only(iso: &str) -> std::borrow::Cow<'_, str> {
    match iso.split_once('T') {
        Some((date, _)) => std::borrow::Cow::Borrowed(date),
        None => std::borrow::Cow::Borrowed(iso),
    }
}

fn chart_body(ui: &mut egui::Ui, points: &[chart::Point], range_from: &str, range_to: &str) {
    if points.is_empty() {
        design::empty_state(
            ui,
            Icon::BarChart,
            "no conversations in range",
            "Nothing was created in the selected period.",
        );
    } else {
        chart::line_chart_axes(
            ui,
            points,
            "new conversations",
            Some((range_from, range_to)),
        );
    }
}

fn breakdown(ui: &mut egui::Ui, title: &str, note: &str, rows: &[NameCount], outer_w: f32) {
    design::card_titled_exact(
        ui,
        title,
        if note.is_empty() { None } else { Some(note) },
        outer_w,
        None,
        |ui| {
            if rows.is_empty() {
                design::hint_text(ui, "Nothing to show in this range.");
                return;
            }
            let max = rows.iter().map(|r| r.count).max().unwrap_or(1).max(1);
            const VISIBLE: usize = 8;
            for row in rows.iter().take(VISIBLE) {
                design::bar_row(ui, &row.name, row.count, max, true);
            }
            if rows.len() > VISIBLE {
                design::hint_text(ui, &format!("and {} more…", rows.len() - VISIBLE));
            }
        },
    );
}

fn channel_metrics_table(ui: &mut egui::Ui, rows: &[ChannelMetric]) {
    design::card_titled(ui, "Channel metrics", None, |ui| {
        if rows.is_empty() {
            design::hint_text(ui, "(none)");
            return;
        }
        design::data_table(
            ui,
            "channel-metrics",
            &[
                "Channel",
                "Count",
                "First Response (min)",
                "Resolution (min)",
            ],
            rows.len(),
            26.0,
            |ui, i| {
                let row = &rows[i];
                ui.label(
                    egui::RichText::new(row.channel.as_str())
                        .size(13.0)
                        .strong()
                        .color(design::tokens(ui).foreground),
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
            },
        );
    });
}

fn comparison_table(ui: &mut egui::Ui, rows: &[MailboxComparison]) {
    if rows.is_empty() {
        return;
    }
    design::card_titled(ui, "Mailbox comparison", None, |ui| {
        design::data_table(
            ui,
            "mailbox-comparison",
            &[
                "Mailbox",
                "New",
                "Active",
                "Closed",
                "Backlog",
                "First resp (min)",
                "Resolution (min)",
                "Great",
                "Ratings",
            ],
            rows.len(),
            26.0,
            |ui, i| {
                let row = &rows[i];
                ui.label(
                    egui::RichText::new(row.name.as_str())
                        .size(13.0)
                        .strong()
                        .color(design::tokens(ui).foreground),
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
            },
        );
    });
}

fn radar_panel(ui: &mut egui::Ui, alerts: &[serde_json::Value], outer_w: f32, min_h: Option<f32>) {
    design::card_titled_exact(
        ui,
        "Issue radar",
        Some("AI-derived alerts from local conversation data"),
        outer_w,
        min_h,
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
            for (i, alert) in alerts.iter().enumerate() {
                if i > 0 {
                    design::divider(ui);
                }
                let title = super::str_field(alert, "title");
                let severity = super::str_field(alert, "severity");
                let conversations = alert
                    .get("conversation_ids")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                ui.horizontal(|ui| {
                    design::badge(ui, &severity, BadgeTone::Ai);
                    ui.label(
                        egui::RichText::new(format!("{title} · {conversations} conversations",))
                            .size(13.0)
                            .color(design::tokens(ui).foreground),
                    );
                });
            }
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
        assert_eq!(data.unassigned, 0);
        assert_eq!(data.replies_sent, 1);
        assert_eq!(data.ratings_great, 1);
        assert_eq!(data.ratings_okay, 1);
        assert_eq!(data.ratings_not_good, 0);

        let mailboxes: Vec<&NameCount> = data.by_mailbox.iter().collect();
        assert_eq!(mailboxes.len(), 2);
        assert_eq!(mailboxes[0].name, "Support");
        assert_eq!(mailboxes[0].count, 3);

        assert_eq!(data.by_tag.len(), 2);
        assert_eq!(data.by_channel.len(), 2);
        assert_eq!(data.channel_metrics.len(), 2);
        assert_eq!(data.daily_new.len(), 1);
        assert_eq!(data.daily_new[0].value, 4);
        assert!(data.first_response_avg_min.is_some());
    }

    #[test]
    fn backlog_staleness_derives_from_the_pinned_anchor() {
        let db = fixture(Vec::new());
        let anchor = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 1, 15, 12, 0, 0)
            .single()
            .expect("anchor");
        db.with_write(|conn| {
            for (id, status, updated_days_ago) in
                [(1i64, "active", 8i64), (2, "pending", 3)]
            {
                let ts = spp_core::business_hours::iso_days_ago_at(updated_days_ago, anchor);
                conn.execute(
                    "INSERT INTO conversations (id, remote_id, number, subject, preview, status,
                         type, mailbox_local_id, assignee_local_id, customer_local_id, priority,
                         created_at, updated_at, closed_at, first_customer_message_at,
                         local_created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'email', 1, 5, 9, 'none', ?7, ?7, NULL, ?7, ?7)",
                    rusqlite::params![
                        id,
                        1000 + id,
                        500 + id,
                        format!("conv {id}"),
                        format!("conv {id} preview"),
                        status,
                        ts
                    ],
                )
                .expect("seed anchored conversation");
            }
        });

        let scope = DashboardScope {
            today: Some(anchor),
            ..DashboardScope::default()
        };
        let pinned = db.with_read(|conn| load_dashboard(conn, &scope));
        assert_eq!(
            pinned.backlog, 1,
            "the 8d-stale conversation counts at the anchor"
        );

        let wall = db.with_read(|conn| load_dashboard(conn, &DashboardScope::default()));
        assert_eq!(
            wall.backlog, 2,
            "both anchored-past rows are stale at the wall clock"
        );
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
        assert!(alerts.is_empty());
    }

    #[test]
    fn mailboxes_lists_ordered() {
        let db = fixture(Vec::new());
        let list = db.with_read(mailboxes);
        assert_eq!(list, vec![(1, "Support".into()), (2, "Billing".into())]);
    }
}

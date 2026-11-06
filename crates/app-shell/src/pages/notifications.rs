use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::notifications::{ListOptions, NotificationRecord};

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    All,
    Mentions,
    Unread,
}

impl Tab {
    const ALL: [Self; 3] = [Self::All, Self::Mentions, Self::Unread];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Mentions => "Mentions",
            Self::Unread => "Unread",
        }
    }
}

pub struct NotificationsPage {
    tab: Tab,
    records: Vec<NotificationRecord>,
    total: i64,
    unread: i64,
    mentions: Vec<NotificationRecord>,
    error: Option<String>,
    loaded: bool,
}

impl NotificationsPage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::All,
            records: Vec::new(),
            total: 0,
            unread: 0,
            mentions: Vec::new(),
            error: None,
            loaded: false,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.loaded = false;
        self.records.clear();
        self.mentions.clear();
    }

    fn reload(&mut self, db: &DbHandle) {
        let me = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten();
        let (records_result, mentions_result) = db.with_read(|conn| {
            (
                spp_core::notifications::list(
                    conn,
                    &ListOptions {
                        me_user_local_id: me,
                        unread_only: false,
                        notif_type: None,
                        limit: 100,
                        offset: 0,
                    },
                ),
                spp_core::notifications::mentions_for_me(conn, me.unwrap_or(0), 50),
            )
        });
        match records_result {
            Ok(result) => {
                self.records = result.notifications;
                self.total = result.total;
                self.unread = result.unread;
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.mentions = mentions_result.unwrap_or_default();
        self.loaded = true;
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if !self.loaded {
            self.reload(db);
        }
    }

    fn toggle_read(&mut self, db: &DbHandle, id: i64) {
        let me = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten();
        let read = self
            .records
            .iter()
            .chain(self.mentions.iter())
            .find(|r| r.id == id)
            .map(|r| r.read_at.is_none())
            .unwrap_or(true);
        let result = db.with_write(|conn| spp_core::notifications::mark_read(conn, id, me, read));
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
        self.reload(db);
    }

    fn mark_all_read(&mut self, db: &DbHandle) {
        let me = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten();
        let result = db.with_write(|conn| spp_core::notifications::mark_all_read(conn, me));
        match result {
            Ok(_) => self.reload(db),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        let subtitle = format!("{} total · {} unread", self.total, self.unread);
        design::page_header(ui, "Notifications", Some(subtitle.as_str()), |ui| {
            if design::button(ui, "mark all read", ButtonVariant::Text).clicked() {
                self.mark_all_read(db);
            }
        });
        if let Some(i) = design::tabs(
            ui,
            &[Tab::All.label(), Tab::Mentions.label(), Tab::Unread.label()],
            Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0),
        ) && let Some(tab) = Tab::ALL.get(i)
        {
            self.tab = *tab;
        }
        if let Some(err) = &self.error {
            design::error_text(ui, &format!("error: {err}"));
        }
        design::divider(ui);

        let rows: Vec<NotificationRecord> = match self.tab {
            Tab::All => self.records.clone(),
            Tab::Unread => self
                .records
                .iter()
                .filter(|r| r.read_at.is_none())
                .cloned()
                .collect(),
            Tab::Mentions => self.mentions.clone(),
        };
        if rows.is_empty() {
            design::empty_state(
                ui,
                Icon::Bell,
                "No notifications",
                "Nothing here for this tab yet — new alerts arrive with the sync.",
            );
            return;
        }

        let mut toggled: Option<i64> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for record in &rows {
                design::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let unread = record.read_at.is_none();
                        if unread {
                            design::status_dot(ui, BadgeTone::Info, false);
                        }
                        let tone = if record.severity == "error" {
                            BadgeTone::Destructive
                        } else {
                            BadgeTone::Info
                        };
                        design::badge(ui, record.notification_type.as_str(), tone);
                        ui.label(
                            egui::RichText::new(relative_time(&record.created_at))
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                        )
                        .on_hover_text(record.created_at.as_str());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if design::button(
                                ui,
                                if unread { "mark read" } else { "unread" },
                                ButtonVariant::Text,
                            )
                            .clicked()
                            {
                                toggled = Some(record.id);
                            }
                        });
                    });
                    ui.label(
                        egui::RichText::new(record.title.as_str())
                            .size(13.5)
                            .strong()
                            .color(design::tokens(ui).foreground),
                    );
                    if let Some(body) = &record.body {
                        design::hint_text(ui, body.as_str());
                    }
                });
            }
        });
        if let Some(id) = toggled {
            self.toggle_read(db, id);
        }
    }
}

impl Default for NotificationsPage {
    fn default() -> Self {
        Self::new()
    }
}

fn relative_time(created_at: &str) -> String {
    let now = spp_core::business_hours::now_iso_millis();
    let Some(minutes) = spp_core::business_hours::wall_minutes_between(created_at, &now) else {
        return created_at.to_string();
    };
    match calendar_day_diff(created_at, &now) {
        Some(0) => format_intraday_minutes(minutes),
        Some(-1) => "Yesterday".to_string(),
        Some(days) if days < -1 => format!("{}d ago", -days),
        _ => created_at.to_string(),
    }
}

fn format_intraday_minutes(minutes: f64) -> String {
    if minutes < 1.0 {
        "just now".to_string()
    } else if minutes < 60.0 {
        format!("{}m ago", minutes as i64)
    } else {
        format!("{}h ago", (minutes / 60.0) as i64)
    }
}

fn calendar_day_diff(a_iso: &str, b_iso: &str) -> Option<i64> {
    let (ay, am, ad) = date_parts(a_iso)?;
    let (by, bm, bd) = date_parts(b_iso)?;
    Some(days_from_civil(ay, am, ad) - days_from_civil(by, bm, bd))
}

fn date_parts(iso: &str) -> Option<(i64, i64, i64)> {
    let mut it = iso.get(..10)?.split('-');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe
}

#[cfg(test)]
mod tests {
    use super::super::i64_field;
    use super::*;
    use crate::testsupport::fixture;

    fn seed_notification(
        conn: &rusqlite::Connection,
        id: i64,
        ntype: &str,
        title: &str,
        read: bool,
    ) {
        conn.execute(
            "INSERT INTO notifications (id, type, severity, title, target_user_id, dedup_key, created_at, read_at)
             VALUES (?1, ?2, 'info', ?3, 5, ?4, '2026-01-01T00:00:00Z',
                     CASE WHEN ?5 = 1 THEN '2026-01-01T00:01:00Z' ELSE NULL END)",
            rusqlite::params![id, ntype, title, format!("seed-{id}"), read],
        )
        .expect("notification");
    }

    fn set_me(conn: &rusqlite::Connection) {
        spp_core::settings::set_string(conn, "me_remote_id", "505").expect("me");
    }

    #[test]
    fn list_shows_all_and_unread_counts() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "mentioned", "Ada mentioned you", false);
            seed_notification(conn, 2, "campaign_reply", "Campaign finished", true);
            seed_notification(conn, 3, "sync_failure", "Sync completed", false);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        assert_eq!(page.total, 3);
        assert_eq!(page.unread, 2);
        assert_eq!(page.records.len(), 3);
    }

    #[test]
    fn mark_read_toggles_and_reloads() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "mentioned", "Ada mentioned you", false);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        assert_eq!(page.unread, 1);

        page.toggle_read(&db, 1);
        assert_eq!(page.unread, 0, "toggle flips to read");

        page.toggle_read(&db, 1);
        assert_eq!(page.unread, 1, "toggle flips back to unread");
    }

    #[test]
    fn mark_all_read_clears_the_unread_counter() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "mentioned", "first", false);
            seed_notification(conn, 2, "sync_failure", "second", false);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        assert_eq!(page.unread, 2);
        page.mark_all_read(&db);
        assert_eq!(page.unread, 0);
        assert_eq!(page.total, 2);
    }

    #[test]
    fn mention_tab_separates_mentions_from_the_rest() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "mentioned", "you were mentioned", false);
            seed_notification(conn, 2, "sync_failure", "sync done", false);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        assert!(
            page.mentions
                .iter()
                .all(|r| r.notification_type.as_str().contains("mention"))
        );
        assert!(page.mentions.iter().any(|r| r.title.contains("mentioned")));
        assert!(page.records.iter().any(|r| r.title.contains("sync done")));
    }

    #[test]
    fn mention_toggle_reads_the_mentions_list_when_records_miss_the_row() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            for i in 0..100 {
                seed_notification(conn, 1000 + i, "sync_failure", "filler", false);
            }
            seed_notification(conn, 1, "mentioned", "old read mention", true);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        assert!(
            page.records.iter().all(|r| r.id != 1),
            "the mention sits outside the All list window"
        );
        assert!(
            page.mentions.iter().any(|r| r.id == 1),
            "the Mentions tab still renders the row"
        );

        page.toggle_read(&db, 1);
        let read_at: Option<String> = db
            .with_read(|conn| {
                conn.query_row("SELECT read_at FROM notifications WHERE id = 1", [], |r| {
                    r.get::<_, Option<String>>(0)
                })
            })
            .expect("the mention row exists");
        assert!(
            read_at.is_none(),
            "the read mention must flip back to unread"
        );
    }

    #[test]
    fn i64_field_helper_still_reads_plain_values() {
        let v = serde_json::json!({"total": 7});
        assert_eq!(i64_field(&v, "total"), 7);
    }

    #[test]
    fn relative_minutes_match_the_lab_relmap() {
        assert_eq!(format_intraday_minutes(0.3), "just now");
        assert_eq!(format_intraday_minutes(5.0), "5m ago");
        assert_eq!(format_intraday_minutes(120.0), "2h ago");
        assert_eq!(format_intraday_minutes(1439.0), "23h ago");
        assert_eq!(
            relative_time(&spp_core::business_hours::iso_days_ago(0)),
            "just now"
        );
        assert_eq!(
            relative_time(&spp_core::business_hours::iso_days_ago(1)),
            "Yesterday"
        );
        assert_eq!(
            relative_time(&spp_core::business_hours::iso_days_ago(2)),
            "2d ago"
        );
        assert_eq!(
            relative_time(&spp_core::business_hours::iso_days_ago(9)),
            "9d ago"
        );
    }

    #[test]
    fn relative_time_falls_back_to_the_raw_stamp_when_unparseable() {
        assert_eq!(relative_time("not a timestamp"), "not a timestamp");
        let past = spp_core::business_hours::iso_days_ago(2);
        assert_eq!(relative_time(&past), "2d ago");
    }
}

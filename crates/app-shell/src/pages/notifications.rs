//! Notifications page (port of the web `pages/notifications.rs`).
//!
//! Presentational pass (shadcn redesign): this page renders through the
//! shared design system in `crate::design`; the queries, state, and
//! behavior are unchanged.
//!
//! All-notifications + mentions tabs (web: the two list endpoints
//! `/api/notifications` and `/api/notifications/mentions`), per-row
//! mark-read toggles, mark-all-read, and the unread badge in the header —
//! all in-process through `spp_core::notifications`.

use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::notifications::{ListOptions, NotificationRecord};

use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;

/// Notification list tabs (web: `active_tab`).
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

/// The interactive notifications page state.
pub struct NotificationsPage {
    tab: Tab,
    records: Vec<NotificationRecord>,
    total: i64,
    unread: i64,
    mentions: Vec<NotificationRecord>,
    error: Option<String>,
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
        }
    }

    /// Live-update invalidation (web: the notification unread version key).
    pub fn invalidate(&mut self) {
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
    }

    fn ensure_loaded(&mut self, db: &DbHandle) {
        if self.records.is_empty() {
            self.reload(db);
        }
    }

    fn toggle_read(&mut self, db: &DbHandle, id: i64) {
        let me = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten();
        // Clean-audit item 130: the Mentions tab renders `self.mentions`,
        // not `self.records` — a mention row can sit outside the All
        // list's 100-row window, and the records-only lookup fell through
        // to `unwrap_or(true)`, re-marking a READ mention as read (the
        // "unread" button silently did nothing). Look the row up in both
        // lists — every rendered row lives in one of them.
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

    /// Render the page.
    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) {
        self.ensure_loaded(db);

        let subtitle = format!("{} total · {} unread", self.total, self.unread);
        design::page_header(ui, "Notifications", Some(subtitle.as_str()), |ui| {
            if design::button(ui, "mark all read", ButtonVariant::Ghost).clicked() {
                self.mark_all_read(db);
            }
        });
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if design::chip(ui, self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
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
                "(no notifications)",
                "Nothing here for this tab yet.",
            );
            return;
        }

        let mut toggled: Option<i64> = None;
        egui::ScrollArea::vertical().show_rows(ui, 44.0, rows.len(), |ui, range| {
            for record in &rows[range] {
                design::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let unread = record.read_at.is_none();
                        let dot = if unread { "●" } else { " " };
                        let tone = if record.severity == "error" {
                            BadgeTone::Destructive
                        } else {
                            BadgeTone::Info
                        };
                        design::badge(
                            ui,
                            &format!("{dot} {}", record.notification_type.as_str()),
                            tone,
                        );
                        ui.monospace(
                            egui::RichText::new(record.created_at.as_str())
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                        );
                        if design::button(
                            ui,
                            if unread { "mark read" } else { "unread" },
                            ButtonVariant::Ghost,
                        )
                        .clicked()
                        {
                            toggled = Some(record.id);
                        }
                    });
                    ui.label(
                        egui::RichText::new(record.title.as_str())
                            .size(13.0)
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

    /// Point `me` at Ada (user 5, remote 505).
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

    /// Clean-audit item 130: a READ mention pushed out of the All list's
    /// 100-row window (the fillers outrank it on `id DESC`) is still
    /// rendered by the Mentions tab — toggling it must read the row's
    /// real state from the mentions list and flip it to unread, not fall
    /// through to the records-only miss and re-mark it read.
    #[test]
    fn mention_toggle_reads_the_mentions_list_when_records_miss_the_row() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            for i in 0..100 {
                seed_notification(conn, 1000 + i, "sync_failure", "filler", false);
            }
            // Same created_at as the fillers, lowest id: outside the
            // All list's LIMIT 100 window, inside the mentions list.
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

        page.toggle_read(&db, 1); // the row's "unread" button
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
}

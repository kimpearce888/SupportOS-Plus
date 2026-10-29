//! Notifications page (port of the web `pages/notifications.rs`).
//!
//! RS-20 reskin: rebuilt onto the DC widget set (`crate::design`) per the
//! conformed lab's `notifications` sheet (DS "stack" archetype): the tab
//! row is the DS-C-09 underline rail (same three labels, same state
//! machine); the mark-all-read act rides the header as the lab's text
//! button; each card carries the lab's toolbar anatomy — the real unread
//! state dot, the raw type badge, the relative time with the raw UTC
//! surviving in the hover title — with the read/unread act pushed right;
//! the lab's quirk receipt rides beneath the list. The queries, state,
//! and behavior are unchanged.
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
    /// Clean-audit item 147: reload latch — an empty mirror is a legal
    /// steady state, so "list empty" can no longer mean "re-query every
    /// frame".
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

    /// Live-update invalidation (web: the notification unread version key).
    pub fn invalidate(&mut self) {
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
            // The lab's header act: the text button (DS-C-11 text).
            if design::button(ui, "mark all read", ButtonVariant::Text).clicked() {
                self.mark_all_read(db);
            }
        });
        // DS-C-09: the chip row is the underline tab rail — same three
        // labels, same state machine.
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
                // The lab's card anatomy: toolbar (state dot + raw type
                // badge + relative time, the act pushed right) over the
                // strong title and the hint body.
                design::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        let unread = record.read_at.is_none();
                        // The lab's ds-udot: a real state dot — the
                        // pre-rebuild build painted '● ' INSIDE the badge
                        // text (the quirk receipt below).
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
        // The lab's quirk receipt for the unread-dot presentation fix.
        design::quirk(
            ui,
            "Reskin fix (presentation): unread was a '● ' character inside \
             the badge text (notifications.rs:197-208) — now a real state \
             dot; raw UTC survives in the hover title.",
        );
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

/// The lab's relative timestamps ("2h ago" / "Yesterday" / "2d ago") —
/// presentation only; the raw UTC survives in the hover title. The lab's
/// relmap reads CALENDAR days (the previous calendar day is "Yesterday",
/// however many hours ago), so the day count compares the date prefixes;
/// unparseable or future stamps fall back to the raw string (honest — no
/// invented time).
fn relative_time(created_at: &str) -> String {
    let now = spp_core::business_hours::now_iso_millis();
    let Some(minutes) = spp_core::business_hours::wall_minutes_between(created_at, &now) else {
        return created_at.to_string();
    };
    match calendar_day_diff(created_at, &now) {
        Some(0) => format_intraday_minutes(minutes),
        // created_at is one calendar day BEFORE now (the diff is a - b).
        Some(-1) => "Yesterday".to_string(),
        Some(days) if days < -1 => format!("{}d ago", -days),
        _ => created_at.to_string(),
    }
}

/// The lab's intraday relmap semantics: <1m "just now", <1h "{n}m ago",
/// else "{n}h ago".
fn format_intraday_minutes(minutes: f64) -> String {
    if minutes < 1.0 {
        "just now".to_string()
    } else if minutes < 60.0 {
        format!("{}m ago", minutes as i64)
    } else {
        format!("{}h ago", (minutes / 60.0) as i64)
    }
}

/// Calendar-day difference `a - b` from the ISO date prefixes — no
/// timezone library, just the civil-date day count (Howard Hinnant's
/// days-from-civil, public domain formulation).
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

    /// The lab's relmap semantics, deterministic: the intraday branches on
    /// the formatter, the calendar-day branches through the real glue
    /// (whole-day offsets from the run moment — the same seeds the
    /// goldens use).
    #[test]
    fn relative_minutes_match_the_lab_relmap() {
        assert_eq!(format_intraday_minutes(0.3), "just now");
        assert_eq!(format_intraday_minutes(5.0), "5m ago");
        assert_eq!(format_intraday_minutes(120.0), "2h ago");
        assert_eq!(format_intraday_minutes(1439.0), "23h ago");
        // Calendar-day semantics: the previous calendar day is
        // "Yesterday" however many hours ago (the lab's 18:40 row).
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

    /// Unparseable stamps fall back to the raw UTC (honest — no invented
    /// time); parseable ones ride the formatter.
    #[test]
    fn relative_time_falls_back_to_the_raw_stamp_when_unparseable() {
        assert_eq!(relative_time("not a timestamp"), "not a timestamp");
        let past = spp_core::business_hours::iso_days_ago(2);
        assert_eq!(relative_time(&past), "2d ago");
    }
}

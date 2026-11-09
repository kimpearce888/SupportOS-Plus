use eframe::egui;
use spp_core::db::DbHandle;
use spp_core::notifications::{ListOptions, NotificationRecord};

use super::relative_time;
use crate::design::{self, BadgeTone, ButtonVariant};
use crate::icons::Icon;
use crate::pages::{NavRequest, PageKey};

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

/// UX-13: one visual card per distinct (type, title, body) notification —
/// identical repeats (e.g. the same job failing 18 times) collapse into a
/// single row with a ×N count instead of 18 byte-identical cards.
#[derive(Debug, Clone)]
struct NotificationGroup {
    records: Vec<NotificationRecord>,
}

impl NotificationGroup {
    fn key(record: &NotificationRecord) -> (String, String, String) {
        (
            record.notification_type.as_str().to_string(),
            record.title.as_str().to_string(),
            record.body.clone().unwrap_or_default(),
        )
    }

    fn push(&mut self, record: NotificationRecord) {
        self.records.push(record);
    }

    fn count(&self) -> usize {
        self.records.len()
    }

    fn any_unread(&self) -> bool {
        self.records.iter().any(|r| r.read_at.is_none())
    }

    fn ids(&self) -> Vec<i64> {
        self.records.iter().map(|r| r.id).collect()
    }

    fn latest(&self) -> &NotificationRecord {
        self.records
            .iter()
            .max_by(|a, b| a.created_at.cmp(&b.created_at))
            .unwrap_or(&self.records[0])
    }

    fn sample(&self) -> &NotificationRecord {
        &self.records[0]
    }
}

fn group_records(records: &[NotificationRecord]) -> Vec<NotificationGroup> {
    let mut groups: Vec<((String, String, String), NotificationGroup)> = Vec::new();
    for record in records {
        let key = NotificationGroup::key(record);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, group)) => group.push(record.clone()),
            None => {
                let mut group = NotificationGroup {
                    records: Vec::new(),
                };
                group.push(record.clone());
                groups.push((key, group));
            }
        }
    }
    groups.into_iter().map(|(_, g)| g).collect()
}

/// UX-13: friendly label for the raw internal notification type — no more
/// snake_case enums (`job_failure`) leaking into the UI.
fn type_label(notification_type: &str) -> String {
    match notification_type {
        "job_failure" => "Job failure".to_string(),
        "mentioned" => "Mention".to_string(),
        "campaign_reply" => "Campaign reply".to_string(),
        "sync_failure" => "Sync failure".to_string(),
        "sync_completed" => "Sync completed".to_string(),
        "rating_received" => "Rating".to_string(),
        other => {
            let mut text = other.replace('_', " ");
            if let Some(first) = text.get(0..1) {
                let upper = first.to_uppercase();
                text.replace_range(0..1, &upper);
            }
            text
        }
    }
}

/// AUD-P1-1 (C1): resolve a notification's deep-link fields to the page
/// that shows the thing it is about. The records already carry
/// `conversation_id`, `customer_local_id`, `issue_id`, `campaign_id` and
/// `job_id` — this maps them onto the existing NavRequest deep links
/// (conversation > customer > issue > campaign > job, first hit wins).
fn deep_link(record: &NotificationRecord) -> Option<NavRequest> {
    if record.conversation_id.is_some() {
        return record.conversation_id.map(NavRequest::OpenConversation);
    }
    if record.customer_local_id.is_some() {
        return record.customer_local_id.map(NavRequest::OpenCustomer);
    }
    if record.issue_id.is_some() {
        return Some(NavRequest::OpenKnownIssues);
    }
    if record.campaign_id.is_some() {
        return Some(NavRequest::Page(PageKey::Outreach));
    }
    if record.job_id.is_some() {
        return Some(NavRequest::Page(PageKey::Diagnostics));
    }
    None
}

/// AUD-P1-1 (C1): where the Open button takes you, in hover text.
fn deep_link_hover(record: &NotificationRecord) -> Option<String> {
    let nav = deep_link(record)?;
    Some(match nav {
        NavRequest::OpenConversation(id) => format!(
            "open conversation #{} in the Inbox",
            record.conversation_number.unwrap_or(id)
        ),
        NavRequest::OpenCustomer(id) => format!("open this customer's profile (#{id})"),
        NavRequest::OpenKnownIssues => "open Issue Radar on the Known issues tab".to_string(),
        NavRequest::Page(PageKey::Outreach) => {
            "open Outreach — the campaign this reply belongs to".to_string()
        }
        NavRequest::Page(PageKey::Diagnostics) => {
            "open Diagnostics — the background job that failed".to_string()
        }
        _ => "open the related page".to_string(),
    })
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

    fn toggle_group(&mut self, db: &DbHandle, ids: &[i64], read: bool) {
        let me = db
            .with_read(spp_core::notifications::me_user_local_id)
            .ok()
            .flatten();
        for id in ids {
            let result =
                db.with_write(|conn| spp_core::notifications::mark_read(conn, *id, me, read));
            if let Err(e) = result {
                self.error = Some(e.to_string());
            }
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

    pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle) -> Option<NavRequest> {
        self.ensure_loaded(db);

        let subtitle = format!("{} total · {} unread", self.total, self.unread);
        design::page_header(ui, "Notifications", Some(subtitle.as_str()), |ui| {
            if design::button(ui, "Mark All Read", ButtonVariant::Text).clicked() {
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
            return None;
        }

        // UX-13: collapse identical notifications into one card per group so
        // a repeated failure reads as "×18", not as 18 duplicate cards.
        let groups = group_records(&rows);
        let mut toggled: Option<(Vec<i64>, bool)> = None;
        let mut nav: Option<NavRequest> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for group in &groups {
                let unread = group.any_unread();
                let sample = group.sample();
                let latest = group.latest();
                design::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        if unread {
                            design::status_dot(ui, BadgeTone::Info, false);
                        }
                        let tone = if sample.severity == "error" {
                            BadgeTone::Destructive
                        } else {
                            BadgeTone::Info
                        };
                        design::badge(ui, &type_label(sample.notification_type.as_str()), tone);
                        if group.count() > 1 {
                            design::page_counter(ui, &format!("×{}", group.count()));
                        }
                        let span = if group.count() > 1 {
                            format!(
                                "{} · latest {}",
                                relative_time(sample.created_at.as_str()),
                                relative_time(latest.created_at.as_str())
                            )
                        } else {
                            relative_time(sample.created_at.as_str())
                        };
                        ui.label(
                            egui::RichText::new(span)
                                .small()
                                .color(design::tokens(ui).muted_foreground),
                        )
                        .on_hover_text(latest.created_at.as_str());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if design::button(
                                ui,
                                if unread { "Mark Read" } else { "Mark Unread" },
                                ButtonVariant::Text,
                            )
                            .on_hover_text(if group.count() > 1 {
                                format!("applies to all {} in this group", group.count())
                            } else {
                                "toggle this notification".to_string()
                            })
                            .clicked()
                            {
                                toggled = Some((group.ids(), !unread));
                            }
                            // AUD-P1-1 (C1): the card's deep-link fields are
                            // live — "mentioned you on #501" is one click
                            // from the conversation now.
                            if nav.is_none()
                                && let Some(hover) = deep_link_hover(sample)
                                && design::button(ui, "Open", ButtonVariant::Outline)
                                    .on_hover_text(hover)
                                    .clicked()
                            {
                                nav = deep_link(sample);
                            }
                        });
                    });
                    ui.label(
                        egui::RichText::new(sample.title.as_str())
                            .size(13.5)
                            .strong()
                            .color(design::tokens(ui).foreground),
                    );
                    if let Some(body) = &sample.body {
                        // UX-13: bodies render as truncated small code with
                        // the full text one hover away. UX-35: job-failure
                        // bodies are now friendly summaries — their hover
                        // reveals the verbatim technical error kept in the
                        // structured payload.
                        let t = design::tokens(ui);
                        let galley = egui::WidgetText::from(
                            egui::RichText::new(body.as_str())
                                .monospace()
                                .size(design::scale::text::SMALL)
                                .color(t.muted_foreground),
                        )
                        .into_galley(
                            ui,
                            Some(egui::TextWrapMode::Truncate),
                            ui.available_width(),
                            egui::TextStyle::Small,
                        );
                        let (id, rect) = ui.allocate_space(galley.size());
                        ui.painter().galley(rect.min, galley, t.muted_foreground);
                        ui.advance_cursor_after_rect(rect);
                        if let Some(detail) = sample.technical_detail() {
                            let hover = ui.interact(rect, id, egui::Sense::hover());
                            hover.on_hover_text(format!("Technical detail: {detail}"));
                        }
                    }
                });
            }
        });
        if let Some((ids, read)) = toggled {
            self.toggle_group(db, &ids, read);
        }
        nav
    }

    /// AUD-P1-1 (C1): the current unread count, for the shell chrome's
    /// sidebar badge (synced after every reload).
    #[must_use]
    pub fn unread_badge(&self) -> u32 {
        self.unread.max(0) as u32
    }
}

impl Default for NotificationsPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::format_intraday_minutes;
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

        page.toggle_group(&db, &[1], true);
        assert_eq!(page.unread, 0, "explicit mark-read flips to read");

        page.toggle_group(&db, &[1], false);
        assert_eq!(page.unread, 1, "explicit mark-unread flips back");
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

        page.toggle_group(&db, &[1], false);
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
    fn ux13_identical_notifications_collapse_into_one_group() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            for i in 0..18 {
                seed_notification(
                    conn,
                    100 + i,
                    "job_failure",
                    "Background job failed (ai/analyze_ticket)",
                    false,
                );
            }
            seed_notification(conn, 200, "mentioned", "Ada mentioned you", true);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        let groups = group_records(&page.records);
        assert_eq!(groups.len(), 2, "18 identical rows collapse to one group");
        let failure = groups
            .iter()
            .find(|g| g.sample().notification_type.as_str() == "job_failure")
            .expect("failure group");
        assert_eq!(failure.count(), 18);
        assert!(failure.any_unread());
        assert_eq!(failure.ids().len(), 18);
    }

    #[test]
    fn ux13_group_toggle_flips_every_record_in_the_group() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "job_failure", "same failure", false);
            seed_notification(conn, 2, "job_failure", "same failure", false);
        });
        let mut page = NotificationsPage::new();
        page.reload(&db);
        let groups = group_records(&page.records);
        let ids = groups[0].ids();
        page.toggle_group(&db, &ids, true);
        assert_eq!(page.unread, 0, "both group members flip to read");
        page.toggle_group(&db, &ids, false);
        assert_eq!(page.unread, 2, "both group members flip back");
    }

    #[test]
    fn ux13_type_labels_are_human() {
        assert_eq!(type_label("job_failure"), "Job failure");
        assert_eq!(type_label("mentioned"), "Mention");
        assert_eq!(type_label("some_new_kind"), "Some new kind");
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

    // ---- AUD-P1-1 (C1): notification click-through + unread badge ----

    fn link_record(
        conversation_id: Option<i64>,
        customer_local_id: Option<i64>,
        issue_id: Option<i64>,
        campaign_id: Option<i64>,
        job_id: Option<i64>,
    ) -> NotificationRecord {
        NotificationRecord {
            id: 1,
            notification_type: spp_core::catalog::NotificationType::Mentioned,
            severity: "info".to_string(),
            title: "mentioned you".to_string(),
            body: None,
            target_user_local_id: Some(5),
            actor_user_local_id: None,
            conversation_id,
            conversation_number: conversation_id.map(|id| 500 + id),
            customer_local_id,
            issue_id,
            campaign_id,
            job_id,
            side_thread_id: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            read_at: None,
            payload: None,
        }
    }

    #[test]
    fn audp11_deep_link_prefers_the_conversation_over_everything_else() {
        let record = link_record(Some(42), Some(9), Some(3), Some(7), Some(11));
        assert_eq!(
            deep_link(&record),
            Some(NavRequest::OpenConversation(42)),
            "conversation wins when several targets are present"
        );
    }

    #[test]
    fn audp11_deep_link_falls_through_the_target_chain() {
        assert_eq!(
            deep_link(&link_record(None, Some(9), None, None, None)),
            Some(NavRequest::OpenCustomer(9))
        );
        assert_eq!(
            deep_link(&link_record(None, None, Some(3), Some(7), Some(11))),
            Some(NavRequest::OpenKnownIssues)
        );
        assert_eq!(
            deep_link(&link_record(None, None, None, Some(7), Some(11))),
            Some(NavRequest::Page(PageKey::Outreach))
        );
        assert_eq!(
            deep_link(&link_record(None, None, None, None, Some(11))),
            Some(NavRequest::Page(PageKey::Diagnostics))
        );
        assert_eq!(
            deep_link(&link_record(None, None, None, None, None)),
            None,
            "no deep-link fields — no Open affordance"
        );
    }

    #[test]
    fn audp11_unread_badge_tracks_the_page_reload() {
        let db = fixture(Vec::new());
        db.with_write(|conn| {
            set_me(conn);
            seed_notification(conn, 1, "mentioned", "one", false);
            seed_notification(conn, 2, "sync_failure", "two", false);
            seed_notification(conn, 3, "mentioned", "three", true);
        });
        let mut page = NotificationsPage::new();
        assert_eq!(page.unread_badge(), 0, "nothing loaded yet");
        page.reload(&db);
        assert_eq!(page.unread_badge(), 2);
        page.mark_all_read(&db);
        assert_eq!(page.unread_badge(), 0, "the badge clears with the reload");
    }
}

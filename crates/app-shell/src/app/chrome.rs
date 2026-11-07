use super::*;

pub(super) fn human_outbound_kind(kind: &str) -> &str {
    match kind {
        "create_reply" => "Reply",
        "create_note" => "Note",
        "update_status" => "Status change",
        "assign" => "Assignment",
        other => other,
    }
}

#[must_use]
pub(super) fn sync_completed_toast(
    e: &spp_core::events::SyncCompletedEvent,
) -> Option<(crate::toasts::ToastKind, String, Option<String>)> {
    if e.trigger != spp_core::events::SyncTrigger::Manual {
        return None;
    }
    if e.processed > 0 {
        let detail = match e.kind.as_str() {
            "initial" => "Your Help Scout history is in the local mirror now.".to_string(),
            "docs" => "Help Center articles refreshed.".to_string(),
            _ => "Latest changes pulled from Help Scout.".to_string(),
        };
        Some((
            crate::toasts::ToastKind::Success,
            format!("Sync finished: {} record(s) updated", e.processed),
            Some(detail),
        ))
    } else if e.errors > 0 {
        Some((
            crate::toasts::ToastKind::Warning,
            "Sync finished with errors".to_string(),
            Some("See Diagnostics for the exact failures.".to_string()),
        ))
    } else {
        Some((
            crate::toasts::ToastKind::Info,
            "Up to date — nothing new from Help Scout".to_string(),
            Some(
                "The last sync checked the account and found no changes. \
                 This is what a healthy, quiet account looks like."
                    .to_string(),
            ),
        ))
    }
}

pub(super) const NAV_GROUPS: &[(&str, &[PageKey])] = &[
    (
        "Workspace",
        &[
            PageKey::Dashboard,
            PageKey::Inbox,
            PageKey::LiveChats,
            PageKey::Operations,
        ],
    ),
    (
        "Customers & knowledge",
        &[
            PageKey::Customers,
            PageKey::Organizations,
            PageKey::Knowledge,
        ],
    ),
    (
        "Insights",
        &[PageKey::Reports, PageKey::IssueRadar, PageKey::Incidents],
    ),
    (
        "Automation & AI",
        &[
            PageKey::CustomObjects,
            PageKey::Automation,
            PageKey::AiCenter,
            PageKey::Outreach,
        ],
    ),
    (
        "System",
        &[
            PageKey::Notifications,
            PageKey::Search,
            PageKey::SupportGraph,
            PageKey::Connectors,
            PageKey::Diagnostics,
            PageKey::SyncHealth,
            PageKey::Settings,
            PageKey::Onboarding,
            PageKey::Shell,
        ],
    ),
];

pub(super) fn nav_row(ui: &mut egui::Ui, current: PageKey, key: PageKey) -> bool {
    let selected = current == key;
    let t = design::tokens(ui);
    let fg = if selected {
        t.selection_foreground
    } else {
        t.foreground
    };
    // UXE-1-2: 30px rows, 13px labels, selected label strong — the nav
    // grid keeps one rhythm top to bottom.
    let mut rich = egui::RichText::new(key.label()).size(13.0).color(fg);
    if selected {
        rich = rich.strong();
    }
    let galley = egui::WidgetText::from(rich).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 30.0));
    let response = ui.interact(rect, auto_id.with(key.label()), egui::Sense::click());
    let fill = if selected {
        t.selection
    } else if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
    }
    let icon_size = 15.0;
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - icon_size / 2.0),
        egui::pos2(
            rect.min.x + 10.0 + icon_size,
            rect.center().y + icon_size / 2.0,
        ),
    );
    Icon::for_page(key).paint_at(
        ui.painter(),
        icon_rect,
        if selected { fg } else { t.muted_foreground },
    );
    ui.painter().galley(
        egui::pos2(
            icon_rect.max.x + 8.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        fg,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    if let Some(cursor) = ui.visuals().interact_cursor
        && response.hovered()
    {
        ui.ctx().set_cursor_icon(cursor);
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, key.label())
    });
    response.clicked()
}

pub(super) fn palette_trigger(ui: &mut egui::Ui) -> egui::Response {
    let t = design::tokens(ui);
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 32.0));
    let response = ui
        .interact(
            rect,
            auto_id.with("spp-nav-palette-trigger"),
            egui::Sense::click(),
        )
        .on_hover_text("command palette (Ctrl/Cmd+K)");
    let fill = if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(7), fill);
    }
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(7),
        egui::Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - 7.5),
        egui::pos2(rect.min.x + 25.0, rect.center().y + 7.5),
    );
    Icon::Search.paint_at(ui.painter(), icon_rect, t.muted_foreground);
    ui.painter().text(
        egui::pos2(icon_rect.max.x + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Search…",
        egui::FontId::proportional(13.0),
        t.muted_foreground,
    );
    let kbd = "⌘K";
    let kbd_font = egui::FontId::proportional(11.0);
    let kbd_width = 26.0;
    let kbd_rect = egui::Rect::from_min_max(
        egui::pos2(rect.max.x - kbd_width - 8.0, rect.center().y - 9.0),
        egui::pos2(rect.max.x - 8.0, rect.center().y + 9.0),
    );
    ui.painter()
        .rect_filled(kbd_rect, egui::CornerRadius::same(4), t.muted);
    ui.painter().text(
        kbd_rect.center(),
        egui::Align2::CENTER_CENTER,
        kbd,
        kbd_font,
        t.muted_foreground,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(9),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::Role::Button,
            ui.is_enabled(),
            "search (command palette)".to_owned(),
        )
    });
    response
}

pub(super) fn theme_row(ui: &mut egui::Ui, next: Theme) -> bool {
    let t = design::tokens(ui);
    let (icon, label) = match next {
        Theme::Dark => (Icon::Moon, "switch to dark theme"),
        Theme::Light => (Icon::Sun, "switch to light theme"),
    };
    let galley = egui::WidgetText::from(
        egui::RichText::new(label)
            .size(13.0)
            .color(t.muted_foreground),
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let (auto_id, rect) = ui.allocate_space(egui::vec2(ui.available_width(), 30.0));
    let response = ui.interact(rect, auto_id.with("spp-nav-theme"), egui::Sense::click());
    let fill = if response.hovered() || response.has_focus() {
        t.accent
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
    }
    let icon_rect = egui::Rect::from_min_max(
        egui::pos2(rect.min.x + 10.0, rect.center().y - 7.5),
        egui::pos2(rect.min.x + 25.0, rect.center().y + 7.5),
    );
    icon.paint_at(ui.painter(), icon_rect, t.muted_foreground);
    ui.painter().galley(
        egui::pos2(
            icon_rect.max.x + 8.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        t.muted_foreground,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(8),
            egui::Stroke::new(2.0, t.ring),
            egui::StrokeKind::Outside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label.to_owned())
    });
    response.clicked()
}

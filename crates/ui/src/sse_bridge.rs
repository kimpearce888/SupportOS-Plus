use crate::sse::LiveEvent;
use crate::state::UiState;
use crate::toasts::ToastKind;

fn rating_label(rating: &str) -> Option<&'static str> {
    match rating {
        "great" => Some("Great"),
        "okay" => Some("Okay"),
        "not-good" => Some("Not good"),
        _ => None,
    }
}

#[must_use]
pub fn rating_toast(
    rating: Option<&str>,
    conversation_number: Option<i64>,
    customer_name: Option<&str>,
    comments: Option<&str>,
) -> Option<(ToastKind, String, Option<String>)> {
    let rating = rating?;
    let label = rating_label(rating)?;
    let kind = match rating {
        "great" => ToastKind::Success,
        "okay" => ToastKind::Info,
        _ => ToastKind::Warning,
    };
    let mut message = format!("New {label} rating");
    if let Some(n) = conversation_number {
        message.push_str(&format!(" on #{n}"));
    }
    if let Some(name) = customer_name {
        message.push_str(&format!(" · {name}"));
    }
    let detail = comments.filter(|c| !c.is_empty()).map(String::from);
    Some((kind, message, detail))
}

#[must_use]
pub fn webhook_conversation_toast(
    reason: &str,
    conversation_number: Option<i64>,
    subject: Option<&str>,
) -> Option<(ToastKind, String)> {
    if reason != "webhook" {
        return None;
    }
    let n = conversation_number?;
    let mut message = format!("#{n} updated — pushed by webhook");
    if let Some(subject) = subject.filter(|s| !s.is_empty()) {
        message.push_str(&format!(": {}", truncate_chars(subject, 80)));
    }
    Some((ToastKind::Info, message))
}

#[must_use]
pub fn campaign_toasts(
    campaign_id: i64,
    status: &str,
    sent: u32,
    failed: u32,
    remaining: u32,
) -> Vec<(ToastKind, String)> {
    let mut out = Vec::new();
    if status == "completed" {
        let mut message = format!("Campaign #{campaign_id} completed — {sent} sent");
        if failed > 0 {
            message.push_str(&format!(", {failed} failed"));
        }
        message.push('.');
        out.push((ToastKind::Success, message));
    } else if failed > 0 && remaining == 0 {
        out.push((
            ToastKind::Warning,
            format!("Campaign #{campaign_id} finished with {failed} failed recipient(s)."),
        ));
    }
    out
}

#[must_use]
pub fn critical_notification_toast(severity: &str, title: &str) -> Option<(ToastKind, String)> {
    if severity == "critical" {
        Some((ToastKind::Error, title.to_string()))
    } else {
        None
    }
}

#[must_use]
fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn handle_event(ui: &UiState, event: &LiveEvent) {
    match event {
        LiveEvent::RatingReceived {
            rating,
            conversation_number,
            customer_name,
            comments,
            ..
        } => {
            for key in crate::queries::keys_for_event("ratings", None, 0) {
                crate::queries::invalidate(key);
            }
            if let Some((kind, message, detail)) = rating_toast(
                rating.as_deref(),
                *conversation_number,
                customer_name.as_deref(),
                comments.as_deref(),
            ) {
                crate::toasts::push(kind, message, detail);
            }
        }
        LiveEvent::RatingsRefreshed { .. } => {
            for key in crate::queries::keys_for_event("ratings", None, 0) {
                crate::queries::invalidate(key);
            }
        }
        LiveEvent::SyncCompleted {
            kind, processed, ..
        } => {
            for key in crate::queries::keys_for_event("sync", Some(kind), *processed) {
                crate::queries::invalidate(key);
            }
            ui.refresh_nav_counts();
        }
        LiveEvent::ConversationUpdated {
            conversation_number,
            subject,
            reason,
            ..
        } => {
            for key in crate::queries::keys_for_event("conversation", None, 0) {
                crate::queries::invalidate(key);
            }
            ui.refresh_nav_counts();
            if let Some((kind, message)) =
                webhook_conversation_toast(reason, *conversation_number, subject.as_deref())
            {
                crate::toasts::push(kind, message, None);
            }
        }
        LiveEvent::CampaignUpdated {
            campaign_id,
            status,
            sent,
            failed,
            remaining,
            ..
        } => {
            for key in crate::queries::keys_for_event("campaign", None, 0) {
                crate::queries::invalidate(key);
            }
            for (kind, message) in campaign_toasts(*campaign_id, status, *sent, *failed, *remaining)
            {
                crate::toasts::push(kind, message, None);
            }
        }
        LiveEvent::NotificationReceived {
            severity, title, ..
        } => {
            for key in crate::queries::keys_for_event("notification", None, 0) {
                crate::queries::invalidate(key);
            }
            ui.refresh_nav_counts();
            if let Some((kind, message)) = critical_notification_toast(severity, title) {
                crate::toasts::push(kind, message, None);
            }
        }
        LiveEvent::Hello { .. } | LiveEvent::Error { .. } => {}
    }
}

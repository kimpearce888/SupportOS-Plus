use std::sync::Mutex;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

pub const EVENT_NAMES: [&str; 7] = [
    "hello",
    "ratings",
    "sync",
    "conversation",
    "campaign",
    "notification",
    "error",
];

#[derive(Debug, Clone)]
pub enum LiveEvent {
    Hello {
        at: String,
        version: String,
    },
    RatingReceived {
        rating: Option<String>,
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        customer_id: Option<i64>,
        customer_name: Option<String>,
        comments: Option<String>,
        at: String,
    },
    RatingsRefreshed {
        processed: u32,
        fresh: u32,
        at: String,
    },
    SyncCompleted {
        kind: String,
        processed: u32,
        errors: u32,
        at: String,
    },
    ConversationUpdated {
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        mailbox_id: Option<i64>,
        subject: Option<String>,
        reason: String,
        at: String,
    },
    CampaignUpdated {
        campaign_id: i64,
        status: String,
        sent: u32,
        failed: u32,
        unknown: u32,
        remaining: u32,
        at: String,
    },
    NotificationReceived {
        id: i64,
        kind: String,
        severity: String,
        title: String,
        conversation_id: Option<i64>,
        conversation_number: Option<i64>,
        customer_id: Option<i64>,
        target_user_local_id: Option<i64>,
        unread_count: u32,
        at: String,
    },
    Error {
        message: String,
    },
}

pub type EventHandler = Box<dyn Fn(&LiveEvent) + Send + Sync + 'static>;

static SUBSCRIBERS: Mutex<Vec<EventHandler>> = Mutex::new(Vec::new());

pub fn subscribe(handler: EventHandler) -> Result<Box<dyn FnOnce() -> Result<(), String>>, String> {
    let mut subs = SUBSCRIBERS
        .lock()
        .map_err(|e| format!("subscriber lock poisoned: {e}"))?;
    subs.push(handler);
    let idx = subs.len() - 1;
    drop(subs);

    ensure_source()?;

    Ok(Box::new(move || -> Result<(), String> {
        let mut subs = SUBSCRIBERS
            .lock()
            .map_err(|e| format!("subscriber lock poisoned: {e}"))?;
        if idx < subs.len() {
            subs[idx] = Box::new(|_| {});
        }
        Ok(())
    }))
}

#[allow(clippy::missing_const_for_thread_local)]
fn ensure_source() -> Result<(), String> {
    thread_local! {
        static SOURCE: std::cell::RefCell<Option<EventSourceHandle>> = std::cell::RefCell::new(None);
    }

    SOURCE.with(|s| {
        if s.borrow().is_some() {
            return Ok(());
        }

        let _window = web_sys::window().ok_or("no window")?;
        let url = format!("{}/api/events", crate::api::api_base());
        let event_source = web_sys::EventSource::new(&url)
            .map_err(|e| format!("EventSource::new({url}) failed: {e:?}"))?;

        for name in EVENT_NAMES {
            let cb = Closure::<dyn FnMut(web_sys::MessageEvent)>::new({
                let name = name.to_string();
                move |event: web_sys::MessageEvent| {
                    let data_str = event.data().as_string().unwrap_or_else(|| "{}".to_string());
                    dispatch_named(&name, &data_str);
                }
            });
            event_source
                .add_event_listener_with_callback(name, cb.as_ref().unchecked_ref())
                .map_err(|e| format!("addEventListener({name}) failed: {e:?}"))?;
            cb.forget();
        }

        let handle = EventSourceHandle {
            source: event_source,
        };
        *s.borrow_mut() = Some(handle);
        Ok(())
    })
}

fn dispatch_named(name: &str, data: &str) {
    let event = match (name, serde_json::from_str::<serde_json::Value>(data)) {
        (_, Ok(v)) if !v.is_null() => v,
        _ => return,
    };
    let live = match name {
        "hello" => parse_h(&event),
        "ratings" => parse_ratings(&event),
        "sync" => LiveEvent::SyncCompleted {
            kind: str_field(&event, "kind").unwrap_or_default(),
            processed: num_field(&event, "processed").unwrap_or(0).max(0) as u32,
            errors: num_field(&event, "errors").unwrap_or(0).max(0) as u32,
            at: str_field(&event, "at").unwrap_or_default(),
        },
        "conversation" => LiveEvent::ConversationUpdated {
            conversation_id: num_field(&event, "conversationId"),
            conversation_number: num_field(&event, "conversationNumber"),
            mailbox_id: num_field(&event, "mailboxId"),
            subject: str_field(&event, "subject"),
            reason: str_field(&event, "reason").unwrap_or_default(),
            at: str_field(&event, "at").unwrap_or_default(),
        },
        "campaign" => LiveEvent::CampaignUpdated {
            campaign_id: num_field(&event, "campaignId").unwrap_or(0),
            status: str_field(&event, "status").unwrap_or_default(),
            sent: num_field(&event, "sent").unwrap_or(0).max(0) as u32,
            failed: num_field(&event, "failed").unwrap_or(0).max(0) as u32,
            unknown: num_field(&event, "unknown").unwrap_or(0).max(0) as u32,
            remaining: num_field(&event, "remaining").unwrap_or(0).max(0) as u32,
            at: str_field(&event, "at").unwrap_or_default(),
        },
        "notification" => LiveEvent::NotificationReceived {
            id: num_field(&event, "id").unwrap_or(0),
            kind: str_field(&event, "type").unwrap_or_default(),
            severity: str_field(&event, "severity").unwrap_or_default(),
            title: str_field(&event, "title").unwrap_or_default(),
            conversation_id: num_field(&event, "conversationId"),
            conversation_number: num_field(&event, "conversationNumber"),
            customer_id: num_field(&event, "customerId"),
            target_user_local_id: num_field(&event, "targetUserLocalId"),
            unread_count: num_field(&event, "unreadCount").unwrap_or(0).max(0) as u32,
            at: str_field(&event, "at").unwrap_or_default(),
        },
        "error" => LiveEvent::Error {
            message: str_field(&event, "message").unwrap_or_default(),
        },
        _ => return,
    };
    let subs = match SUBSCRIBERS.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    for handler in subs.iter() {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handler(&live);
        }));
    }
}

fn parse_h(v: &serde_json::Value) -> LiveEvent {
    LiveEvent::Hello {
        at: str_field(v, "at").unwrap_or_default(),
        version: str_field(v, "version").unwrap_or_default(),
    }
}

fn parse_ratings(v: &serde_json::Value) -> LiveEvent {
    if v.get("processed").is_some() {
        LiveEvent::RatingsRefreshed {
            processed: num_field(v, "processed").unwrap_or(0).max(0) as u32,
            fresh: num_field(v, "fresh").unwrap_or(0).max(0) as u32,
            at: str_field(v, "at").unwrap_or_default(),
        }
    } else {
        LiveEvent::RatingReceived {
            rating: str_field(v, "rating"),
            conversation_id: num_field(v, "conversationId"),
            conversation_number: num_field(v, "conversationNumber"),
            customer_id: num_field(v, "customerId"),
            customer_name: str_field(v, "customerName"),
            comments: str_field(v, "comments"),
            at: str_field(v, "at").unwrap_or_default(),
        }
    }
}

fn str_field(v: &serde_json::Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(String::from)
}

fn num_field(v: &serde_json::Value, key: &str) -> Option<i64> {
    v.get(key)?.as_i64()
}

struct EventSourceHandle {
    #[allow(dead_code)]
    source: web_sys::EventSource,
}

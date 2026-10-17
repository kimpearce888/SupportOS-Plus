use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::{HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const RATE_LIMIT_MAX: u32 = 300;
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);
pub const RATE_LIMIT_MAP_MAX: usize = 512;
pub const WEBHOOK_PATH: &str = "/api/webhooks/helpscout";

#[derive(Clone, Copy, Debug)]
struct HitEntry {
    count: u32,
    reset_at: Instant,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    hits: std::sync::Arc<Mutex<HashMap<String, HitEntry>>>,
}

impl RateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check(&self, method: &Method, path: &str, key: &str) -> Result<(), u64> {
        if method == Method::GET || method == Method::HEAD || method == Method::OPTIONS {
            return Ok(());
        }
        if path == WEBHOOK_PATH {
            return Ok(());
        }

        let now = Instant::now();
        let mut hits = self
            .hits
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if hits.len() > RATE_LIMIT_MAP_MAX {
            hits.retain(|_, v| now < v.reset_at);
        }

        let entry = hits.entry(key.to_string()).or_insert(HitEntry {
            count: 0,
            reset_at: now + RATE_LIMIT_WINDOW,
        });

        if now > entry.reset_at {
            entry.count = 0;
            entry.reset_at = now + RATE_LIMIT_WINDOW;
        }

        entry.count += 1;
        if entry.count > RATE_LIMIT_MAX {
            let retry_after_secs = entry
                .reset_at
                .saturating_duration_since(now)
                .as_secs()
                .max(1);
            return Err(retry_after_secs);
        }

        Ok(())
    }

    #[must_use]
    pub fn too_many_requests_response(retry_after_secs: u64) -> Response {
        let body = serde_json::json!({
            "statusCode": 429,
            "error": "TooManyRequests",
            "message": "Too many write requests in one minute. Slow down and retry shortly."
        });
        let retry_value = HeaderValue::from_str(&retry_after_secs.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("60"));
        Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::RETRY_AFTER, retry_value)
            .body(axum::body::Body::from(body.to_string()))
            .expect("valid 429 response")
    }
}

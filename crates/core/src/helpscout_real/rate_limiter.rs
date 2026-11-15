use super::*;

#[derive(Debug, Clone)]
pub(crate) struct RateLimitState {
    pub limit_per_minute: i64,
    pub remaining: Option<i64>,
    pub retry_after_sec: Option<i64>,
    pub updated_at: String,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            limit_per_minute: 150,
            remaining: None,
            retry_after_sec: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub struct HsRateLimiter {
    state: Mutex<RateLimitState>,
    send_timestamps: Mutex<Vec<i64>>,
    db: Option<crate::db::DbHandle>,
}

impl HsRateLimiter {
    pub fn new(db: Option<crate::db::DbHandle>) -> Self {
        let limiter = Self {
            state: Mutex::new(RateLimitState::default()),
            send_timestamps: Mutex::new(Vec::new()),
            db,
        };
        limiter.load();
        limiter
    }

    #[must_use]
    pub(crate) fn rate_limit_state(&self) -> RateLimitState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    fn load(&self) {
        let Some(db) = &self.db else { return };
        let conn = db.write_guard();
        let _ = apply_m043(&conn);
        let Ok(row) = conn.query_row(
            "SELECT limit_per_minute, remaining, retry_after_sec, updated_at
               FROM hs_rate_limit WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, String>(3)?,
                ))
            },
        ) else {
            return;
        };
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.limit_per_minute = row.0;
        state.remaining = row.1;
        state.retry_after_sec = row.2;
        state.updated_at = row.3;
    }

    fn persist(&self, state: &RateLimitState) {
        let Some(db) = &self.db else { return };
        let conn = db.write_guard();
        let _ = conn.execute(
            "INSERT INTO hs_rate_limit (id, limit_per_minute, remaining, retry_after_sec, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                limit_per_minute = excluded.limit_per_minute,
                remaining = excluded.remaining,
                retry_after_sec = excluded.retry_after_sec,
                updated_at = excluded.updated_at",
            rusqlite::params![
                state.limit_per_minute,
                state.remaining,
                state.retry_after_sec,
                state.updated_at
            ],
        );
    }

    pub(crate) fn record_response(
        &self,
        limit: Option<i64>,
        remaining: Option<i64>,
        retry_after: Option<i64>,
        is_write: bool,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(l) = limit {
                state.limit_per_minute = l;
            }
            if remaining.is_some() {
                state.remaining = remaining;
            }
            state.retry_after_sec = if retry_after.is_some() {
                retry_after
            } else {
                None
            };
            state.updated_at = chrono::Utc::now().to_rfc3339();
            self.persist(&state);
        }
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        sends.push(now_ms);
        if is_write {
            sends.push(now_ms);
        }
    }

    pub(crate) fn record_error_429(&self, retry_after_sec: Option<i64>) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.retry_after_sec = Some(retry_after_sec.unwrap_or(30));
        state.remaining = Some(0);
        state.updated_at = chrono::Utc::now().to_rfc3339();
        self.persist(&state);
    }

    pub fn snapshot(&self) -> Value {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        json!({
            "limitPerMinute": state.limit_per_minute,
            "remaining": state.remaining,
            "retryAfterSec": state.retry_after_sec,
            "updatedAt": state.updated_at,
            "inFlightWindow": sends.len(),
        })
    }
}

impl QueueRateLimiter for HsRateLimiter {
    fn wait_time_ms(&self, _is_write: bool) -> i64 {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let (Some(retry), Some(0)) = (state.retry_after_sec, state.remaining)
            && let Ok(updated) = chrono::DateTime::parse_from_rfc3339(&state.updated_at)
        {
            let reset_at = updated.timestamp_millis() + retry * 1000;
            if reset_at > now_ms {
                return reset_at - now_ms + 250;
            }
        }
        let mut sends = self
            .send_timestamps
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        sends.retain(|t| now_ms - t < 60_000);
        let effective_limit = (state.limit_per_minute - 5).max(5);
        if sends.len() as i64 + 1 > effective_limit
            && let Some(oldest) = sends.first()
        {
            let wait = 60_000 - (now_ms - oldest) + 100;
            if wait > 0 {
                return wait;
            }
        }
        0
    }
}

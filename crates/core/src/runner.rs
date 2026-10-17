use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::Connection;

use crate::error::Result;
use crate::jobs::{self, ClaimedJob};

#[derive(Debug, Clone)]
pub enum HandlerOutcome {
    Success,
    Failure { message: String },
}

impl HandlerOutcome {
    #[must_use]
    pub fn success() -> Self {
        Self::Success
    }

    #[must_use]
    pub fn failure(message: impl Into<String>) -> Self {
        Self::Failure {
            message: message.into(),
        }
    }
}

pub trait JobHandler: Send + Sync {
    fn handle(&self, payload: &str) -> HandlerOutcome;
}

#[derive(Clone, Default)]
pub struct JobRegistry {
    handlers: Arc<HashMap<String, Arc<dyn JobHandler>>>,
}

impl JobRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn register(mut self, kind: &str, handler: Arc<dyn JobHandler>) -> Self {
        Arc::make_mut(&mut self.handlers).insert(kind.to_string(), handler);
        self
    }

    #[must_use]
    pub fn get(&self, kind: &str) -> Option<Arc<dyn JobHandler>> {
        self.handlers.get(kind).cloned()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

pub struct Runner<'a> {
    conn: &'a mut Connection,
    registry: &'a JobRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub processed: u32,
    pub succeeded: u32,
    pub failed: u32,
    pub dead_lettered: u32,
}

impl std::fmt::Display for RunSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} processed, {} succeeded, {} failed ({} dead-lettered)",
            self.processed, self.succeeded, self.failed, self.dead_lettered
        )
    }
}

impl<'a> Runner<'a> {
    pub fn new(conn: &'a mut Connection, registry: &'a JobRegistry) -> Self {
        Self { conn, registry }
    }

    pub fn run_until_idle(&mut self, max_iterations: u32) -> Result<RunSummary> {
        jobs::ensure_jobs_table(self.conn)?;
        let mut summary = RunSummary {
            processed: 0,
            succeeded: 0,
            failed: 0,
            dead_lettered: 0,
        };

        for _ in 0..max_iterations {
            let Some(job) = jobs::claim_next(self.conn, None)? else {
                break;
            };
            summary.processed += 1;

            let outcome = self.execute(&job);
            match outcome {
                HandlerOutcome::Success => {
                    jobs::complete(self.conn, job.id)?;
                    summary.succeeded += 1;
                }
                HandlerOutcome::Failure { message } => {
                    let dead_before = count_dead(self.conn)?;
                    jobs::fail(self.conn, job.id, &message, true)?;
                    let dead_after = count_dead(self.conn)?;
                    summary.failed += 1;
                    if dead_after > dead_before {
                        summary.dead_lettered += 1;
                    }
                }
            }
        }
        Ok(summary)
    }

    fn execute(&self, job: &ClaimedJob) -> HandlerOutcome {
        match self.registry.get(&job.kind) {
            Some(handler) => handler.handle(&job.payload),
            None => HandlerOutcome::failure(format!(
                "no handler registered for job kind {:?}",
                job.kind
            )),
        }
    }
}

fn count_dead(conn: &Connection) -> Result<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM jobs WHERE status = 'failed'",
        [],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(0))
}

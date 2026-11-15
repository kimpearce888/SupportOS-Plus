use super::*;
use crate::db::DbHandle;
use crate::error::Error;
use crate::helpscout::{
    ConversationPatch, ConversationQuery, CreateThreadInput, CustomerQuery, HelpScoutProvider,
    HsBeaconChat, HsDocArticle, HsMailbox, HsRating, HsTag, HsTeam, HsUser, Page, ThreadCreated,
};
use crate::helpscout_real::HsApiError;
use std::sync::Mutex;

struct ScriptedProvider {
    failures_remaining: Mutex<i64>,
    created: Mutex<Vec<crate::helpscout::CreateConversationInput>>,
}

impl ScriptedProvider {
    fn always_failing() -> Arc<Self> {
        Arc::new(Self {
            failures_remaining: Mutex::new(i64::MAX),
            created: Mutex::new(Vec::new()),
        })
    }

    fn succeeding() -> Arc<Self> {
        Arc::new(Self {
            failures_remaining: Mutex::new(0),
            created: Mutex::new(Vec::new()),
        })
    }

    fn sends(&self) -> usize {
        self.created.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

#[async_trait::async_trait]
impl HelpScoutProvider for ScriptedProvider {
    fn kind(&self) -> &'static str {
        "scripted"
    }
    async fn get_me(&self) -> Result<HsUser> {
        Err(Error::Other("not needed".into()))
    }
    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        Ok(Vec::new())
    }
    async fn list_users(&self) -> Result<Vec<HsUser>> {
        Ok(Vec::new())
    }
    async fn list_teams(&self) -> Result<Vec<HsTeam>> {
        Ok(Vec::new())
    }
    async fn list_tags(&self) -> Result<Vec<HsTag>> {
        Ok(Vec::new())
    }
    async fn list_conversations(
        &self,
        _query: &ConversationQuery,
    ) -> Result<Page<crate::helpscout::HsConversation>> {
        Ok(Page {
            items: Vec::new(),
            next_cursor: None,
        })
    }
    async fn list_customers(
        &self,
        _query: &CustomerQuery,
    ) -> Result<Page<crate::helpscout::HsCustomer>> {
        Ok(Page {
            items: Vec::new(),
            next_cursor: None,
        })
    }
    async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
        Ok(Vec::new())
    }
    async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
        Ok(Vec::new())
    }
    async fn list_ratings(&self) -> Result<Vec<HsRating>> {
        Ok(Vec::new())
    }
    async fn create_reply_thread(
        &self,
        _input: CreateThreadInput,
    ) -> Result<crate::helpscout::ThreadCreated> {
        Err(Error::Other("not needed".into()))
    }
    async fn create_note_thread(&self, _input: CreateThreadInput) -> Result<ThreadCreated> {
        Err(Error::Other("not needed".into()))
    }
    async fn update_conversation(
        &self,
        _conversation_id: i64,
        _patch: ConversationPatch,
    ) -> Result<bool> {
        Ok(false)
    }
    async fn create_conversation(
        &self,
        input: crate::helpscout::CreateConversationInput,
    ) -> Result<crate::helpscout::ConversationCreated> {
        let mut left = self
            .failures_remaining
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if *left > 0 {
            *left -= 1;
            return Err(Error::Other(Box::new(HsApiError {
                status_code: 500,
                message: "backend exploded".into(),
                friendly: "Help Scout returned a server error".into(),
                retryable: true,
            })));
        }
        drop(left);
        self.created
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(input);
        Ok(crate::helpscout::ConversationCreated {
            conversation_id: 778_001,
            number: 5252,
            thread_id: 9101,
        })
    }
}

fn db() -> DbHandle {
    let mut conn = Connection::open_in_memory().expect("db");
    crate::bootstrap::apply_all(&mut conn).expect("migrations");
    DbHandle::new(conn)
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn seed_campaign(conn: &Connection) -> (i64, i64) {
    conn.execute(
        "INSERT INTO mailboxes (remote_id, name) VALUES (2024, 'Support')",
        [],
    )
    .expect("mailbox");
    let mailbox_local = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO outreach_campaigns (name, subject, body, mailbox_local_id, tags, status)
         VALUES ('spring check', 'Quick question', 'Hello!', ?1, '[]', 'queued')",
        params![mailbox_local],
    )
    .expect("campaign");
    let campaign = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO outreach_recipients
             (campaign_id, customer_local_id, customer_remote_id, email, snapshot, state)
         VALUES (?1, 1, 5501, 'person@example.com', '{}', 'selected')",
        params![campaign],
    )
    .expect("recipient");
    let recipient = conn.last_insert_rowid();
    (campaign, recipient)
}

fn recipient_row(conn: &Connection, recipient: i64) -> (String, i64, Option<String>) {
    conn.query_row(
        "SELECT state, attempts, last_error FROM outreach_recipients WHERE id = ?1",
        params![recipient],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .expect("recipient row")
}

#[test]
fn retryable_failures_spend_attempts_and_terminate() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| seed_campaign(c));
    let provider: Arc<dyn crate::helpscout::HelpScoutProvider> = ScriptedProvider::always_failing();

    let mut last = serde_json::Value::Null;
    for _ in 0..5 {
        last = rt().block_on(async { send_batch(&db, &provider, campaign).await });
    }

    let (state, attempts, error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(
        attempts, 3,
        "exactly SEND_MAX_ATTEMPTS sends happen, then the recipient is parked: {last}"
    );
    assert_eq!(
        state, "failed",
        "the 429/5xx retry storm terminates at the cap instead of looping forever"
    );
    assert!(
        error.as_deref().unwrap_or("").contains("exhausted")
            || error.as_deref().unwrap_or("").contains("500"),
        "the failure is recorded honestly: {error:?} / {last}"
    );
}

#[test]
fn a_fresh_claimed_row_is_invisible_to_a_second_pick() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| {
        let (campaign, recipient) = seed_campaign(c);
        c.execute(
            "UPDATE outreach_recipients SET state = 'sending',
                 updated_at = datetime('now')
               WHERE id = ?1",
            params![recipient],
        )
        .expect("claim");
        (campaign, recipient)
    });
    let provider = ScriptedProvider::succeeding();
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
    rt().block_on(async { send_batch(&db, &sender, campaign).await });

    assert_eq!(
        provider.sends(),
        0,
        "a fresh 'sending' row is never re-picked"
    );
    let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(state, "sending", "the in-flight claim is left alone");
}

#[test]
fn a_stale_claimed_row_that_landed_is_recovered_as_sent() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| {
        let (campaign, recipient) = seed_campaign(c);
        c.execute(
            "INSERT INTO customers (remote_id, first_name, last_name, email)
             VALUES (5501, 'Pat', 'Person', 'person@example.com')",
            [],
        )
        .expect("customer");
        let customer_local = c.last_insert_rowid();
        c.execute(
            "INSERT INTO conversations (remote_id, number, subject, preview, status,
                 customer_local_id)
             VALUES (99001, 99, 'Quick question', 'preview', 'active', ?1)",
            params![customer_local],
        )
        .expect("landed conversation");
        c.execute(
            "UPDATE outreach_recipients SET state = 'sending',
                 updated_at = datetime('now', '-11 minutes')
               WHERE id = ?1",
            params![recipient],
        )
        .expect("stale claim");
        (campaign, recipient)
    });
    let provider = ScriptedProvider::succeeding();
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
    rt().block_on(async { send_batch(&db, &sender, campaign).await });

    assert_eq!(
        provider.sends(),
        0,
        "a crashed send that provably landed is marked sent, never re-sent"
    );
    let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(state, "sent", "recovered from the platform mirror");
    let remote: i64 = db
        .with_read(|c| {
            c.query_row(
                "SELECT hs_conversation_remote_id FROM outreach_recipients WHERE id = ?1",
                params![recipient],
                |r| r.get(0),
            )
        })
        .expect("recovered remote id");
    assert_eq!(remote, 99001, "the mirrored conversation id is recorded");
}

#[test]
fn a_stale_claimed_row_that_never_landed_is_requeued_and_sent_once() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| {
        let (campaign, recipient) = seed_campaign(c);
        c.execute(
            "UPDATE outreach_recipients SET state = 'sending',
                 updated_at = datetime('now', '-11 minutes')
               WHERE id = ?1",
            params![recipient],
        )
        .expect("stale claim");
        (campaign, recipient)
    });
    let provider = ScriptedProvider::succeeding();
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
    let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

    assert_eq!(
        provider.sends(),
        1,
        "the genuinely undelivered row is requeued and sent exactly once: {summary}"
    );
    let (state, attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(state, "sent");
    assert_eq!(attempts, 1, "the recovery send spends an attempt");
}

#[test]
fn a_reconciled_unknown_returns_to_the_queue_under_the_cap() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| {
        let (campaign, recipient) = seed_campaign(c);
        c.execute(
            "UPDATE outreach_recipients SET state = 'unknown', attempts = 0
               WHERE id = ?1",
            params![recipient],
        )
        .expect("unknown");
        (campaign, recipient)
    });
    let provider = ScriptedProvider::succeeding();
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
    rt().block_on(async { send_batch(&db, &sender, campaign).await });

    let (state, _attempts, _error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(
        state, "sent",
        "a provably-undelivered unknown returns to the queue and is sent (was 'failed')"
    );
}

#[test]
fn a_reconciled_unknown_with_exhausted_attempts_fails_honestly() {
    let db = db();
    let (campaign, recipient) = db.with_write(|c| {
        let (campaign, recipient) = seed_campaign(c);
        c.execute(
            "UPDATE outreach_recipients SET state = 'unknown', attempts = 3
               WHERE id = ?1",
            params![recipient],
        )
        .expect("unknown exhausted");
        (campaign, recipient)
    });
    let provider = ScriptedProvider::succeeding();
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = provider.clone();
    rt().block_on(async { send_batch(&db, &sender, campaign).await });

    assert_eq!(provider.sends(), 0, "attempts exhausted: no blind re-send");
    let (state, attempts, error) = db.with_read(|c| recipient_row(c, recipient));
    assert_eq!(
        state, "failed",
        "the terminal state is honest, not a lingering 'unknown'"
    );
    assert_eq!(attempts, 3);
    assert!(
        error.as_deref().unwrap_or("").contains("exhausted"),
        "the reason is recorded: {error:?}"
    );
}

#[test]
fn the_staleness_clock_column_is_ensured_by_the_batch_run() {
    let db = db();
    let (campaign, _recipient) = db.with_write(|c| seed_campaign(c));
    let provider: Arc<dyn crate::helpscout::HelpScoutProvider> = ScriptedProvider::succeeding();
    rt().block_on(async { send_batch(&db, &provider, campaign).await });

    let has_column: bool = db
        .with_read(|c| {
            c.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('outreach_recipients')
                  WHERE name = 'updated_at'",
                [],
                |r| r.get(0),
            )
        })
        .expect("pragma");
    assert!(has_column, "the claim/staleness clock column exists");
}

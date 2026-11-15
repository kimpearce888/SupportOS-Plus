use super::*;
use crate::db::DbHandle;
use crate::error::Error;
use crate::helpscout::{
    ConversationPatch, ConversationQuery, CreateThreadInput, CustomerQuery, HelpScoutProvider,
    HsBeaconChat, HsDocArticle, HsMailbox, HsRating, HsTag, HsTeam, HsUser, Page, ThreadCreated,
};
use std::sync::Mutex;

struct RecordingSender {
    created: Mutex<Vec<crate::helpscout::CreateConversationInput>>,
}

#[async_trait::async_trait]
impl HelpScoutProvider for RecordingSender {
    fn kind(&self) -> &'static str {
        "recording"
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
        self.created
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(input);
        Ok(crate::helpscout::ConversationCreated {
            conversation_id: 777_001,
            number: 4242,
            thread_id: 9001,
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
         VALUES ('winter check', 'Quick question', 'Hello!', ?1, '[]', 'queued')",
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
    (mailbox_local, campaign)
}

#[test]
fn campaign_sends_pass_the_remote_mailbox_id() {
    let db = db();
    let (mailbox_local, campaign) = db.with_write(|c| seed_campaign(c));
    let recorder = Arc::new(RecordingSender {
        created: Mutex::new(Vec::new()),
    });
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = recorder.clone();
    let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

    let created = recorder.created.lock().unwrap_or_else(|p| p.into_inner());
    assert_eq!(
        created.len(),
        1,
        "one platform conversation per recipient: {summary}"
    );
    assert_ne!(
        created[0].mailbox_id, mailbox_local,
        "the LOCAL mirror row id must never reach the platform"
    );
    assert_eq!(
        created[0].mailbox_id, 2024,
        "the platform must receive the REMOTE mailbox id"
    );
    assert_eq!(created[0].customer_id, 5501);

    let state: String = db
        .with_read(|c| {
            c.query_row(
                "SELECT state FROM outreach_recipients WHERE campaign_id = ?1",
                params![campaign],
                |r| r.get(0),
            )
        })
        .expect("recipient row");
    assert_eq!(state, "sent", "the send is recorded as delivered");
}

#[test]
fn an_unsynced_mailbox_fails_the_batch_honestly() {
    let db = db();
    let campaign = db.with_write(|c| {
        c.execute("PRAGMA foreign_keys = OFF", []).expect("fk off");
        c.execute(
            "INSERT INTO outreach_campaigns
                 (name, subject, body, mailbox_local_id, tags, status)
             VALUES ('broken', 'Hi', 'Body', 999, '[]', 'queued')",
            [],
        )
        .expect("campaign");
        c.execute("PRAGMA foreign_keys = ON", []).expect("fk on");
        let campaign = c.last_insert_rowid();
        c.execute(
            "INSERT INTO outreach_recipients
                 (campaign_id, customer_local_id, customer_remote_id, email, snapshot, state)
             VALUES (?1, 1, 5501, 'person@example.com', '{}', 'selected')",
            params![campaign],
        )
        .expect("recipient");
        campaign
    });
    let recorder = Arc::new(RecordingSender {
        created: Mutex::new(Vec::new()),
    });
    let sender: Arc<dyn crate::helpscout::HelpScoutProvider> = recorder.clone();
    let summary = rt().block_on(async { send_batch(&db, &sender, campaign).await });

    let created = recorder.created.lock().unwrap_or_else(|p| p.into_inner());
    assert!(
        created.is_empty(),
        "nothing may be sent with an unsynced mailbox: {created:?}"
    );
    let (state, error): (String, Option<String>) = db
        .with_read(|c| {
            c.query_row(
                "SELECT state, last_error FROM outreach_recipients WHERE campaign_id = ?1",
                params![campaign],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
        })
        .expect("recipient row");
    assert_eq!(state, "failed", "permanent failure, no silent retry loop");
    assert!(
        error.as_deref().unwrap_or("").contains("not synced"),
        "the error says what happened: {error:?} / {summary}"
    );
}

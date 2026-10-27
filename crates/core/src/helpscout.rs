use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HsUser {
    pub remote_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub user_type: String,
    pub timezone: Option<String>,
    pub photo_url: Option<String>,
    pub initials: Option<String>,
    pub mention: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub alternate_emails: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsTeam {
    pub remote_id: i64,
    pub name: String,
    pub member_user_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsMailbox {
    pub remote_id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub email: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsTag {
    pub remote_id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub color: Option<String>,
    pub ticket_count: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomer {
    pub remote_id: i64,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub organization: Option<String>,
    pub job_title: Option<String>,
    pub phone: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub photo_url: Option<String>,
    #[serde(default)]
    pub organization_id: Option<i64>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub age: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub emails: Vec<HsCustomerEmail>,
    #[serde(default)]
    pub phones: Vec<HsCustomerPhone>,
    #[serde(default)]
    pub chats: Vec<HsCustomerChat>,
    #[serde(default)]
    pub websites: Vec<HsCustomerWebsite>,
    #[serde(default)]
    pub social_profiles: Vec<HsCustomerSocialProfile>,
    #[serde(default)]
    pub address: Option<HsCustomerAddress>,
    #[serde(default)]
    pub properties: Vec<HsCustomerPropertyValue>,
}

impl HsCustomer {
    /// An all-empty customer shell — every field unset. Used to assemble
    /// channel-only results (see `get_customer_channels`) without repeating
    /// twenty `None`s.
    #[must_use]
    pub fn placeholder() -> Self {
        Self {
            remote_id: 0,
            first_name: None,
            last_name: None,
            email: None,
            organization: None,
            job_title: None,
            phone: None,
            created_at: None,
            updated_at: None,
            photo_url: None,
            organization_id: None,
            background: None,
            age: None,
            gender: None,
            location: None,
            emails: Vec::new(),
            phones: Vec::new(),
            chats: Vec::new(),
            websites: Vec::new(),
            social_profiles: Vec::new(),
            address: None,
            properties: Vec::new(),
        }
    }

    /// True when no contact channels are embedded in the payload. The real
    /// Help Scout API NEVER embeds them (they live behind per-customer
    /// sub-resources — see `HelpScoutProvider::get_customer_channels`), so
    /// an all-empty result on a freshly fetched customer means the channels
    /// must be fetched separately, not that the customer has none.
    #[must_use]
    pub fn channels_all_empty(&self) -> bool {
        self.emails.is_empty()
            && self.phones.is_empty()
            && self.chats.is_empty()
            && self.websites.is_empty()
            && self.social_profiles.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerEmail {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerPhone {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

/// Live-API audit fix (C4): Help Scout stores customer chat handles behind
/// the `/v2/customers/{id}/chats` sub-resource; they never appear embedded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerChat {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerWebsite {
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerSocialProfile {
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerAddress {
    #[serde(default)]
    pub line1: Option<String>,
    #[serde(default)]
    pub line2: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(rename = "postalCode", default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsCustomerPropertyValue {
    #[serde(rename = "definitionRemoteId", default)]
    pub definition_remote_id: Option<i64>,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HsConversation {
    pub remote_id: i64,
    pub number: i64,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_via: Option<String>,
    pub subject: Option<String>,
    pub preview: Option<String>,
    pub status: String,
    #[serde(default)]
    pub state: Option<String>,
    pub mailbox_id: i64,
    pub assignee_id: Option<i64>,
    #[serde(default)]
    pub assignee_type: Option<String>,
    #[serde(default)]
    pub assigned_team_id: Option<i64>,
    pub customer_id: i64,
    pub priority: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closed_at: Option<String>,
    #[serde(default)]
    pub snoozed_until: Option<String>,
    #[serde(default)]
    pub thread_count: i64,
    #[serde(default)]
    pub merged_into: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "fields")]
    pub custom_fields: Vec<HsCustomFieldValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HsCustomFieldValue {
    #[serde(rename = "fieldId")]
    pub field_id: i64,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub system_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsBeaconChat {
    pub remote_id: i64,
    pub customer_id: i64,
    pub mailbox_id: i64,
    pub status: String,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// P2: one participant of a live Beacon chat (customer or assignee) —
/// the Chat API carries people as compact objects with split names.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsChatParticipant {
    pub remote_id: i64,
    #[serde(default)]
    pub first: Option<String>,
    #[serde(default)]
    pub last: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

impl HsChatParticipant {
    /// Display name: "First Last", falling back to the email, then the id.
    #[must_use]
    pub fn name(&self) -> String {
        let full = [self.first.as_deref(), self.last.as_deref()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let trimmed = full.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        self.email
            .clone()
            .unwrap_or_else(|| format!("#{}", self.remote_id))
    }
}

/// P2: one live Beacon chat, as the Chat API and the five
/// `beacon.chat.*` webhook events carry it. Chat ids are **UUID
/// strings**, unlike every numeric id the Inbox API uses, and a chat is
/// not a conversation: Help Scout creates the conversation when the chat
/// *ends*, and `GET /v2/chats/{id}` answers 404 from then on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsChat {
    /// The platform's own UUID string (never ours).
    pub id: String,
    #[serde(default)]
    pub beacon_id: Option<String>,
    #[serde(default)]
    pub mailbox_id: i64,
    #[serde(default)]
    pub customer: Option<HsChatParticipant>,
    #[serde(default)]
    pub assignee: Option<HsChatParticipant>,
    /// `open` or `ended`.
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    /// The conversation the chat collapsed into once ended (parsed from
    /// `conversationId` / `conversation.id` when the platform sends it).
    #[serde(default)]
    pub conversation_remote_id: Option<i64>,
    /// The embedded event/message list: `GET /v2/chats/{id}` embeds
    /// events, webhook payloads carry the newest state. Kept raw — the
    /// mirror ingests them defensively and stores what is there.
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
}

/// P2: one `beacon.chat.*` webhook event, parsed: the event name plus
/// the chat object it carries (the five events are the platform's only
/// push path — there is no chat list endpoint).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsChatEvent {
    pub event_type: String,
    pub chat: HsChat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsDocArticle {
    /// Audit fix (C2, live): Docs API ids are 24-char hex ObjectIds
    /// ("6adf725b922c92c2dd6ce7d7") — the old i64 parsing turned every id
    /// into 0, so category/article URLs hit `/0/` and the API answered
    /// 400 "ECMA 262 regex does not match 0".
    pub remote_id: String,
    pub collection_id: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub number: Option<i64>,
    pub slug: Option<String>,
    pub name: String,
    #[serde(default)]
    pub status: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub views: Option<i64>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsRating {
    pub remote_id: i64,
    pub conversation_id: Option<i64>,
    pub thread_id: Option<i64>,
    pub rating: Option<String>,
    pub comment: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_name: Option<String>,
    pub user_id: Option<i64>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsReportRow {
    pub key: String,
    pub name: String,
    pub source: String,
    pub data: serde_json::Value,
}

/// K3: the article draft the review queue publishes through the Docs API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocArticleDraftInput {
    /// The Docs API collection the article belongs to (24-char hex ObjectId).
    pub collection_id: String,
    pub name: String,
    /// Markdown body (published as the article `text`).
    pub text: String,
    /// When set, the linked article is updated (PUT /v1/articles/{id}/drafts);
    /// when absent a new `notpublished` article is created (POST /v1/articles).
    pub remote_id: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsFolder {
    pub remote_id: i64,
    pub mailbox_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub user_id: Option<i64>,
    pub total_count: i64,
    pub active_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsField {
    pub remote_id: i64,
    pub mailbox_id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub system_type: Option<String>,
    pub required: bool,
    pub sort_order: i64,
    pub options: Vec<HsFieldOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsFieldOption {
    pub id: i64,
    pub order: i64,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsSavedReply {
    pub remote_id: i64,
    pub name: String,
    pub preview: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsWorkflow {
    pub remote_id: i64,
    pub mailbox_id: Option<i64>,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsWebhookConfig {
    pub remote_id: i64,
    pub url: String,
    pub events: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsPropertyDef {
    pub remote_id: i64,
    pub name: String,
    pub slug: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsOrganization {
    pub remote_id: i64,
    pub name: String,
    pub domains: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HsThreadRecipient {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HsThreadAttachment {
    #[serde(default)]
    pub remote_id: i64,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HsThread {
    pub remote_id: i64,
    pub conversation_id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: Option<String>,
    pub state: Option<String>,
    pub body: Option<String>,
    pub created_by_customer_id: Option<i64>,
    pub created_by_user_id: Option<i64>,
    pub assigned_to_id: Option<i64>,
    pub created_at: Option<String>,
    #[serde(default)]
    pub to: Vec<HsThreadRecipient>,
    #[serde(default)]
    pub cc: Vec<HsThreadRecipient>,
    #[serde(default)]
    pub attachments: Vec<HsThreadAttachment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_for: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsUserStatus {
    pub user_id: i64,
    pub email_status: Option<String>,
    pub email_updated_at: Option<String>,
    pub chat_status: Option<String>,
    pub mailbox_statuses: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsDocCollection {
    /// Audit fix (C2, live): 24-char hex ObjectId, see HsDocArticle.
    pub remote_id: String,
    pub slug: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub article_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsDocCategory {
    /// Audit fix (C2, live): 24-char hex ObjectId, see HsDocArticle.
    pub remote_id: String,
    pub collection_id: String,
    pub slug: Option<String>,
    pub name: String,
    #[serde(default)]
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationQuery {
    pub mailbox_id: Option<i64>,
    pub status: Option<String>,
    pub modified_since: Option<String>,
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct CustomerQuery {
    pub mailbox_id: Option<i64>,
    pub modified_since: Option<String>,
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
}

#[async_trait::async_trait]
pub trait HelpScoutProvider: Send + Sync {
    fn kind(&self) -> &'static str;

    async fn get_me(&self) -> Result<HsUser>;

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>>;

    async fn list_users(&self) -> Result<Vec<HsUser>>;

    async fn list_teams(&self) -> Result<Vec<HsTeam>>;

    async fn list_tags(&self) -> Result<Vec<HsTag>>;

    async fn list_conversations(&self, query: &ConversationQuery) -> Result<Page<HsConversation>>;

    async fn list_customers(&self, query: &CustomerQuery) -> Result<Page<HsCustomer>>;

    async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>>;

    async fn list_docs(&self) -> Result<Vec<HsDocArticle>>;

    async fn list_ratings(&self) -> Result<Vec<HsRating>>;

    async fn get_rating(&self, _rating_id: i64) -> Result<Option<HsRating>> {
        Ok(None)
    }

    /// K5: fetch one Reports API endpoint (registry key from
    /// `reports_api::REPORTS`) for a date range. `Ok(None)` means the
    /// connection has no report data to offer (not connected, or the
    /// endpoint returned nothing); a plan/permission wall surfaces as the
    /// provider's API error so the caller can classify it (403 = plan
    /// blocked) instead of silently rendering zeros.
    async fn get_report(
        &self,
        _key: &str,
        _start: &str,
        _end: &str,
    ) -> Result<Option<HsReportRow>> {
        Ok(None)
    }

    async fn list_folders(&self, _mailbox_id: i64) -> Result<Vec<HsFolder>> {
        Ok(Vec::new())
    }

    async fn list_inbox_fields(&self, _mailbox_id: i64) -> Result<Vec<HsField>> {
        Ok(Vec::new())
    }

    async fn list_saved_replies(&self, _mailbox_id: i64) -> Result<Vec<HsSavedReply>> {
        Ok(Vec::new())
    }

    /// P3: create a saved reply on the platform
    /// (POST /v2/mailboxes/{id}/saved-replies — the platform's required
    /// `name` is enforced there, `text` is HTML). The default rejects: a
    /// connection without a Help Scout link never pretends to write.
    async fn create_saved_reply(&self, _mailbox_id: i64, _name: &str, _text: &str) -> Result<i64> {
        Err(crate::error::Error::Other(
            "saved reply writes require the real provider".into(),
        ))
    }

    /// P3: update a saved reply (PUT /v2/mailboxes/{id}/saved-replies/{id}).
    async fn update_saved_reply(
        &self,
        _mailbox_id: i64,
        _saved_reply_id: i64,
        _name: &str,
        _text: &str,
    ) -> Result<()> {
        Err(crate::error::Error::Other(
            "saved reply writes require the real provider".into(),
        ))
    }

    /// P3: soft-delete a saved reply
    /// (DELETE /v2/mailboxes/{id}/saved-replies/{id}).
    async fn delete_saved_reply(&self, _mailbox_id: i64, _saved_reply_id: i64) -> Result<()> {
        Err(crate::error::Error::Other(
            "saved reply writes require the real provider".into(),
        ))
    }

    async fn list_workflows(&self) -> Result<Vec<HsWorkflow>> {
        Ok(Vec::new())
    }

    async fn list_webhooks(&self) -> Result<Vec<HsWebhookConfig>> {
        Ok(Vec::new())
    }

    async fn create_webhook(
        &self,
        _url: &str,
        _events: &[String],
        _secret: &str,
        _label: &str,
    ) -> Result<i64> {
        Err(crate::error::Error::Other(
            "webhook registration requires the real provider".into(),
        ))
    }

    async fn delete_webhook(&self, _remote_id: i64) -> Result<bool> {
        Err(crate::error::Error::Other(
            "webhook deletion requires the real provider".into(),
        ))
    }

    /// P2: fetch one live chat by its UUID (`GET /v2/chats/{id}` — embeds
    /// the event list). `Ok(None)` is the honest answer for a chat that
    /// has collapsed into a conversation (the platform answers 404 from
    /// then on); the default covers every connection without the Chat
    /// API.
    async fn get_chat(&self, _chat_id: &str) -> Result<Option<HsChat>> {
        Ok(None)
    }

    /// P2: send a live chat message
    /// (PUT /v2/chats/{id}/messages/{uuid}). The message id is ours, so
    /// a retry after a timeout can never double-send — the platform's
    /// 422 "id value is already in use" answer is treated as delivered
    /// (the real provider does exactly that). Only the assigned agent
    /// may message: an unassigned chat is accepted by the send itself, a
    /// chat held by someone else answers 400 with its reason.
    async fn send_chat_message(
        &self,
        _chat_id: &str,
        _message_id: &str,
        _text: &str,
    ) -> Result<()> {
        Err(crate::error::Error::Other(
            "live chat sends require the real provider".into(),
        ))
    }

    async fn list_customer_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        Ok(Vec::new())
    }

    async fn list_organization_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        Ok(Vec::new())
    }

    async fn list_organizations(&self) -> Result<Vec<HsOrganization>> {
        Ok(Vec::new())
    }

    async fn get_conversation(&self, _conversation_id: i64) -> Result<Option<HsConversation>> {
        Ok(None)
    }

    async fn list_threads(&self, _conversation_id: i64) -> Result<Vec<HsThread>> {
        Ok(Vec::new())
    }

    async fn get_customer(&self, _customer_id: i64) -> Result<Option<HsCustomer>> {
        Ok(None)
    }

    /// Live-API audit fix (C4): Help Scout NEVER embeds customer contact
    /// channels in the customer payload — emails/phones/chats/websites/social
    /// profiles live behind per-customer sub-resources
    /// (`GET /v2/customers/{id}/emails|phones|chats|websites|social-profiles`).
    /// Returns the fetched channels; the default (mock providers without the
    /// endpoints) reports none so callers keep their existing behavior.
    async fn get_customer_channels(&self, _customer_id: i64) -> Result<Option<HsCustomer>> {
        Ok(None)
    }

    async fn get_user_status(&self, _user_id: i64) -> Result<Option<HsUserStatus>> {
        Ok(None)
    }

    async fn list_system_users(&self) -> Result<Vec<HsUser>> {
        Ok(Vec::new())
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        Ok(Vec::new())
    }

    async fn list_doc_categories(&self, _collection_id: String) -> Result<Vec<HsDocCategory>> {
        Ok(Vec::new())
    }

    async fn list_doc_articles(&self, _collection_id: String) -> Result<Vec<HsDocArticle>> {
        Ok(Vec::new())
    }

    /// K3 verify + targeted re-sync: one full article by remote id. `None`
    /// when the article is unreachable (the caller decides whether that is
    /// an error); the default covers every connection without Docs write
    /// access.
    async fn get_doc_article(&self, _remote_id: String) -> Result<Option<HsDocArticle>> {
        Ok(None)
    }

    /// K3: upsert the draft into the Docs API — create a new `notpublished`
    /// article (POST /v1/articles?reload=true) or update the linked
    /// article's draft copy (PUT /v1/articles/{id}/drafts). Returns the
    /// remote article id. The default rejects: a connection without the
    /// Docs API never pretends to publish.
    async fn upsert_doc_article_draft(&self, _input: &DocArticleDraftInput) -> Result<String> {
        Err(Error::Config(
            "Docs API publishing is unavailable on this connection - no Docs API key is \
             configured."
                .into(),
        ))
    }

    /// K3: publish the article with its final text (PUT /v1/articles/{id}).
    async fn publish_doc_article(&self, _remote_id: String, _name: &str, _text: &str) -> Result<()> {
        Err(Error::Config(
            "Docs API publishing is unavailable on this connection - no Docs API key is \
             configured."
                .into(),
        ))
    }

    async fn create_reply_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated>;

    async fn create_note_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated>;

    async fn update_conversation(
        &self,
        conversation_id: i64,
        patch: ConversationPatch,
    ) -> Result<bool>;

    async fn create_conversation(
        &self,
        _input: CreateConversationInput,
    ) -> Result<ConversationCreated> {
        Err(crate::error::Error::Other(
            "create_conversation is not supported by this provider".into(),
        ))
    }

    fn submit_rating(
        &self,
        _conversation_remote_id: i64,
        _rating: &str,
        _comments: Option<&str>,
    ) -> Option<HsRating> {
        None
    }

    fn customer_replies(&self, _conversation_remote_id: i64, _text: &str) {}

    fn reset(&self) {}

    /// Whether a Docs API key is configured for Help Center sync
    /// (v1.4.0 R5, D15): lets the sync engine surface "Help Center
    /// sync is idle: no Docs API key" to the UI instead of silently
    /// mirroring an empty Help Center. The real provider overrides
    /// with the actual key state.
    fn docs_configured(&self) -> bool {
        true
    }

    async fn update_tags(&self, _conversation_id: i64, _tags: Vec<String>) -> Result<bool> {
        Err(crate::error::Error::Other(
            "update_tags is not supported by this provider".into(),
        ))
    }

    async fn update_custom_fields(
        &self,
        _conversation_id: i64,
        _fields: Vec<(i64, Option<String>)>,
    ) -> Result<bool> {
        Err(crate::error::Error::Other(
            "update_custom_fields is not supported by this provider".into(),
        ))
    }

    async fn snooze_conversation(
        &self,
        _conversation_id: i64,
        _snoozed_until: String,
        _unsnooze_on_customer_reply: bool,
    ) -> Result<bool> {
        Err(crate::error::Error::Other(
            "snooze_conversation is not supported by this provider".into(),
        ))
    }

    async fn unsnooze_conversation(&self, _conversation_id: i64) -> Result<bool> {
        Err(crate::error::Error::Other(
            "unsnooze_conversation is not supported by this provider".into(),
        ))
    }

    async fn schedule_thread(
        &self,
        _conversation_id: i64,
        _thread_id: i64,
        _scheduled_for: String,
        _unschedule_on_customer_reply: bool,
    ) -> Result<bool> {
        Err(crate::error::Error::Other(
            "schedule_thread is not supported by this provider".into(),
        ))
    }

    async fn publish_scheduled_thread(
        &self,
        _conversation_id: i64,
        _thread_id: i64,
    ) -> Result<bool> {
        Err(crate::error::Error::Other(
            "publish_scheduled_thread is not supported by this provider".into(),
        ))
    }

    async fn delete_thread_schedule(&self, _conversation_id: i64, _thread_id: i64) -> Result<bool> {
        Err(crate::error::Error::Other(
            "delete_thread_schedule is not supported by this provider".into(),
        ))
    }

    async fn run_workflow(&self, _workflow_id: i64, _conversation_id: i64) -> Result<bool> {
        Err(crate::error::Error::Other(
            "run_workflow is not supported by this provider".into(),
        ))
    }

    async fn get_attachment_data(
        &self,
        _conversation_id: i64,
        _thread_id: i64,
        _attachment_id: i64,
    ) -> Result<Option<AttachmentData>> {
        Ok(None)
    }

    async fn get_routing_configuration(
        &self,
        _mailbox_id: i64,
    ) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }

    async fn ping(&self) -> Result<bool> {
        Err(crate::error::Error::Other(
            "ping is not supported by this provider".into(),
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentData {
    pub data: Vec<u8>,
    pub mime_type: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateThreadInput {
    pub conversation_id: i64,
    pub text: String,
    pub draft: bool,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub status_after: Option<String>,
    pub assign_to: Option<i64>,
    /// Remote customer id — the POST /reply body requires a `customer`
    /// object (Help Scout rejects the request without it).
    pub customer_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadCreated {
    pub thread_id: i64,
    pub conversation_id: i64,
}

#[derive(Debug, Clone)]
pub struct CreateConversationInput {
    pub mailbox_id: i64,
    pub customer_id: i64,
    pub subject: String,
    pub body: String,
    pub tags: Vec<String>,
    pub status: Option<String>,
    /// Live-API audit fix (C1): the real POST /v2/conversations REQUIRES a
    /// top-level `type` (the channel: email|chat|phone) — it answers
    /// `400 {"path":"type","message":"must not be null"}` without one.
    /// `None` means the default channel, email.
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationCreated {
    pub conversation_id: i64,
    pub number: i64,
    pub thread_id: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationPatch {
    pub subject: Option<String>,
    pub status: Option<String>,
    pub mailbox_id: Option<i64>,
    pub assign_to: Option<Option<i64>>,
}

use std::sync::Mutex;

pub struct EmptyHelpScoutProvider {
    world: Mutex<InertWorld>,
}

impl Default for EmptyHelpScoutProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
pub struct InertWorld {
    pub me: HsUser,
    pub mailboxes: Vec<HsMailbox>,
    pub users: Vec<HsUser>,
    pub system_users: Vec<HsUser>,
    pub teams: Vec<HsTeam>,
    pub tags: Vec<HsTag>,
    pub conversations: Vec<HsConversation>,
    pub customers: Vec<HsCustomer>,
    pub folders: Vec<HsFolder>,
    pub fields: Vec<HsField>,
    pub saved_replies: Vec<HsSavedReply>,
    pub workflows: Vec<HsWorkflow>,
    pub webhooks: Vec<HsWebhookConfig>,
    pub customer_props: Vec<HsPropertyDef>,
    pub org_props: Vec<HsPropertyDef>,
    pub organizations: Vec<HsOrganization>,
    pub threads: Vec<HsThread>,
    pub ratings: Vec<HsRating>,
    pub user_statuses: Vec<HsUserStatus>,
    pub doc_collections: Vec<HsDocCollection>,
    pub doc_categories: Vec<HsDocCategory>,
    pub doc_articles: Vec<HsDocArticle>,
}

impl EmptyHelpScoutProvider {
    /// Build the inert provider boot uses when no Help Scout
    /// credentials are configured: every read returns the empty
    /// world, and every operation that could create state is
    /// rejected (nothing is simulated).
    #[must_use]
    pub fn new() -> Self {
        Self {
            world: Mutex::new(InertWorld::default()),
        }
    }

    fn lock_world(&self) -> std::sync::MutexGuard<'_, InertWorld> {
        self.world.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait::async_trait]
impl HelpScoutProvider for EmptyHelpScoutProvider {
    fn kind(&self) -> &'static str {
        "empty"
    }

    fn docs_configured(&self) -> bool {
        // No credentials resolved: no Docs key, no Help Center sync.
        false
    }

    async fn get_me(&self) -> Result<HsUser> {
        let world = self
            .world
            .lock()
            .expect("EmptyHelpScoutProvider mutex poisoned");
        Ok(world.me.clone())
    }

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        let world = self
            .world
            .lock()
            .expect("EmptyHelpScoutProvider mutex poisoned");
        Ok(world.mailboxes.clone())
    }

    async fn list_users(&self) -> Result<Vec<HsUser>> {
        let world = self
            .world
            .lock()
            .expect("EmptyHelpScoutProvider mutex poisoned");
        Ok(world.users.clone())
    }

    async fn list_teams(&self) -> Result<Vec<HsTeam>> {
        let world = self
            .world
            .lock()
            .expect("EmptyHelpScoutProvider mutex poisoned");
        Ok(world.teams.clone())
    }

    async fn list_tags(&self) -> Result<Vec<HsTag>> {
        let world = self
            .world
            .lock()
            .expect("EmptyHelpScoutProvider mutex poisoned");
        Ok(world.tags.clone())
    }

    async fn list_conversations(&self, query: &ConversationQuery) -> Result<Page<HsConversation>> {
        let world = self.lock_world();
        let mut items: Vec<HsConversation> = world
            .conversations
            .iter()
            .filter(|c| c.merged_into.is_none())
            .filter(|c| query.mailbox_id.is_none_or(|m| c.mailbox_id == m))
            .filter(|c| {
                query.status.as_deref().is_none_or(|s| {
                    s == "all"
                        || s == c.status
                        || (s == "open" && (c.status == "active" || c.status == "pending"))
                })
            })
            .filter(|c| {
                query.modified_since.as_deref().is_none_or(|since| {
                    let modified = c.updated_at.as_deref().or(c.created_at.as_deref());
                    modified.is_some_and(|m| m >= since)
                })
            })
            .cloned()
            .collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.remote_id));
        let size = query.page_size.unwrap_or(25) as usize;
        let start = query.cursor.as_deref().and_then(decode_cursor).unwrap_or(0);
        let slice: Vec<HsConversation> = items.iter().skip(start).take(size).cloned().collect();
        let next_index = start + size;
        let next_cursor = if next_index < items.len() {
            Some(encode_cursor(next_index))
        } else {
            None
        };
        Ok(Page {
            items: slice,
            next_cursor,
        })
    }

    async fn list_customers(&self, query: &CustomerQuery) -> Result<Page<HsCustomer>> {
        let world = self.lock_world();
        let mut items: Vec<HsCustomer> = world
            .customers
            .iter()
            .filter(|c| {
                query
                    .modified_since
                    .as_deref()
                    .is_none_or(|since| c.updated_at.as_deref().is_none_or(|u| u >= since))
            })
            .cloned()
            .collect();
        items.sort_by_key(|c| c.remote_id);
        let size = query.page_size.unwrap_or(50) as usize;
        let start = query.cursor.as_deref().and_then(decode_cursor).unwrap_or(0);
        let slice: Vec<HsCustomer> = items.iter().skip(start).take(size).cloned().collect();
        let next_index = start + size;
        let next_cursor = if next_index < items.len() {
            Some(encode_cursor(next_index))
        } else {
            None
        };
        Ok(Page {
            items: slice,
            next_cursor,
        })
    }

    async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
        let world = self.lock_world();
        Ok(world
            .conversations
            .iter()
            .filter(|c| c.kind.as_deref() == Some("chat") && c.merged_into.is_none())
            .map(|c| HsBeaconChat {
                remote_id: c.remote_id,
                customer_id: c.customer_id,
                mailbox_id: c.mailbox_id,
                status: c.status.clone(),
                created_at: c.created_at.clone(),
                updated_at: c.updated_at.clone(),
            })
            .collect())
    }

    async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
        let world = self.lock_world();
        Ok(world.doc_articles.clone())
    }

    async fn list_ratings(&self) -> Result<Vec<HsRating>> {
        let world = self.lock_world();
        Ok(world.ratings.clone())
    }

    async fn get_rating(&self, rating_id: i64) -> Result<Option<HsRating>> {
        let world = self.lock_world();
        Ok(world
            .ratings
            .iter()
            .find(|r| r.remote_id == rating_id)
            .cloned())
    }

    async fn list_folders(&self, mailbox_id: i64) -> Result<Vec<HsFolder>> {
        let world = self.lock_world();
        Ok(world
            .folders
            .iter()
            .filter(|f| f.mailbox_id == mailbox_id)
            .cloned()
            .collect())
    }

    async fn list_inbox_fields(&self, mailbox_id: i64) -> Result<Vec<HsField>> {
        let world = self.lock_world();
        Ok(world
            .fields
            .iter()
            .filter(|f| f.mailbox_id == mailbox_id)
            .cloned()
            .collect())
    }

    async fn list_saved_replies(&self, _mailbox_id: i64) -> Result<Vec<HsSavedReply>> {
        let world = self.lock_world();
        Ok(world.saved_replies.clone())
    }

    async fn list_workflows(&self) -> Result<Vec<HsWorkflow>> {
        let world = self.lock_world();
        Ok(world.workflows.clone())
    }

    async fn list_webhooks(&self) -> Result<Vec<HsWebhookConfig>> {
        let world = self.lock_world();
        Ok(world.webhooks.clone())
    }

    async fn create_webhook(
        &self,
        _url: &str,
        _events: &[String],
        _secret: &str,
        _label: &str,
    ) -> Result<i64> {
        Err(no_account())
    }

    async fn delete_webhook(&self, _remote_id: i64) -> Result<bool> {
        Err(no_account())
    }

    async fn list_customer_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        let world = self.lock_world();
        Ok(world.customer_props.clone())
    }

    async fn list_organization_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        let world = self.lock_world();
        Ok(world.org_props.clone())
    }

    async fn list_organizations(&self) -> Result<Vec<HsOrganization>> {
        let world = self.lock_world();
        Ok(world.organizations.clone())
    }

    async fn get_conversation(&self, conversation_id: i64) -> Result<Option<HsConversation>> {
        let world = self.lock_world();
        let conv = world
            .conversations
            .iter()
            .find(|c| c.remote_id == conversation_id)
            .cloned();
        match conv {
            Some(c) if c.merged_into.is_some() => {
                Err(crate::helpscout_real::HsApiError {
                    status_code: 301,
                    message: format!(
                        "Conversation merged into {}",
                        c.merged_into.unwrap_or_default()
                    ),
                    friendly: "This conversation was merged into another conversation in Help Scout. Open the target conversation instead.".into(),
                    retryable: false,
                }
                .into())
            }
            other => Ok(other),
        }
    }

    async fn list_threads(&self, conversation_id: i64) -> Result<Vec<HsThread>> {
        let world = self.lock_world();
        let mut threads: Vec<HsThread> = world
            .threads
            .iter()
            .filter(|t| t.conversation_id == conversation_id)
            .cloned()
            .collect();
        threads.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(threads)
    }

    async fn get_customer(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        let world = self.lock_world();
        Ok(world
            .customers
            .iter()
            .find(|c| c.remote_id == customer_id)
            .cloned())
    }

    async fn get_user_status(&self, user_id: i64) -> Result<Option<HsUserStatus>> {
        let world = self.lock_world();
        Ok(world
            .user_statuses
            .iter()
            .find(|s| s.user_id == user_id)
            .cloned())
    }

    async fn list_system_users(&self) -> Result<Vec<HsUser>> {
        let world = self.lock_world();
        Ok(world.system_users.clone())
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        let world = self.lock_world();
        Ok(world.doc_collections.clone())
    }

    async fn list_doc_categories(&self, collection_id: String) -> Result<Vec<HsDocCategory>> {
        let world = self.lock_world();
        Ok(world
            .doc_categories
            .iter()
            .filter(|c| c.collection_id == collection_id)
            .cloned()
            .collect())
    }

    async fn list_doc_articles(&self, collection_id: String) -> Result<Vec<HsDocArticle>> {
        let world = self.lock_world();
        Ok(world
            .doc_articles
            .iter()
            .filter(|a| a.collection_id == collection_id)
            .cloned()
            .collect())
    }

    async fn create_reply_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let me = world.me.clone();
        let next_id = world.threads.iter().map(|t| t.remote_id).max().unwrap_or(0) + 1;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == input.conversation_id)
        else {
            return Err(not_found_remote("POST"));
        };
        let customer_email = world
            .customers
            .iter()
            .find(|c| c.remote_id == conv.customer_id)
            .and_then(|c| c.email.clone())
            .map(|email| HsThreadRecipient {
                id: Some(conv.customer_id),
                email: Some(email),
            });
        let thread = HsThread {
            remote_id: next_id,
            conversation_id: input.conversation_id,
            kind: "reply".into(),
            status: None,
            state: Some(if input.draft { "draft" } else { "published" }.into()),
            body: Some(input.text.clone()),
            created_by_customer_id: None,
            created_by_user_id: Some(me.remote_id),
            assigned_to_id: None,
            created_at: Some(now.clone()),
            to: customer_email.into_iter().collect(),
            cc: input
                .cc
                .iter()
                .map(|email| HsThreadRecipient {
                    id: None,
                    email: Some(email.clone()),
                })
                .collect(),
            ..Default::default()
        };
        world.threads.push(thread);
        conv.thread_count = world
            .threads
            .iter()
            .filter(|t| t.conversation_id == input.conversation_id)
            .count() as i64;
        conv.updated_at = Some(now);
        if !input.draft {
            conv.status = match input.status_after.as_deref() {
                // Audit item 25: `open` is the v2 status the real provider
                // accepts; this loopback world models it as v1 `active`
                // (the same status its list_conversations "open" filter
                // already matches). `waiting` was never a status in
                // either vocabulary and now maps to `active` too, so a
                // legacy stored setting cannot poison the mirror.
                Some("open" | "waiting") => "active".into(),
                Some("active" | "closed" | "pending" | "spam") => input
                    .status_after
                    .clone()
                    .unwrap_or_else(|| "active".into()),
                _ => "active".into(),
            };
            if let Some(assign_to) = input.assign_to {
                conv.assignee_id = Some(assign_to);
            }
        }
        Ok(ThreadCreated {
            thread_id: next_id,
            conversation_id: input.conversation_id,
        })
    }

    async fn create_note_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let me = world.me.clone();
        let next_id = world.threads.iter().map(|t| t.remote_id).max().unwrap_or(0) + 1;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == input.conversation_id)
        else {
            return Err(not_found_remote("POST"));
        };
        let thread = HsThread {
            remote_id: next_id,
            conversation_id: input.conversation_id,
            kind: "note".into(),
            status: None,
            state: Some("published".into()),
            body: Some(input.text.clone()),
            created_by_customer_id: None,
            created_by_user_id: Some(me.remote_id),
            assigned_to_id: None,
            created_at: Some(now.clone()),
            ..Default::default()
        };
        world.threads.push(thread);
        conv.thread_count = world
            .threads
            .iter()
            .filter(|t| t.conversation_id == input.conversation_id)
            .count() as i64;
        conv.updated_at = Some(now);
        Ok(ThreadCreated {
            thread_id: next_id,
            conversation_id: input.conversation_id,
        })
    }

    async fn update_conversation(
        &self,
        conversation_id: i64,
        patch: ConversationPatch,
    ) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("PATCH"));
        };
        if let Some(subject) = patch.subject {
            conv.subject = Some(subject);
        }
        if let Some(status) = patch.status {
            let closing = status == "closed";
            conv.status = status;
            if closing && conv.closed_at.is_none() {
                conv.closed_at = Some(
                    chrono::Utc::now()
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string(),
                );
            }
        }
        if let Some(mailbox_id) = patch.mailbox_id {
            conv.mailbox_id = mailbox_id;
        }
        if let Some(assign_to) = patch.assign_to {
            conv.assignee_id = assign_to;
        }
        conv.updated_at =
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        Ok(true)
    }

    async fn create_conversation(
        &self,
        _input: CreateConversationInput,
    ) -> Result<ConversationCreated> {
        Err(no_account())
    }

    async fn update_tags(&self, conversation_id: i64, tags: Vec<String>) -> Result<bool> {
        let mut guard = self.lock_world();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let world: &mut InertWorld = &mut guard;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("PUT"));
        };
        conv.tags = tags;
        conv.updated_at = Some(now);
        Ok(true)
    }

    async fn update_custom_fields(
        &self,
        conversation_id: i64,
        fields: Vec<(i64, Option<String>)>,
    ) -> Result<bool> {
        let mut guard = self.lock_world();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let world: &mut InertWorld = &mut guard;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("PUT"));
        };
        let preserved_system: Vec<crate::helpscout::HsCustomFieldValue> = conv
            .custom_fields
            .iter()
            .filter(|f| f.system_type.is_some() && !fields.iter().any(|(id, _)| *id == f.field_id))
            .cloned()
            .collect();
        conv.custom_fields = preserved_system;
        for (id, value) in fields {
            let def = world.fields.iter().find(|d| d.remote_id == id);
            let value_str = value.unwrap_or_default();
            let text = def
                .and_then(|d| {
                    d.options
                        .iter()
                        .find(|o| o.id.to_string() == value_str)
                        .map(|o| o.label.clone())
                })
                .unwrap_or_else(|| value_str.clone());
            conv.custom_fields
                .push(crate::helpscout::HsCustomFieldValue {
                    field_id: id,
                    value: Some(value_str),
                    text: Some(text),
                    system_type: def.and_then(|d| d.system_type.clone()),
                });
        }
        conv.updated_at = Some(now);
        Ok(true)
    }

    async fn snooze_conversation(
        &self,
        conversation_id: i64,
        snoozed_until: String,
        _unsnooze_on_customer_reply: bool,
    ) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("PUT"));
        };
        conv.snoozed_until = Some(snoozed_until);
        Ok(true)
    }

    async fn unsnooze_conversation(&self, conversation_id: i64) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("DELETE"));
        };
        conv.snoozed_until = None;
        Ok(true)
    }

    async fn schedule_thread(
        &self,
        conversation_id: i64,
        thread_id: i64,
        scheduled_for: String,
        _unschedule_on_customer_reply: bool,
    ) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(t) = world.threads.iter_mut().find(|t| t.remote_id == thread_id) else {
            return Err(not_found_remote("PUT"));
        };
        t.scheduled_for = Some(scheduled_for);
        t.state = Some("scheduled".into());
        let _ = conversation_id;
        Ok(true)
    }

    async fn publish_scheduled_thread(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(t) = world.threads.iter_mut().find(|t| t.remote_id == thread_id) else {
            return Err(not_found_remote("PATCH"));
        };
        t.state = Some("published".into());
        let _ = conversation_id;
        Ok(true)
    }

    async fn delete_thread_schedule(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let Some(t) = world.threads.iter_mut().find(|t| t.remote_id == thread_id) else {
            return Err(not_found_remote("DELETE"));
        };
        t.state = Some("draft".into());
        t.scheduled_for = None;
        let _ = conversation_id;
        Ok(true)
    }

    async fn run_workflow(&self, workflow_id: i64, conversation_id: i64) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut InertWorld = &mut guard;
        let wf = world.workflows.iter().find(|w| w.remote_id == workflow_id);
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_id)
        else {
            return Err(not_found_remote("POST"));
        };
        if wf.is_none() {
            return Err(not_found_remote("POST"));
        }
        if wf.unwrap().name.contains("Tier 1") {
            conv.assignee_id = Some(1001);
            conv.assignee_type = Some("user".into());
        }
        Ok(true)
    }

    async fn get_attachment_data(
        &self,
        conversation_id: i64,
        _thread_id: i64,
        attachment_id: i64,
    ) -> Result<Option<crate::helpscout::AttachmentData>> {
        let guard = self.lock_world();
        let world: &InertWorld = &guard;
        let t = world.threads.iter().find(|x| {
            x.conversation_id == conversation_id
                && x.attachments.iter().any(|a| a.remote_id == attachment_id)
        });
        let att = t.and_then(|t| t.attachments.iter().find(|a| a.remote_id == attachment_id));
        let Some(att) = att else {
            return Ok(None);
        };
        // R7 (D14) honesty: the inert world carries attachment *metadata*
        // but there are no bytes behind it. Earlier revisions fabricated a
        // "Simulated attachment content..." body here, which could be
        // persisted to disk as if it were a real download. The empty
        // provider refuses instead; real bytes come from the real provider.
        let _ = att;
        Err(crate::error::Error::Config(
            "the empty provider has no attachment bytes to download; connect Help Scout \
             (Settings -> Help Scout) and re-run the download"
                .into(),
        ))
    }

    async fn get_routing_configuration(
        &self,
        mailbox_id: i64,
    ) -> Result<Option<serde_json::Value>> {
        let guard = self.lock_world();
        let world: &InertWorld = &guard;
        Ok(Some(serde_json::json!({
            "state": "enabled",
            "assignmentLimit": 10,
            "assignmentMethod": "round_robin",
            "userIds": world.users.iter().map(|u| u.remote_id).collect::<Vec<_>>(),
            "rotation": [],
            "mailboxId": mailbox_id,
        })))
    }

    async fn ping(&self) -> Result<bool> {
        Ok(true)
    }
}

fn encode_cursor(start: usize) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(start.to_string())
}

fn decode_cursor(cursor: &str) -> Option<usize> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(cursor)
        .ok()?;
    String::from_utf8(bytes).ok()?.parse().ok()
}

fn no_account() -> crate::error::Error {
    crate::error::Error::Other(
        "No Help Scout account is connected. Add your credentials in \
         Settings, then restart SupportOS+."
            .into(),
    )
}

fn not_found_remote(method: &str) -> crate::error::Error {
    let friendly = crate::helpscout_real::friendly_error(404, "", method);
    crate::helpscout_real::HsApiError {
        status_code: 404,
        message: "Conversation not found".into(),
        friendly,
        retryable: false,
    }
    .into()
}

#[cfg(test)]
mod batch20_audit25_tests {
    use super::*;

    /// Seed one conversation into the inert world and return the provider.
    fn provider_with_conversation() -> EmptyHelpScoutProvider {
        let provider = EmptyHelpScoutProvider::new();
        {
            let mut world = provider.lock_world();
            world.conversations.push(HsConversation {
                remote_id: 101,
                number: 779,
                subject: Some("Status vocabulary".into()),
                status: "active".into(),
                ..Default::default()
            });
        }
        provider
    }

    fn reply_status_after(provider: &EmptyHelpScoutProvider, status: &str) -> String {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        rt.block_on(provider.create_reply_thread(CreateThreadInput {
            conversation_id: 101,
            text: "reply".into(),
            cc: Vec::new(),
            bcc: Vec::new(),
            draft: false,
            status_after: Some(status.into()),
            assign_to: None,
            customer_id: None,
        }))
        .expect("reply created");
        let world = provider.lock_world();
        world
            .conversations
            .iter()
            .find(|c| c.remote_id == 101)
            .expect("conversation")
            .status
            .clone()
    }

    /// Clean-audit item 25: the loopback world speaks the platform's
    /// vocabulary. `open` (the real v2 await-agent status) and the legacy
    /// phantom `waiting` both land on the world's `active` — never an
    /// unknown status string the mirror cannot reason about.
    #[test]
    fn audit25_loopback_maps_open_and_legacy_waiting_onto_active() {
        let provider = provider_with_conversation();
        assert_eq!(reply_status_after(&provider, "open"), "active");
        assert_eq!(reply_status_after(&provider, "waiting"), "active");
        assert_eq!(reply_status_after(&provider, "pending"), "pending");
        assert_eq!(reply_status_after(&provider, "closed"), "closed");
    }
}

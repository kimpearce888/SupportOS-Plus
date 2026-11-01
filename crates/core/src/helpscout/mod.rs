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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsChat {
    pub id: String,
    #[serde(default)]
    pub beacon_id: Option<String>,
    #[serde(default)]
    pub mailbox_id: i64,
    #[serde(default)]
    pub customer: Option<HsChatParticipant>,
    #[serde(default)]
    pub assignee: Option<HsChatParticipant>,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub conversation_remote_id: Option<i64>,
    #[serde(default)]
    pub events: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsChatEvent {
    pub event_type: String,
    pub chat: HsChat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HsDocArticle {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocArticleDraftInput {
    pub collection_id: String,
    pub name: String,
    pub text: String,
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

    async fn create_saved_reply(&self, _mailbox_id: i64, _name: &str, _text: &str) -> Result<i64> {
        Err(crate::error::Error::Other(
            "saved reply writes require the real provider".into(),
        ))
    }

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

    async fn get_chat(&self, _chat_id: &str) -> Result<Option<HsChat>> {
        Ok(None)
    }

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

    async fn get_doc_article(&self, _remote_id: String) -> Result<Option<HsDocArticle>> {
        Ok(None)
    }

    async fn upsert_doc_article_draft(&self, _input: &DocArticleDraftInput) -> Result<String> {
        Err(Error::Config(
            "Docs API publishing is unavailable on this connection - no Docs API key is \
             configured."
                .into(),
        ))
    }

    async fn publish_doc_article(
        &self,
        _remote_id: String,
        _name: &str,
        _text: &str,
    ) -> Result<()> {
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

mod inert;

pub use inert::{EmptyHelpScoutProvider, InertWorld};

#[cfg(test)]
mod batch20_audit25_tests {
    use super::inert::EmptyHelpScoutProvider;
    use super::*;

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

    #[test]
    fn audit25_loopback_maps_open_and_legacy_waiting_onto_active() {
        let provider = provider_with_conversation();
        assert_eq!(reply_status_after(&provider, "open"), "active");
        assert_eq!(reply_status_after(&provider, "waiting"), "active");
        assert_eq!(reply_status_after(&provider, "pending"), "pending");
        assert_eq!(reply_status_after(&provider, "closed"), "closed");
    }
}

use serde::{Deserialize, Serialize};

use crate::error::Result;

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
    pub websites: Vec<HsCustomerWebsite>,
    #[serde(default)]
    pub social_profiles: Vec<HsCustomerSocialProfile>,
    #[serde(default)]
    pub address: Option<HsCustomerAddress>,
    #[serde(default)]
    pub properties: Vec<HsCustomerPropertyValue>,
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
pub struct HsDocArticle {
    pub remote_id: i64,
    pub collection_id: i64,
    #[serde(default)]
    pub category_id: Option<i64>,
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
    pub remote_id: i64,
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
    pub remote_id: i64,
    pub collection_id: i64,
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

    async fn get_company_overall_report(
        &self,
        _start: &str,
        _end: &str,
    ) -> Result<Option<HsReportRow>> {
        Ok(None)
    }

    async fn get_conversations_overall_report(
        &self,
        _start: &str,
        _end: &str,
    ) -> Result<Option<HsReportRow>> {
        Ok(None)
    }

    async fn get_happiness_ratings_report(
        &self,
        _start: &str,
        _end: &str,
    ) -> Result<Option<HsReportRow>> {
        Ok(None)
    }

    async fn get_productivity_overall_report(
        &self,
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

    async fn get_user_status(&self, _user_id: i64) -> Result<Option<HsUserStatus>> {
        Ok(None)
    }

    async fn list_system_users(&self) -> Result<Vec<HsUser>> {
        Ok(Vec::new())
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        Ok(Vec::new())
    }

    async fn list_doc_categories(&self, _collection_id: i64) -> Result<Vec<HsDocCategory>> {
        Ok(Vec::new())
    }

    async fn list_doc_articles(&self, _collection_id: i64) -> Result<Vec<HsDocArticle>> {
        Ok(Vec::new())
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

pub struct FakeHelpScoutProvider {
    world: Mutex<FakeWorld>,
}

#[derive(Debug, Clone, Default)]
pub struct FakeWorld {
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

fn days_ago(n: i64, hour: u32, minute: u32) -> String {
    let t = chrono::Utc::now() - chrono::Duration::days(n);
    let date = t.date_naive();
    use chrono::TimeZone;
    chrono::Utc
        .from_utc_datetime(
            &date
                .and_hms_opt(hour, minute, 0)
                .unwrap_or_else(|| t.naive_utc()),
        )
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn minutes_after(iso: &str, minutes: i64) -> String {
    parse_iso(iso)
        .map(|t| {
            (t + chrono::Duration::minutes(minutes))
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .unwrap_or_else(|| iso.to_string())
}

fn hours_ago_now(hours: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::hours(hours))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_iso(iso: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

impl FakeWorld {
    #[must_use]
    pub fn demo() -> Self {
        let mut b = WorldBuilder::new();

        let c1 = b.conversation(ConvSpec {
            subject: "Scheduled report sent at wrong hour (Santiago time)",
            preview:
                "Our daily dispatch report is being sent at 3 AM Chilean time instead of 8 AM...",
            mailbox_id: 201,
            customer_id: 3001,
            status: "active",
            tags: &["timezone", "vip"],
            assignee_id: Some(1001),
            created_days_ago: 3,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c1, "customer", "<p>Hello,</p><p>Our daily dispatch report is being sent at 3 AM Chilean time instead of 8 AM as configured. We are in Santiago (UTC-4 currently due to daylight saving). The workspace timezone says \"America/Santiago\" in the settings page but the schedule editor still shows UTC times.</p><p>Can you tell me how to make the schedule follow our local timezone? This affects our morning operations meeting.</p><p>Thank you,<br>Lucía Morales<br>Andes Logistics</p>", days_ago(3, 9, 12), Some(3001), None);
        b.thread(c1, "reply", "<p>Hi Lucía,</p><p>Thanks for the details. I can see the workspace is set to America/Santiago and the \"Daily dispatch\" schedule is currently stored with a UTC offset from before the daylight-saving change.</p><p>Could you open the schedule and re-save it once? That re-stamps it with the current offset. I am checking with engineering whether a mid-cycle DST change can re-anchor schedules automatically.</p><p>Best,<br>Alex</p>", days_ago(3, 13, 5), None, Some(1001));
        b.thread(c1, "customer", "<p>I re-saved the schedule and it now shows 8 AM correctly. But a second report (\"Weekly summary\") is still one hour off.</p>", days_ago(2, 10, 22), Some(3001), None);

        let c2 = b.conversation(ConvSpec {
            subject: "Meeting reminders in wrong timezone after DST",
            preview:
                "Since the clock change last weekend all meeting reminders arrive one hour late...",
            mailbox_id: 201,
            customer_id: 3002,
            status: "active",
            tags: &["timezone", "release-2-4"],
            assignee_id: None,
            created_days_ago: 5,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c2, "customer", "<p>Since the clock change last weekend all meeting reminders arrive one hour late. We are in Chile. Is there a fix?</p>", days_ago(5, 11, 3), Some(3002), None);

        let c3 = b.conversation(ConvSpec {
            subject: "Timezone for scheduled exports",
            preview: "How do I set the timezone used for scheduled exports?",
            mailbox_id: 201,
            customer_id: 3005,
            status: "closed",
            tags: &["timezone"],
            assignee_id: Some(1002),
            created_days_ago: 40,
            closed_days_ago: Some(39),
            snoozed_until: None,
        });
        b.thread(c3, "customer", "<p>How do I set the timezone used for scheduled exports? They all arrive in UTC and my team is in Stockholm.</p>", days_ago(40, 9, 45), Some(3005), None);
        b.thread(c3, "reply", "<p>Hi Emma,</p><p>Scheduled exports follow the workspace timezone: Settings > Workspace > Regional settings. After changing it, re-save each schedule once so the stored times re-anchor to the new timezone.</p><p>Best,<br>Priya</p>", days_ago(40, 12, 10), None, Some(1002));
        b.thread(
            c3,
            "customer",
            "<p>That worked, thank you!</p>",
            days_ago(39, 8, 30),
            Some(3005),
            None,
        );

        let c4 = b.conversation(ConvSpec {
            subject: "Invitation email never arrives for new teammate",
            preview: "I invited daniel@brightpathedu.org three times but no email arrives...",
            mailbox_id: 201,
            customer_id: 3003,
            status: "pending",
            tags: &["registration"],
            assignee_id: Some(1002),
            created_days_ago: 6,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c4, "customer", "<p>Hello,</p><p>I invited daniel@brightpathedu.org three times yesterday but no invitation email arrives. Our mail provider logs show nothing from your domain either. Could you check whether the invitations are being sent?</p><p>Thanks,<br>Sarah</p>", days_ago(6, 10, 5), Some(3003), None);
        b.thread(c4, "note", "<p>Checked mail logs - invitation to daniel@brightpathedu.org bounced with \"550 policy reasons\" from their provider. Re-sent after whitelisting; asked customer to confirm arrival. If it bounces again we will recommend sending to an alias address.</p>", days_ago(6, 15, 20), None, Some(1002));
        b.thread(c4, "reply", "<p>Hi Sarah,</p><p>The invitation to daniel@brightpathedu.org was bouncing with a policy rejection from your mail provider. I have re-sent it and whitelisted your domain on our side. Could you confirm whether it arrives in the next few minutes? If not, we can send it to an alternate address.</p><p>Best,<br>Priya</p>", days_ago(6, 15, 25), None, Some(1002));

        let c5 = b.conversation(ConvSpec {
            subject: "What can a Viewer see?",
            preview:
                "What is the difference between Viewer and Editor? Can viewers see all reports...",
            mailbox_id: 201,
            customer_id: 3004,
            status: "closed",
            tags: &["viewer"],
            assignee_id: Some(1001),
            created_days_ago: 12,
            closed_days_ago: Some(11),
            snoozed_until: None,
        });
        b.thread(c5, "customer", "<p>What is the difference between Viewer and Editor? Can viewers see all reports or only ones shared with them? Can they export data?</p>", days_ago(12, 9, 40), Some(3004), None);
        b.thread(c5, "reply", "<p>Hi Daniel,</p><p>A Viewer can see every dashboard and report shared with their team, but cannot edit, comment, or create new ones. Viewers can export data from reports they can see (CSV/PDF). An Editor seat is required for edit rights.</p><p>Best,<br>Alex</p>", days_ago(12, 11, 15), None, Some(1001));

        let c6h1 = b.conversation(ConvSpec {
            subject: "Webhook payload format after v2.4 upgrade",
            preview: "After upgrading to v2.4 our webhook receiver rejects the payload schema...",
            mailbox_id: 201,
            customer_id: 3006,
            status: "closed",
            tags: &["integration"],
            assignee_id: Some(1002),
            created_days_ago: 70,
            closed_days_ago: Some(68),
            snoozed_until: None,
        });
        b.thread(c6h1, "customer", "<p>Hello,</p><p>After upgrading to v2.4 last Saturday our webhook receiver started rejecting the payload schema. I captured the failing delivery from the integrations log (delivery ID WH-10231) and diffed it against the v2.3 format:</p><p>- The \"event.type\" field now uses dot notation (\"conversation.updated\" instead of \"conversationUpdated\")<br>- The \"payload\" object is base64-encoded rather than plain JSON<br>- Headers include a new X-Signature-v2 alongside the legacy X-Signature</p><p>Our receiver validates against a strict JSON schema and returns HTTP 422 before the handler runs, so nothing is processed. I could relax the schema, but I would rather understand the intended contract first. Is there a changelog entry describing the new format, and is the legacy format supported during a transition period? We process roughly 4,000 events per day through this endpoint, so I want to migrate deliberately rather than reactively.</p><p>Thanks,<br>Ravi Sundaram<br>PixelWorks IT</p>", days_ago(70, 9, 40), Some(3006), None);
        b.thread(c6h1, "reply", "<p>Hi Ravi,</p><p>The v2.4 release notes cover the webhook contract change under \"Breaking changes\". The legacy format is supported until the end of the quarter via the workspace setting \"Webhooks: legacy payload\", after which dot-notation events become the only format. Both signature headers validate with the same secret during the transition.</p><p>Recommended migration order: add schema acceptance for both shapes first, monitor dual-format traffic for a week, then drop the legacy branch.</p><p>Best,<br>Priya</p>", days_ago(69, 11, 20), None, Some(1002));
        b.thread(c6h1, "customer", "<p>That is exactly what I needed — the dual-format monitoring suggestion made the migration straightforward. Receiver deployed with both schemas accepted and traffic looks clean. Closing from my side.</p>", days_ago(68, 8, 15), Some(3006), None);

        let c6h2 = b.conversation(ConvSpec {
            subject: "API rate limits for bulk export endpoint",
            preview: "What are the documented rate limits for the bulk export API and do they reset per token...",
            mailbox_id: 201,
            customer_id: 3006,
            status: "closed",
            tags: &["api"],
            assignee_id: Some(1002),
            created_days_ago: 38,
            closed_days_ago: Some(36),
            snoozed_until: None,
        });
        b.thread(c6h2, "customer", "<p>Hello,</p><p>Two questions about the bulk export API (<code>/v3/exports</code>):</p><p>1. The documentation mentions a per-minute rate limit but not whether it applies per API token, per workspace, or per endpoint. Which is it? We run two workers with separate tokens from the same workspace and saw inconsistent 429 behavior.<br>2. When a 429 returns the Retry-After header, does the documented limit reset at that instant or at the next window boundary?</p><p>Context: we schedule exports nightly with a 15-minute window, and a mid-run 429 currently aborts the whole job. I would rather back off and resume than abort, but I need to know which clock the limit resets on.</p><p>Thanks,<br>Ravi</p>", days_ago(38, 10, 5), Some(3006), None);
        b.thread(c6h2, "reply", "<p>Hi Ravi,</p><p>Answers below:</p><p>1. The limit is per API token, not per workspace. Your two workers each have the full documented quota, which explains the inconsistency you saw — one worker was likely consuming a shared proxy cache.<br>2. The window is a fixed rolling 60 seconds counted from the first request; Retry-After points to the end of the current window, so backing off until that timestamp is correct and resuming is safe.</p><p>Your resume-instead-of-abort plan is exactly what the header is for.</p><p>Best,<br>Priya</p>", days_ago(37, 9, 50), None, Some(1002));
        b.thread(c6h2, "customer", "<p>Clear and complete. Implemented per-token accounting with resume-on-429 and the nightly job has been clean since. Thank you!</p>", days_ago(36, 9, 10), Some(3006), None);

        let c6h3 = b.conversation(ConvSpec {
            subject: "SSO SAML metadata renewal question",
            preview: "Our identity provider is rotating certificates next month - what do we need to update...",
            mailbox_id: 201,
            customer_id: 3006,
            status: "closed",
            tags: &["sso", "account"],
            assignee_id: Some(1001),
            created_days_ago: 17,
            closed_days_ago: Some(15),
            snoozed_until: None,
        });
        b.thread(c6h3, "customer", "<p>Hello,</p><p>Our identity provider rotates SAML signing certificates annually and the next rotation lands on the first of next month. Before that date I want to confirm the renewal procedure on your side so logins do not break for our 120 users:</p><p>- Does the workspace accept a metadata URL that serves both the current and the upcoming certificate during overlap, or must the new certificate be uploaded manually?<br>- Is there a documented propagation delay after metadata refresh that we should schedule around?<br>- Are there logs in the admin panel that would show a failing assertion signature specifically, so I can distinguish a rotation issue from a clock-skew issue?</p><p>Historically the annual rotation has been smooth, but last year the overlap window was shorter than the propagation delay and a few users hit a failed login loop. I would like to avoid a repeat.</p><p>Thanks,<br>Ravi</p>", days_ago(17, 9, 25), Some(3006), None);
        b.thread(c6h3, "reply", "<p>Hi Ravi,</p><p>The metadata URL path is the recommended one: we fetch it nightly and accept every certificate it advertises, so serving both during the overlap period is exactly right. Propagation is at most 24 hours after the nightly fetch, so start the overlap window two days early. Admin → Security → SSO log entries distinguish \"assertion signature validation failed\" (rotation) from \"assertion time window exceeded\" (clock skew).</p><p>Best,<br>Alex</p>", days_ago(16, 14, 5), None, Some(1001));
        b.thread(c6h3, "customer", "<p>Started the overlap window today as suggested. Rotation completed overnight with zero failed logins — the log filter you pointed out made verification quick. Thanks again.</p>", days_ago(15, 9, 0), Some(3006), None);

        let c6 = b.conversation(ConvSpec {
            subject: "Slack integration stopped posting updates",
            preview:
                "Since last week the Slack integration no longer posts updates to our channel...",
            mailbox_id: 201,
            customer_id: 3006,
            status: "active",
            tags: &["integration", "escalated", "release-2-4"],
            assignee_id: Some(1003),
            created_days_ago: 4,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c6, "customer", "<p>Hi,</p><p>Since last week the Slack integration no longer posts updates to our #ops channel. I disconnected and reconnected once already. We use it for alerting so this is urgent for us.</p><p>Log ID from the integrations page: INT-88231.</p><p>Ravi</p>", days_ago(4, 8, 55), Some(3006), None);
        b.thread(c6, "note", "<p>INT-88231 shows repeated 401 from Slack side after their token rotation policy change. Escalating to engineering - reference ENG-4471. Customer-facing wording must stay generic until engineering confirms.</p>", days_ago(3, 9, 30), None, Some(1003));
        b.thread(c6, "reply", "<p>Hi Ravi,</p><p>Thanks for the log ID. We traced the failure to an authentication change on Slack's side affecting some workspaces. Our engineering team is working on a fix and I will update you as soon as it is deployed. Your historical data is unaffected.</p><p>Best,<br>Tom</p>", days_ago(3, 9, 45), None, Some(1003));

        let c7 = b.conversation(ConvSpec {
            subject: "Card payment failing but card is valid",
            preview: "Our subscription shows past due but our card works everywhere else...",
            mailbox_id: 202,
            customer_id: 3007,
            status: "active",
            tags: &["billing"],
            assignee_id: None,
            created_days_ago: 2,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c7, "customer", "<p>Bonjour,</p><p>Our subscription shows \"past due\" but our card works everywhere else. The bank says no charge was even attempted this month. Can you retry the payment?</p><p>Merci,<br>Chloe Dubois<br>Atelier France</p>", days_ago(2, 9, 5), Some(3007), None);

        let c8 = b.conversation(ConvSpec {
            subject: "Need VAT number on invoices",
            preview: "Can you add our VAT number FR40303265045 to all invoices...",
            mailbox_id: 202,
            customer_id: 3007,
            status: "closed",
            tags: &["billing"],
            assignee_id: Some(1001),
            created_days_ago: 25,
            closed_days_ago: Some(24),
            snoozed_until: None,
        });
        b.thread(c8, "customer", "<p>Can you add our VAT number FR40303265045 to all invoices, including past ones? Our accounting needs it for the annual filing.</p>", days_ago(25, 10, 15), Some(3007), None);
        b.thread(c8, "reply", "<p>Hi Chloe,</p><p>I have added VAT number FR40303265045 to your billing profile and re-issued the last 12 invoices as PDFs; they are attached to your billing history. Future invoices will include it automatically.</p><p>Best,<br>Alex</p>", days_ago(24, 14, 0), None, Some(1001));

        let c9 = b.conversation(ConvSpec {
            subject: "Can automation rules run on a schedule?",
            preview: "Can I schedule an automation rule to run every morning at 9 and tag stale tickets...",
            mailbox_id: 201,
            customer_id: 3008,
            status: "active",
            tags: &["automation"],
            assignee_id: Some(1002),
            created_days_ago: 1,
            closed_days_ago: None,
            snoozed_until: None,
        });
        b.thread(c9, "customer", "<p>Hello,</p><p>Two questions about automation rules:</p><p>1) Can I schedule a rule to run every morning at 9 AM, e.g. to tag stale tickets?</p><p>2) Is there an API to trigger rules externally?</p><p>Thank you,<br>Hiro Tanaka</p>", days_ago(1, 8, 20), Some(3008), None);

        let c10 = b.conversation(ConvSpec {
            subject: "Duplicate account created",
            preview:
                "I accidentally signed up twice with two emails. Can you merge the accounts...",
            mailbox_id: 201,
            customer_id: 3005,
            status: "closed",
            tags: &["registration"],
            assignee_id: Some(1001),
            created_days_ago: 55,
            closed_days_ago: Some(54),
            snoozed_until: None,
        });
        b.thread(c10, "customer", "<p>I accidentally signed up twice with two emails. Can you merge the accounts? The one to keep is emma.lindqvist@nordicmail.se.</p>", days_ago(55, 13, 30), Some(3005), None);
        b.thread(c10, "reply", "<p>Hi Emma,</p><p>I merged the accounts and moved the license to emma.lindqvist@nordicmail.se. The duplicate address can no longer be used to log in.</p><p>Best,<br>Alex</p>", days_ago(54, 10, 0), None, Some(1001));

        let c11 = b.conversation(ConvSpec {
            subject: "Data export format question",
            preview: "Can exports include the raw JSON fields in addition to CSV...",
            mailbox_id: 201,
            customer_id: 3003,
            status: "pending",
            tags: &[],
            assignee_id: Some(1001),
            created_days_ago: 8,
            closed_days_ago: None,
            snoozed_until: Some(days_ago(-2, 9, 0)),
        });
        b.thread(c11, "customer", "<p>Can exports include the raw JSON fields in addition to CSV? We want to load them into our warehouse.</p>", days_ago(8, 11, 11), Some(3003), None);
        b.thread(c11, "reply", "<p>Hi Sarah,</p><p>CSV is the only scheduled-export format today. I have noted your interest in JSON. Would a one-off manual export work for you in the meantime?</p><p>Best,<br>Alex</p>", days_ago(8, 15, 45), None, Some(1001));

        let c12 = b.conversation(ConvSpec {
            subject: "Reminder one hour late",
            preview: "Meeting reminders are one hour late since the weekend.",
            mailbox_id: 201,
            customer_id: 3002,
            status: "closed",
            tags: &["timezone"],
            assignee_id: None,
            created_days_ago: 5,
            closed_days_ago: None,
            snoozed_until: None,
        });
        if let Some(merged) = b.conversations.iter_mut().find(|c| c.remote_id == c12) {
            merged.merged_into = Some(c2);
        }

        let ch1 = b.chat_session(ChatSpec {
            subject: "Quick question about viewer seats",
            preview: "Do viewers count against our seat limit?",
            mailbox_id: 201,
            customer_id: 3004,
            status: "closed",
            tags: &["beacon", "viewer"],
            assignee_id: Some(1002),
            created_days_ago: 2,
            start_hour: 15,
            closed_after_min: Some(14),
        });
        b.thread(
            ch1,
            "customer",
            "Hi! Quick question — do viewer seats count against our plan limit?",
            days_ago(2, 15, 0),
            Some(3004),
            None,
        );
        b.thread(ch1, "reply", "Hi Daniel! Viewers are unlimited on the Growth plan — only editor seats count. You are currently at 7 of 10 editor seats, so you can invite as many viewers as you like.", minutes_after(&days_ago(2, 15, 0), 6), None, Some(1002));
        b.thread(
            ch1,
            "customer",
            "Perfect, exactly what I needed. Thanks Priya!",
            minutes_after(&days_ago(2, 15, 0), 11),
            Some(3004),
            None,
        );

        let ch2 = b.chat_session(ChatSpec {
            subject: "SSO login loop",
            preview: "SSO keeps redirecting me back to the login page.",
            mailbox_id: 201,
            customer_id: 3008,
            status: "closed",
            tags: &["beacon", "sso"],
            assignee_id: Some(1002),
            created_days_ago: 4,
            start_hour: 9,
            closed_after_min: Some(9),
        });
        b.thread(ch2, "customer", "Hi — SSO keeps redirecting me back to the login page. Chrome on macOS, started this morning.", days_ago(4, 9, 0), Some(3008), None);
        b.thread(ch2, "reply", "Hi Hiro! Please try an incognito window first. If that works, clear cookies for app.zylker.io — a stale session cookie is the usual cause of this loop. There is also a checklist in our internal SSO article I can walk you through.", minutes_after(&days_ago(4, 9, 0), 4), None, Some(1002));
        b.thread(
            ch2,
            "customer",
            "Incognito worked. Cleared the cookies and I am in. Arigatō!",
            minutes_after(&days_ago(4, 9, 0), 8),
            Some(3008),
            None,
        );

        let ch3 = b.chat_session(ChatSpec {
            subject: "Receipt for last month",
            preview: "Can you resend the receipt for last month?",
            mailbox_id: 202,
            customer_id: 3007,
            status: "closed",
            tags: &["beacon", "billing"],
            assignee_id: Some(1001),
            created_days_ago: 6,
            start_hour: 11,
            closed_after_min: Some(5),
        });
        b.thread(ch3, "customer", "Bonjour — can you resend the receipt for last month? My accountant lost the original email.", days_ago(6, 11, 0), Some(3007), None);
        b.thread(ch3, "reply", "Of course, Chloe — I have just re-sent the November receipt to chloe@atelierfrance.fr. It should arrive within a minute. You can also download receipts any time under Billing → Invoices.", minutes_after(&days_ago(6, 11, 0), 3), None, Some(1001));
        b.thread(
            ch3,
            "customer",
            "Received, merci.",
            minutes_after(&days_ago(6, 11, 0), 4),
            Some(3007),
            None,
        );

        let ch4 = b.chat_session(ChatSpec {
            subject: "How do I invite a teammate?",
            preview: "How do I invite a teammate as a viewer?",
            mailbox_id: 201,
            customer_id: 3003,
            status: "closed",
            tags: &["beacon"],
            assignee_id: Some(1001),
            created_days_ago: 9,
            start_hour: 14,
            closed_after_min: Some(4),
        });
        b.thread(
            ch4,
            "customer",
            "How do I invite a teammate as a viewer? I do not want them to edit reports.",
            days_ago(9, 14, 0),
            Some(3003),
            None,
        );
        b.thread(ch4, "reply", "Hi Sarah! Go to Settings → Team → Invite and pick \"Viewer\" in the role dropdown before sending. Viewers can see every shared report but cannot edit or schedule anything.", minutes_after(&days_ago(9, 14, 0), 2), None, Some(1001));
        b.thread(
            ch4,
            "customer",
            "Done — invitation sent. Thanks!",
            minutes_after(&days_ago(9, 14, 0), 3),
            Some(3003),
            None,
        );

        let ch5 = b.chat_session(ChatSpec {
            subject: "Manual export while the schedule is broken",
            preview: "Can I trigger the dispatch report manually today?",
            mailbox_id: 201,
            customer_id: 3002,
            status: "closed",
            tags: &["beacon", "timezone"],
            assignee_id: Some(1002),
            created_days_ago: 1,
            start_hour: 16,
            closed_after_min: Some(12),
        });
        b.thread(ch5, "customer", "Since the DST issue our dispatch report is late — can I trigger it manually for today?", days_ago(1, 16, 0), Some(3002), None);
        b.thread(ch5, "reply", "Yes! Reports → Dispatch → \"Run now\" runs immediately and does not touch the schedule. I have also re-anchored your schedule to the current Santiago offset, so tomorrow's run should fire at 8 AM local again.", minutes_after(&days_ago(1, 16, 0), 8), None, Some(1002));
        b.thread(
            ch5,
            "customer",
            "Perfect — running now. Gracias!",
            minutes_after(&days_ago(1, 16, 0), 10),
            Some(3002),
            None,
        );

        let ch6 = b.chat_session(ChatSpec {
            subject: "Shared view link returns 404",
            preview: "The shared view link I sent a colleague returns a 404.",
            mailbox_id: 201,
            customer_id: 3005,
            status: "active",
            tags: &["beacon", "viewer"],
            assignee_id: Some(1002),
            created_days_ago: 0,
            start_hour: 10,
            closed_after_min: None,
        });
        for c in b.conversations.iter_mut().filter(|c| c.remote_id == ch6) {
            c.created_at = Some(hours_ago_now(2));
            c.updated_at = Some(hours_ago_now(2));
        }
        b.thread(
            ch6,
            "customer",
            "Hi — the shared view link I sent a colleague returns a 404 page. It worked last week.",
            hours_ago_now(2),
            Some(3005),
            None,
        );

        let ratings = vec![
            HsRating {
                remote_id: 601,
                conversation_id: Some(c3),
                thread_id: None,
                rating: Some("great".into()),
                comment: Some("Quick and clear, thank you!".into()),
                customer_id: Some(3005),
                customer_name: Some("Emma Lindqvist".into()),
                user_id: Some(1002),
                created_at: Some(days_ago(39, 9, 0)),
            },
            HsRating {
                remote_id: 602,
                conversation_id: Some(c5),
                thread_id: None,
                rating: Some("great".into()),
                comment: None,
                customer_id: Some(3004),
                customer_name: Some("Daniel Kim".into()),
                user_id: Some(1001),
                created_at: Some(days_ago(11, 12, 0)),
            },
            HsRating {
                remote_id: 603,
                conversation_id: Some(c8),
                thread_id: None,
                rating: Some("okay".into()),
                comment: Some("Fine, but would like this self-service.".into()),
                customer_id: Some(3007),
                customer_name: Some("Chloe Dubois".into()),
                user_id: Some(1001),
                created_at: Some(days_ago(24, 15, 0)),
            },
            HsRating {
                remote_id: 604,
                conversation_id: Some(c10),
                thread_id: None,
                rating: Some("great".into()),
                comment: None,
                customer_id: Some(3005),
                customer_name: Some("Emma Lindqvist".into()),
                user_id: Some(1001),
                created_at: Some(days_ago(54, 11, 0)),
            },
            HsRating {
                remote_id: 605,
                conversation_id: Some(ch1),
                thread_id: None,
                rating: Some("great".into()),
                comment: Some("Answered in six minutes over chat!".into()),
                customer_id: Some(3004),
                customer_name: Some("Daniel Kim".into()),
                user_id: Some(1002),
                created_at: Some(days_ago(2, 15, 14)),
            },
            HsRating {
                remote_id: 606,
                conversation_id: Some(ch3),
                thread_id: None,
                rating: Some("okay".into()),
                comment: Some("Fast, but I would love a self-service receipts page.".into()),
                customer_id: Some(3007),
                customer_name: Some("Chloe Dubois".into()),
                user_id: Some(1001),
                created_at: Some(days_ago(6, 11, 6)),
            },
            HsRating {
                remote_id: 607,
                conversation_id: Some(ch2),
                thread_id: None,
                rating: Some("great".into()),
                comment: None,
                customer_id: Some(3008),
                customer_name: Some("Hiro Tanaka".into()),
                user_id: Some(1002),
                created_at: Some(days_ago(4, 9, 10)),
            },
        ];

        Self {
            me: HsUser {
                remote_id: 1001,
                first_name: Some("Alex".into()),
                last_name: Some("Rivera".into()),
                email: Some("alex@zylker.io".into()),
                role: Some("owner".into()),
                user_type: "user".into(),
                timezone: Some("America/New_York".into()),
                photo_url: None,
                initials: Some("AR".into()),
                mention: Some("alex".into()),
                job_title: Some("Support Lead".into()),
                phone: None,
                alternate_emails: vec![],
                created_at: Some(days_ago(400, 10, 30)),
                updated_at: Some(days_ago(20, 10, 30)),
            },
            mailboxes: vec![
                HsMailbox { remote_id: 201, name: "Support".into(), slug: Some("a1b2c3".into()), email: Some("support@zylker.io".into()), created_at: Some(days_ago(400, 10, 30)), updated_at: Some(days_ago(20, 10, 30)) },
                HsMailbox { remote_id: 202, name: "Billing".into(), slug: Some("d4e5f6".into()), email: Some("billing@zylker.io".into()), created_at: Some(days_ago(350, 10, 30)), updated_at: Some(days_ago(12, 10, 30)) },
            ],
            users: vec![
                HsUser {
                    remote_id: 1001,
                    first_name: Some("Alex".into()),
                    last_name: Some("Rivera".into()),
                    email: Some("alex@zylker.io".into()),
                    role: Some("owner".into()),
                    user_type: "user".into(),
                    timezone: Some("America/New_York".into()),
                    photo_url: None,
                    initials: Some("AR".into()),
                    mention: Some("alex".into()),
                    job_title: Some("Support Lead".into()),
                    phone: None,
                    alternate_emails: vec![],
                    created_at: Some(days_ago(400, 10, 30)),
                    updated_at: Some(days_ago(20, 10, 30)),
                },
                HsUser {
                    remote_id: 1002,
                    first_name: Some("Priya".into()),
                    last_name: Some("Nair".into()),
                    email: Some("priya@zylker.io".into()),
                    role: Some("user".into()),
                    user_type: "user".into(),
                    timezone: Some("Asia/Kolkata".into()),
                    photo_url: None,
                    initials: Some("PN".into()),
                    mention: Some("priya".into()),
                    job_title: Some("Support Engineer".into()),
                    phone: None,
                    alternate_emails: vec![],
                    created_at: Some(days_ago(300, 10, 30)),
                    updated_at: Some(days_ago(15, 10, 30)),
                },
                HsUser {
                    remote_id: 1003,
                    first_name: Some("Tom".into()),
                    last_name: Some("Bright".into()),
                    email: Some("tom@zylker.io".into()),
                    role: Some("user".into()),
                    user_type: "user".into(),
                    timezone: Some("Europe/Berlin".into()),
                    photo_url: None,
                    initials: Some("TB".into()),
                    mention: Some("tom".into()),
                    job_title: Some("Support Engineer".into()),
                    phone: None,
                    alternate_emails: vec![],
                    created_at: Some(days_ago(250, 10, 30)),
                    updated_at: Some(days_ago(10, 10, 30)),
                },
            ],
            system_users: vec![
                HsUser {
                    remote_id: 9001,
                    first_name: Some("AI Agent".into()),
                    last_name: Some(String::new()),
                    email: Some("ai-agent@zylker.io".into()),
                    role: Some("user".into()),
                    user_type: "system_user".into(),
                    timezone: Some("UTC".into()),
                    photo_url: None,
                    initials: Some("AA".into()),
                    mention: None,
                    job_title: None,
                    phone: None,
                    alternate_emails: vec![],
                    created_at: Some(days_ago(60, 10, 30)),
                    updated_at: Some(days_ago(60, 10, 30)),
                },
            ],
            teams: vec![
                HsTeam { remote_id: 501, name: "Tier 1".into(), member_user_ids: vec![1001, 1002] },
                HsTeam { remote_id: 502, name: "Escalations".into(), member_user_ids: vec![1003] },
            ],
            tags: vec![
                HsTag { remote_id: 701, name: "timezone".into(), slug: Some("timezone".into()), color: Some("#37A4FF".into()), ticket_count: Some(6), created_at: Some(days_ago(200, 10, 30)), updated_at: Some(days_ago(2, 10, 30)) },
                HsTag { remote_id: 702, name: "billing".into(), slug: Some("billing".into()), color: Some("#517EDB".into()), ticket_count: Some(5), created_at: Some(days_ago(200, 10, 30)), updated_at: Some(days_ago(3, 10, 30)) },
                HsTag { remote_id: 703, name: "integration".into(), slug: Some("integration".into()), color: Some("#517EDB".into()), ticket_count: Some(4), created_at: Some(days_ago(180, 10, 30)), updated_at: Some(days_ago(1, 10, 30)) },
                HsTag { remote_id: 704, name: "registration".into(), slug: Some("registration".into()), color: Some("#56AF31".into()), ticket_count: Some(3), created_at: Some(days_ago(150, 10, 30)), updated_at: Some(days_ago(5, 10, 30)) },
                HsTag { remote_id: 705, name: "viewer".into(), slug: Some("viewer".into()), color: Some("#56AF31".into()), ticket_count: Some(3), created_at: Some(days_ago(120, 10, 30)), updated_at: Some(days_ago(4, 10, 30)) },
                HsTag { remote_id: 706, name: "automation".into(), slug: Some("automation".into()), color: Some("#929499".into()), ticket_count: Some(2), created_at: Some(days_ago(90, 10, 30)), updated_at: Some(days_ago(6, 10, 30)) },
                HsTag { remote_id: 707, name: "vip".into(), slug: Some("vip".into()), color: Some("#E4BB2F".into()), ticket_count: Some(2), created_at: Some(days_ago(80, 10, 30)), updated_at: Some(days_ago(7, 10, 30)) },
                HsTag { remote_id: 708, name: "escalated".into(), slug: Some("escalated".into()), color: Some("#DE5B49".into()), ticket_count: Some(2), created_at: Some(days_ago(70, 10, 30)), updated_at: Some(days_ago(2, 10, 30)) },
                HsTag { remote_id: 709, name: "release-2-4".into(), slug: Some("release-2-4".into()), color: Some("#929499".into()), ticket_count: Some(3), created_at: Some(days_ago(14, 10, 30)), updated_at: Some(days_ago(1, 10, 30)) },
                HsTag { remote_id: 710, name: "docs-gap".into(), slug: Some("docs-gap".into()), color: Some("#929499".into()), ticket_count: Some(1), created_at: Some(days_ago(30, 10, 30)), updated_at: Some(days_ago(30, 10, 30)) },
                HsTag { remote_id: 711, name: "api".into(), slug: Some("api".into()), color: Some("#37A4FF".into()), ticket_count: Some(1), created_at: Some(days_ago(60, 10, 30)), updated_at: Some(days_ago(36, 10, 30)) },
                HsTag { remote_id: 712, name: "sso".into(), slug: Some("sso".into()), color: Some("#517EDB".into()), ticket_count: Some(1), created_at: Some(days_ago(40, 10, 30)), updated_at: Some(days_ago(15, 10, 30)) },
                HsTag { remote_id: 713, name: "account".into(), slug: Some("account".into()), color: Some("#929499".into()), ticket_count: Some(1), created_at: Some(days_ago(100, 10, 30)), updated_at: Some(days_ago(15, 10, 30)) },
                HsTag { remote_id: 714, name: "beacon".into(), slug: Some("beacon".into()), color: Some("#37A4FF".into()), ticket_count: Some(6), created_at: Some(days_ago(60, 10, 30)), updated_at: Some(days_ago(1, 10, 30)) },
            ],
            folders: vec![
                HsFolder { remote_id: 301, mailbox_id: 201, name: "Unassigned".into(), kind: "unassigned".into(), user_id: None, total_count: 4, active_count: 3 },
                HsFolder { remote_id: 302, mailbox_id: 201, name: "Mine".into(), kind: "mine".into(), user_id: Some(1001), total_count: 6, active_count: 4 },
                HsFolder { remote_id: 303, mailbox_id: 201, name: "Drafts".into(), kind: "drafts".into(), user_id: Some(1001), total_count: 1, active_count: 1 },
                HsFolder { remote_id: 304, mailbox_id: 202, name: "Unassigned".into(), kind: "unassigned".into(), user_id: None, total_count: 2, active_count: 1 },
            ],
            fields: vec![
                HsField {
                    remote_id: 104,
                    mailbox_id: 201,
                    name: "Topic".into(),
                    kind: "dropdown".into(),
                    system_type: None,
                    required: false,
                    sort_order: 1,
                    options: vec![
                        HsFieldOption { id: 168, order: 1, label: "Timezone / Scheduling".into() },
                        HsFieldOption { id: 169, order: 2, label: "Registration".into() },
                        HsFieldOption { id: 170, order: 3, label: "Viewer".into() },
                        HsFieldOption { id: 171, order: 4, label: "Integrations".into() },
                        HsFieldOption { id: 172, order: 5, label: "Billing".into() },
                        HsFieldOption { id: 173, order: 6, label: "Automation".into() },
                    ],
                },
                HsField {
                    remote_id: 105,
                    mailbox_id: 201,
                    name: "ai-topic".into(),
                    kind: "dropdown".into(),
                    system_type: Some("topic".into()),
                    required: false,
                    sort_order: 2,
                    options: vec![
                        HsFieldOption { id: 180, order: 1, label: "Billing".into() },
                        HsFieldOption { id: 181, order: 2, label: "Shipping".into() },
                    ],
                },
                HsField {
                    remote_id: 106,
                    mailbox_id: 201,
                    name: "Account tier".into(),
                    kind: "dropdown".into(),
                    system_type: None,
                    required: false,
                    sort_order: 3,
                    options: vec![
                        HsFieldOption { id: 190, order: 1, label: "Free".into() },
                        HsFieldOption { id: 191, order: 2, label: "Pro".into() },
                        HsFieldOption { id: 192, order: 3, label: "Enterprise".into() },
                    ],
                },
                HsField { remote_id: 107, mailbox_id: 202, name: "Plan issue".into(), kind: "singleline".into(), system_type: None, required: false, sort_order: 1, options: vec![] },
            ],
            saved_replies: vec![
                HsSavedReply {
                    remote_id: 401,
                    name: "Timezone - set workspace timezone".into(),
                    preview: Some("Hi there! You can change the workspace timezone under Settings > Workspace > Regional...".into()),
                    text: Some("Hi there!\n\nYou can change the workspace timezone under **Settings > Workspace > Regional settings**. After changing it, new scheduled items use the new timezone; existing scheduled reports keep their original time.\n\nLet me know if anything still looks off!".into()),
                },
                HsSavedReply {
                    remote_id: 402,
                    name: "Registration - invite not arriving".into(),
                    preview: Some("Sorry the invite did not arrive. Common causes: spam filtering or a typo in the address...".into()),
                    text: Some("Hi there!\n\nSorry the invite did not arrive. The most common causes are spam filtering or a typo in the address. Could you check your spam folder and confirm the exact address you used? I have re-sent the invitation now, and I have also whitelisted your domain on our side.".into()),
                },
                HsSavedReply {
                    remote_id: 403,
                    name: "Viewer role - what it can access".into(),
                    preview: Some("Viewers can see dashboards and reports but cannot edit them...".into()),
                    text: Some("Hi there!\n\nA Viewer can see every dashboard and report that is shared with their team, but cannot edit, comment, or create new ones. If someone needs edit rights, an Editor seat is required.".into()),
                },
                HsSavedReply {
                    remote_id: 404,
                    name: "Billing - update card and retry".into(),
                    preview: Some("You can update your card under Settings > Billing. After updating...".into()),
                    text: Some("Hi there!\n\nYou can update your card under **Settings > Billing > Payment method**. After updating, click \"Retry payment\" so the pending invoice is charged again; the license re-activates immediately after a successful charge.".into()),
                },
                HsSavedReply {
                    remote_id: 405,
                    name: "Integration - reconnect OAuth".into(),
                    preview: Some("To reconnect the integration: open Integrations, click Disconnect...".into()),
                    text: Some("Hi there!\n\nTo reconnect the integration: open **Integrations**, click **Disconnect**, then **Connect** again and approve the permission prompt. Reconnecting never deletes your historical sync data.".into()),
                },
            ],
            workflows: vec![
                HsWorkflow { remote_id: 601, mailbox_id: Some(201), name: "Assign to Tier 1".into(), kind: "manual".into(), status: "active".into(), sort_order: 1 },
                HsWorkflow { remote_id: 602, mailbox_id: Some(201), name: "Spam cleanup".into(), kind: "manual".into(), status: "active".into(), sort_order: 2 },
                HsWorkflow { remote_id: 603, mailbox_id: Some(202), name: "Auto-route billing".into(), kind: "automatic".into(), status: "active".into(), sort_order: 1 },
            ],
            webhooks: vec![
                HsWebhookConfig {
                    remote_id: 801,
                    url: "https://relay.example.com/helpscout".into(),
                    events: vec!["convo.created".into(), "convo.customer.reply.created".into(), "satisfaction.ratings".into()],
                    status: "active".into(),
                },
            ],
            customer_props: vec![
                HsPropertyDef { remote_id: 4101, name: "Plan".into(), slug: Some("plan".into()), kind: "dropdown".into(), sort_order: 1 },
                HsPropertyDef { remote_id: 4102, name: "Employees".into(), slug: Some("employees".into()), kind: "number".into(), sort_order: 2 },
                HsPropertyDef { remote_id: 4103, name: "Region".into(), slug: Some("region".into()), kind: "dropdown".into(), sort_order: 3 },
                HsPropertyDef { remote_id: 4104, name: "Account Manager".into(), slug: Some("account-manager".into()), kind: "text".into(), sort_order: 4 },
            ],
            org_props: vec![
                HsPropertyDef { remote_id: 4201, name: "Industry".into(), slug: Some("industry".into()), kind: "text".into(), sort_order: 1 },
            ],
            organizations: vec![
                HsOrganization { remote_id: 2001, name: "Andes Logistics".into(), domains: vec!["andeslogistics.cl".into()], created_at: Some(days_ago(220, 10, 30)), updated_at: Some(days_ago(10, 10, 30)) },
                HsOrganization { remote_id: 2002, name: "BrightPath Education".into(), domains: vec!["brightpathedu.org".into()], created_at: Some(days_ago(150, 10, 30)), updated_at: Some(days_ago(5, 10, 30)) },
            ],
            customers: vec![
                build_customer(CustomerSpec {
                    remote_id: 3001,
                    first: "Lucía",
                    last: "Morales",
                    job_title: Some("Operations Manager"),
                    emails: vec![cust_email("lucia@andeslogistics.cl", "work"), cust_email("l.morales@gmail.com", "other")],
                    phones: vec![cust_phone("+56 2 1234 5678", "work")],
                    websites: vec![cust_site("https://andeslogistics.cl")],
                    socials: vec![cust_social("luciam", "twitter")],
                    address: Some(HsCustomerAddress {
                        line1: Some("Av. Providencia 1234".into()),
                        line2: None,
                        city: Some("Santiago".into()),
                        state: None,
                        postal_code: Some("7500572".into()),
                        country: Some("Chile".into()),
                    }),
                    organization: Some((2001, "Andes Logistics")),
                    created_days_ago: 220,
                    updated_days_ago: 3,
                    background: Some("Key account contact since 2024. Prefers Spanish, answers in English fine."),
                    age: Some("30-35"),
                    gender: Some("female"),
                    location: Some("Santiago, Chile"),
                    properties: vec![cust_prop(4101, "Pro"), cust_prop(4102, "120"), cust_prop(4103, "LATAM"), cust_prop(4104, "Alex Rivera")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3002,
                    first: "Mateo",
                    last: "Morales",
                    job_title: Some("Dispatcher"),
                    emails: vec![cust_email("mateo@andeslogistics.cl", "work")],
                    phones: vec![],
                    websites: vec![],
                    socials: vec![],
                    address: None,
                    organization: Some((2001, "Andes Logistics")),
                    created_days_ago: 180,
                    updated_days_ago: 10,
                    background: Some("Backup dispatcher; escalate to Lucía for billing topics."),
                    age: Some("25-30"),
                    gender: Some("male"),
                    location: Some("Valparaíso, Chile"),
                    properties: vec![cust_prop(4101, "Pro"), cust_prop(4102, "120"), cust_prop(4103, "LATAM")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3003,
                    first: "Sarah",
                    last: "Okafor",
                    job_title: Some("CTO"),
                    emails: vec![cust_email("sarah@brightpathedu.org", "work")],
                    phones: vec![cust_phone("+1 555 010 2233", "mobile")],
                    websites: vec![cust_site("https://brightpathedu.org")],
                    socials: vec![cust_social("sarahokafor", "linkedin")],
                    address: Some(HsCustomerAddress {
                        line1: Some("88 Kingsway".into()),
                        line2: None,
                        city: Some("London".into()),
                        state: None,
                        postal_code: Some("WC2B 6AA".into()),
                        country: Some("United Kingdom".into()),
                    }),
                    organization: Some((2002, "BrightPath Education")),
                    created_days_ago: 150,
                    updated_days_ago: 5,
                    background: Some("Technical decision maker. Loves detailed RFC-style answers."),
                    age: Some("35-40"),
                    gender: Some("female"),
                    location: Some("London, UK"),
                    properties: vec![cust_prop(4101, "Business"), cust_prop(4102, "45"), cust_prop(4103, "EMEA"), cust_prop(4104, "Alex Rivera")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3004,
                    first: "Daniel",
                    last: "Kim",
                    job_title: Some("Developer"),
                    emails: vec![cust_email("daniel.kim@brightpathedu.org", "work")],
                    phones: vec![],
                    websites: vec![],
                    socials: vec![],
                    address: None,
                    organization: Some((2002, "BrightPath Education")),
                    created_days_ago: 90,
                    updated_days_ago: 8,
                    background: None,
                    age: Some("25-30"),
                    gender: Some("male"),
                    location: Some("London, UK"),
                    properties: vec![cust_prop(4101, "Business"), cust_prop(4102, "45"), cust_prop(4103, "EMEA")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3005,
                    first: "Emma",
                    last: "Lindqvist",
                    job_title: None,
                    emails: vec![cust_email("emma.lindqvist@nordicmail.se", "work")],
                    phones: vec![],
                    websites: vec![],
                    socials: vec![],
                    address: None,
                    organization: None,
                    created_days_ago: 60,
                    updated_days_ago: 12,
                    background: Some("Freelance consultant using the free plan."),
                    age: None,
                    gender: None,
                    location: Some("Stockholm, Sweden"),
                    properties: vec![cust_prop(4101, "Free"), cust_prop(4102, "1"), cust_prop(4103, "EMEA")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3006,
                    first: "Ravi",
                    last: "Sundaram",
                    job_title: Some("IT Admin"),
                    emails: vec![cust_email("ravi@pixelworks.in", "work")],
                    phones: vec![],
                    websites: vec![cust_site("https://pixelworks.in")],
                    socials: vec![],
                    address: None,
                    organization: None,
                    created_days_ago: 45,
                    updated_days_ago: 6,
                    background: Some("Runs IT for a 40-person studio; strong PowerShell user."),
                    age: Some("30-35"),
                    gender: Some("male"),
                    location: Some("Chennai, India"),
                    properties: vec![cust_prop(4101, "Pro"), cust_prop(4102, "40"), cust_prop(4103, "APAC")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3007,
                    first: "Chloe",
                    last: "Dubois",
                    job_title: Some("Finance Lead"),
                    emails: vec![cust_email("chloe@atelierfrance.fr", "work")],
                    phones: vec![],
                    websites: vec![],
                    socials: vec![],
                    address: None,
                    organization: None,
                    created_days_ago: 30,
                    updated_days_ago: 2,
                    background: Some("Invoices go to finance@atelierfrance.fr."),
                    age: Some("40-45"),
                    gender: Some("female"),
                    location: Some("Paris, France"),
                    properties: vec![cust_prop(4101, "Free"), cust_prop(4102, "8"), cust_prop(4103, "EMEA")],
                }),
                build_customer(CustomerSpec {
                    remote_id: 3008,
                    first: "Hiro",
                    last: "Tanaka",
                    job_title: Some("Product Manager"),
                    emails: vec![cust_email("hiro.tanaka@sakuradata.jp", "work")],
                    phones: vec![],
                    websites: vec![],
                    socials: vec![],
                    address: None,
                    organization: None,
                    created_days_ago: 20,
                    updated_days_ago: 1,
                    background: Some("Evaluating the API for an internal tool."),
                    age: Some("30-35"),
                    gender: Some("male"),
                    location: Some("Tokyo, Japan"),
                    properties: vec![cust_prop(4101, "Business"), cust_prop(4102, "60"), cust_prop(4103, "APAC")],
                }),
            ],
            conversations: b.conversations,
            threads: b.threads,
            ratings,
            user_statuses: vec![
                HsUserStatus {
                    user_id: 1001,
                    email_status: Some("active".into()),
                    email_updated_at: Some(days_ago(1, 8, 0)),
                    chat_status: Some("active".into()),
                    mailbox_statuses: serde_json::json!({ "201": "assign", "202": "assign" }),
                },
                HsUserStatus {
                    user_id: 1002,
                    email_status: Some("active".into()),
                    email_updated_at: Some(days_ago(1, 8, 0)),
                    chat_status: Some("assign".into()),
                    mailbox_statuses: serde_json::json!({ "201": "assign" }),
                },
                HsUserStatus {
                    user_id: 1003,
                    email_status: Some("away".into()),
                    email_updated_at: Some(days_ago(2, 9, 0)),
                    chat_status: Some("unavailable".into()),
                    mailbox_statuses: serde_json::json!({}),
                },
            ],
            doc_collections: vec![
                HsDocCollection {
                    remote_id: 801,
                    name: "Getting Started".into(),
                    slug: Some("getting-started".into()),
                    description: Some("First steps with Zylker: workspace setup, team invites and your first report.".into()),
                    visibility: Some("public".into()),
                    article_count: Some(5),
                },
                HsDocCollection {
                    remote_id: 802,
                    name: "Billing & Account".into(),
                    slug: Some("billing-account".into()),
                    description: Some("Plans, seats, invoices, VAT and receipts.".into()),
                    visibility: Some("public".into()),
                    article_count: Some(4),
                },
            ],
            doc_categories: vec![
                HsDocCategory { remote_id: 851, collection_id: 801, name: "Setup".into(), slug: Some("setup".into()), sort_order: Some(1) },
                HsDocCategory { remote_id: 852, collection_id: 801, name: "Team".into(), slug: Some("team".into()), sort_order: Some(2) },
                HsDocCategory { remote_id: 853, collection_id: 802, name: "Invoices".into(), slug: Some("invoices".into()), sort_order: Some(1) },
                HsDocCategory { remote_id: 854, collection_id: 802, name: "Plans & Seats".into(), slug: Some("plans-seats".into()), sort_order: Some(2) },
            ],
            doc_articles: vec![
                HsDocArticle {
                    remote_id: 8011,
                    collection_id: 801,
                    category_id: Some(851),
                    number: Some(101),
                    slug: Some("first-report".into()),
                    name: "Creating your first report".into(),
                    status: Some("published".into()),
                    views: Some(320),
                    text: Some("To create your first report, open the Reports section and click \"New report\". Pick a data source (dispatch, conversations or exports), then drag the fields you want onto the canvas. Schedules are optional: without one the report only runs on demand via the \"Run now\" button. When you add a schedule, the times shown follow your workspace timezone, which you can change under Settings → Workspace.".into()),
                    created_at: Some(days_ago(180, 10, 30)),
                    updated_at: Some(days_ago(12, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8012,
                    collection_id: 801,
                    category_id: Some(851),
                    number: Some(102),
                    slug: Some("schedule-timezones".into()),
                    name: "Understanding schedule timezones".into(),
                    status: Some("published".into()),
                    views: Some(540),
                    text: Some("Schedules store the UTC offset that was active when you last saved them. When daylight-saving time changes in your region, existing schedules keep the old offset and can fire an hour early or late. To fix this, open the schedule and re-save it once after the clock change — the new offset is stamped automatically. Recurring exports, dispatch reports and reminders all follow the same rule. If two reports behave differently after a clock change, check which one was re-saved most recently.".into()),
                    created_at: Some(days_ago(150, 10, 30)),
                    updated_at: Some(days_ago(3, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8013,
                    collection_id: 801,
                    category_id: Some(852),
                    number: Some(103),
                    slug: Some("inviting-teammates".into()),
                    name: "Inviting teammates and roles".into(),
                    status: Some("published".into()),
                    views: Some(610),
                    text: Some("Invite teammates from Settings → Team → Invite. The role dropdown decides what they can do: Editors can build, edit and schedule reports; Viewers can open any shared report but cannot edit anything. Viewer seats are unlimited on every plan — only editors count against your seat limit. Invitations expire after 7 days; simply resend to renew.".into()),
                    created_at: Some(days_ago(140, 10, 30)),
                    updated_at: Some(days_ago(20, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8014,
                    collection_id: 801,
                    category_id: Some(852),
                    number: Some(104),
                    slug: Some("sharing-views".into()),
                    name: "Sharing views with a link".into(),
                    status: Some("draft".into()),
                    views: Some(45),
                    text: Some("DRAFT — not yet published. Share any saved view via the \"Share\" menu → \"Copy link\". Links inherit the visibility of the view: public links work for anyone with the URL, while restricted links require signing in. If a shared link returns 404, the view was most likely deleted or its visibility changed after the link was created.".into()),
                    created_at: Some(days_ago(10, 10, 30)),
                    updated_at: Some(days_ago(2, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8021,
                    collection_id: 802,
                    category_id: Some(853),
                    number: Some(201),
                    slug: Some("receipts-and-invoices".into()),
                    name: "Downloading receipts and invoices".into(),
                    status: Some("published".into()),
                    views: Some(480),
                    text: Some("Every charge generates a receipt. Download receipts any time under Billing → Invoices, using the download icon on each row. Invoices include your billing profile address and, when set, your VAT number. If you need a receipt re-sent by email, contact billing and include the month — re-sending is instant. Accounting exports (CSV) are also available from the same screen for annual filing.".into()),
                    created_at: Some(days_ago(200, 10, 30)),
                    updated_at: Some(days_ago(30, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8022,
                    collection_id: 802,
                    category_id: Some(853),
                    number: Some(202),
                    slug: Some("vat-numbers".into()),
                    name: "Adding a VAT number to invoices".into(),
                    status: Some("published".into()),
                    views: Some(260),
                    text: Some("Add your VAT number under Billing → Billing profile. New invoices include it automatically. We can also re-issue past invoices with the VAT number for your annual filing — contact billing with the range of months you need. The number must include your country prefix (for example FR40303265045 for France).".into()),
                    created_at: Some(days_ago(120, 10, 30)),
                    updated_at: Some(days_ago(24, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8023,
                    collection_id: 802,
                    category_id: Some(854),
                    number: Some(203),
                    slug: Some("plan-limits".into()),
                    name: "Plan limits: editors vs viewers".into(),
                    status: Some("published".into()),
                    views: Some(720),
                    text: Some("Seat limits count editors only. Viewers are unlimited on every plan. On the Growth plan you have 10 editor seats; Studio has 25. You can check current usage under Billing → Plan. Downgrading does not delete extra editors — they become read-only until seats free up again.".into()),
                    created_at: Some(days_ago(210, 10, 30)),
                    updated_at: Some(days_ago(15, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8024,
                    collection_id: 802,
                    category_id: Some(854),
                    number: Some(204),
                    slug: Some("changing-plans".into()),
                    name: "Upgrading or downgrading your plan".into(),
                    status: Some("published".into()),
                    views: Some(190),
                    text: Some("Plan changes take effect immediately and are prorated. Upgrading unlocks the extra editor seats right away. Downgrading keeps your data intact; features outside the new plan become read-only. Failed charges put the account in a \"past due\" state for 14 days before any restriction — update the card under Billing → Payment method and we retry automatically within an hour.".into()),
                    created_at: Some(days_ago(110, 10, 30)),
                    updated_at: Some(days_ago(18, 10, 30)),
                },
                HsDocArticle {
                    remote_id: 8025,
                    collection_id: 801,
                    category_id: Some(851),
                    number: Some(105),
                    slug: Some("sso-troubleshooting".into()),
                    name: "SSO troubleshooting checklist (internal)".into(),
                    status: Some("internal".into()),
                    views: Some(95),
                    text: Some("INTERNAL — for support agents only. SSO redirect loops are almost always a stale session cookie: 1) Ask the customer to try an incognito window. 2) If incognito works, clear cookies for app.zylker.io. 3) If it persists, check the identity provider logs for a failed assertion and verify the ACS URL has no trailing slash. 4) Escalate to platform engineering only after steps 1-3 with the SAML trace attached. Never share this checklist with customers directly.".into()),
                    created_at: Some(days_ago(90, 10, 30)),
                    updated_at: Some(days_ago(4, 10, 30)),
                },
            ],
        }
    }
}

struct WorldBuilder {
    conversations: Vec<HsConversation>,
    threads: Vec<HsThread>,
    conv_num: i64,
    thread_id: i64,
}

impl WorldBuilder {
    fn new() -> Self {
        Self {
            conversations: Vec::new(),
            threads: Vec::new(),
            conv_num: 5000,
            thread_id: 10_000,
        }
    }

    fn conversation(&mut self, opts: ConvSpec<'_>) -> i64 {
        let remote_id = self.conv_num + 100_000;
        let number = self.conv_num + 1;
        self.conv_num = number;
        let c = HsConversation {
            remote_id,
            number,
            kind: Some("email".into()),
            source_type: None,
            source_via: None,
            subject: Some(opts.subject.into()),
            preview: Some(opts.preview.into()),
            status: opts.status.into(),
            state: Some("published".into()),
            mailbox_id: opts.mailbox_id,
            assignee_id: opts.assignee_id,
            assignee_type: opts.assignee_id.map(|_| "user".into()),
            assigned_team_id: None,
            customer_id: opts.customer_id,
            priority: None,
            created_at: Some(days_ago(opts.created_days_ago, 9, 12)),
            updated_at: Some(days_ago((opts.created_days_ago - 1).max(0), 14, 40)),
            closed_at: opts.closed_days_ago.map(|d| days_ago(d, 16, 5)),
            snoozed_until: opts.snoozed_until,
            thread_count: 0,
            merged_into: None,
            tags: opts.tags.iter().map(|t| (*t).into()).collect(),
            custom_fields: Vec::new(),
        };
        self.conversations.push(c);
        remote_id
    }

    fn chat_session(&mut self, opts: ChatSpec<'_>) -> i64 {
        let remote_id = self.conv_num + 100_000;
        let number = self.conv_num + 1;
        self.conv_num = number;
        let start = days_ago(opts.created_days_ago, opts.start_hour, 0);
        let end = opts.closed_after_min.map(|m| minutes_after(&start, m));
        let c = HsConversation {
            remote_id,
            number,
            kind: Some("chat".into()),
            source_type: Some("chat".into()),
            source_via: Some("beacon".into()),
            subject: Some(opts.subject.into()),
            preview: Some(opts.preview.into()),
            status: opts.status.into(),
            state: Some("published".into()),
            mailbox_id: opts.mailbox_id,
            assignee_id: opts.assignee_id,
            assignee_type: opts.assignee_id.map(|_| "user".into()),
            assigned_team_id: None,
            customer_id: opts.customer_id,
            priority: None,
            created_at: Some(start.clone()),
            updated_at: Some(end.clone().unwrap_or(start)),
            closed_at: end,
            snoozed_until: None,
            thread_count: 0,
            merged_into: None,
            tags: opts.tags.iter().map(|t| (*t).into()).collect(),
            custom_fields: Vec::new(),
        };
        self.conversations.push(c);
        remote_id
    }

    fn thread(
        &mut self,
        conv_remote_id: i64,
        kind: &str,
        body: &str,
        created_at: String,
        customer_id: Option<i64>,
        user_id: Option<i64>,
    ) {
        self.thread_id += 1;
        self.threads.push(HsThread {
            remote_id: self.thread_id,
            conversation_id: conv_remote_id,
            kind: kind.into(),
            status: None,
            state: Some("published".into()),
            body: Some(body.into()),
            created_by_customer_id: customer_id,
            created_by_user_id: user_id,
            assigned_to_id: None,
            created_at: Some(created_at),
            ..Default::default()
        });
        let count = self
            .threads
            .iter()
            .filter(|t| t.conversation_id == conv_remote_id)
            .count() as i64;
        if let Some(conv) = self
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conv_remote_id)
        {
            conv.thread_count = count;
        }
    }
}

struct ConvSpec<'a> {
    subject: &'a str,
    preview: &'a str,
    mailbox_id: i64,
    customer_id: i64,
    status: &'a str,
    tags: &'a [&'a str],
    assignee_id: Option<i64>,
    created_days_ago: i64,
    closed_days_ago: Option<i64>,
    snoozed_until: Option<String>,
}

struct ChatSpec<'a> {
    subject: &'a str,
    preview: &'a str,
    mailbox_id: i64,
    customer_id: i64,
    status: &'a str,
    tags: &'a [&'a str],
    assignee_id: Option<i64>,
    created_days_ago: i64,
    start_hour: u32,
    closed_after_min: Option<i64>,
}

struct CustomerSpec<'a> {
    remote_id: i64,
    first: &'a str,
    last: &'a str,
    job_title: Option<&'a str>,
    emails: Vec<HsCustomerEmail>,
    phones: Vec<HsCustomerPhone>,
    websites: Vec<HsCustomerWebsite>,
    socials: Vec<HsCustomerSocialProfile>,
    address: Option<HsCustomerAddress>,
    organization: Option<(i64, &'a str)>,
    created_days_ago: i64,
    updated_days_ago: i64,
    background: Option<&'a str>,
    age: Option<&'a str>,
    gender: Option<&'a str>,
    location: Option<&'a str>,
    properties: Vec<HsCustomerPropertyValue>,
}

fn build_customer(s: CustomerSpec<'_>) -> HsCustomer {
    HsCustomer {
        remote_id: s.remote_id,
        first_name: Some(s.first.into()),
        last_name: Some(s.last.into()),
        email: s.emails.first().and_then(|e| e.value.clone()),
        organization: s.organization.map(|(_, name)| name.into()),
        job_title: s.job_title.map(String::from),
        phone: s.phones.first().and_then(|p| p.value.clone()),
        created_at: Some(days_ago(s.created_days_ago, 10, 30)),
        updated_at: Some(days_ago(s.updated_days_ago, 10, 30)),
        photo_url: None,
        organization_id: s.organization.map(|(id, _)| id),
        background: s.background.map(String::from),
        age: s.age.map(String::from),
        gender: s.gender.map(String::from),
        location: s.location.map(String::from),
        emails: s.emails,
        phones: s.phones,
        websites: s.websites,
        social_profiles: s.socials,
        address: s.address,
        properties: s.properties,
    }
}

fn cust_email(value: &str, kind: &str) -> HsCustomerEmail {
    HsCustomerEmail {
        value: Some(value.into()),
        kind: Some(kind.into()),
    }
}

fn cust_phone(value: &str, kind: &str) -> HsCustomerPhone {
    HsCustomerPhone {
        value: Some(value.into()),
        kind: Some(kind.into()),
    }
}

fn cust_site(value: &str) -> HsCustomerWebsite {
    HsCustomerWebsite {
        value: Some(value.into()),
    }
}

fn cust_social(value: &str, kind: &str) -> HsCustomerSocialProfile {
    HsCustomerSocialProfile {
        value: Some(value.into()),
        kind: Some(kind.into()),
    }
}

fn cust_prop(def: i64, value: &str) -> HsCustomerPropertyValue {
    HsCustomerPropertyValue {
        definition_remote_id: Some(def),
        key: None,
        name: None,
        value: Some(value.into()),
    }
}

impl FakeHelpScoutProvider {
    #[must_use]
    pub fn new_demo() -> Self {
        Self {
            world: Mutex::new(FakeWorld::demo()),
        }
    }

    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            world: Mutex::new(FakeWorld::default()),
        }
    }

    fn lock_world(&self) -> std::sync::MutexGuard<'_, FakeWorld> {
        self.world.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn submit_rating_in_world(
        &self,
        conversation_remote_id: i64,
        rating: &str,
        comments: Option<&str>,
    ) -> Option<HsRating> {
        let mut guard = self.lock_world();
        let world: &mut FakeWorld = &mut guard;
        let conv = world
            .conversations
            .iter()
            .find(|c| c.remote_id == conversation_remote_id)?;
        let next_id = world.ratings.iter().map(|r| r.remote_id).max().unwrap_or(0) + 1;
        let customer_name = world
            .customers
            .iter()
            .find(|c| c.remote_id == conv.customer_id)
            .map(|c| {
                format!(
                    "{} {}",
                    c.first_name.clone().unwrap_or_default(),
                    c.last_name.clone().unwrap_or_default()
                )
                .trim()
                .to_string()
            });
        let row = HsRating {
            remote_id: next_id,
            conversation_id: Some(conv.remote_id),
            thread_id: None,
            rating: Some(rating.to_string()),
            comment: comments.map(str::to_string),
            customer_id: Some(conv.customer_id),
            customer_name,
            user_id: conv.assignee_id,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
        };
        world.ratings.push(row.clone());
        Some(row)
    }

    pub fn customer_replies_in_world(&self, conversation_remote_id: i64, text: &str) {
        let mut guard = self.lock_world();
        let world: &mut FakeWorld = &mut guard;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let next_id = world.threads.iter().map(|t| t.remote_id).max().unwrap_or(0) + 1;
        let Some(conv) = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_remote_id)
        else {
            return;
        };
        let customer_id = conv.customer_id;
        world.threads.push(HsThread {
            remote_id: next_id,
            conversation_id: conversation_remote_id,
            kind: "customer".into(),
            status: None,
            state: Some("published".into()),
            body: Some(text.into()),
            created_by_customer_id: Some(customer_id),
            created_by_user_id: None,
            assigned_to_id: None,
            created_at: Some(now.clone()),
            ..Default::default()
        });
        let count = world
            .threads
            .iter()
            .filter(|t| t.conversation_id == conversation_remote_id)
            .count() as i64;
        let conv = world
            .conversations
            .iter_mut()
            .find(|c| c.remote_id == conversation_remote_id)
            .expect("conversation presence checked above");
        conv.thread_count = count;
        conv.updated_at = Some(now);
        if conv.status == "pending" || conv.status == "closed" {
            conv.status = "active".into();
        }
    }
}

#[async_trait::async_trait]
impl HelpScoutProvider for FakeHelpScoutProvider {
    fn kind(&self) -> &'static str {
        "fake"
    }

    async fn get_me(&self) -> Result<HsUser> {
        let world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
        Ok(world.me.clone())
    }

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        let world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
        Ok(world.mailboxes.clone())
    }

    async fn list_users(&self) -> Result<Vec<HsUser>> {
        let world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
        Ok(world.users.clone())
    }

    async fn list_teams(&self) -> Result<Vec<HsTeam>> {
        let world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
        Ok(world.teams.clone())
    }

    async fn list_tags(&self) -> Result<Vec<HsTag>> {
        let world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
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

    async fn get_company_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        let world = self.lock_world();
        let in_range = world
            .conversations
            .iter()
            .filter(|c| {
                c.created_at
                    .as_deref()
                    .is_some_and(|t| t >= start && t <= end)
            })
            .count();
        Ok(Some(HsReportRow {
            key: "hs_company_overall".into(),
            name: "Help Scout Company Overall".into(),
            source: "helpscout".into(),
            data: serde_json::json!({
                "totalConversations": in_range,
                "startDate": start,
                "endDate": end
            }),
        }))
    }

    async fn get_conversations_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        let world = self.lock_world();
        let in_range: Vec<&HsConversation> = world
            .conversations
            .iter()
            .filter(|c| {
                c.created_at
                    .as_deref()
                    .is_some_and(|t| t >= start && t <= end)
            })
            .collect();
        let by_status = |s: &str| in_range.iter().filter(|c| c.status == s).count();
        Ok(Some(HsReportRow {
            key: "hs_conversations_overall".into(),
            name: "Help Scout Conversations Overall".into(),
            source: "helpscout".into(),
            data: serde_json::json!({
                "totalConversations": in_range.len(),
                "byStatus": {
                    "active": by_status("active"),
                    "closed": by_status("closed"),
                    "pending": by_status("pending")
                }
            }),
        }))
    }

    async fn get_happiness_ratings_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        let world = self.lock_world();
        let in_range: Vec<&HsRating> = world
            .ratings
            .iter()
            .filter(|r| {
                r.created_at
                    .as_deref()
                    .is_some_and(|t| t >= start && t <= end)
            })
            .collect();
        let count = |v: &str| {
            in_range
                .iter()
                .filter(|r| r.rating.as_deref() == Some(v))
                .count()
        };
        Ok(Some(HsReportRow {
            key: "hs_happiness_ratings".into(),
            name: "Help Scout Happiness Ratings".into(),
            source: "helpscout".into(),
            data: serde_json::json!({
                "great": count("great"),
                "okay": count("okay"),
                "notGood": count("not-good")
            }),
        }))
    }

    async fn get_productivity_overall_report(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Option<HsReportRow>> {
        let world = self.lock_world();
        let replies = world
            .threads
            .iter()
            .filter(|t| {
                t.kind == "reply"
                    && t.created_at
                        .as_deref()
                        .is_some_and(|c| c >= start && c <= end)
            })
            .count();
        Ok(Some(HsReportRow {
            key: "hs_productivity_overall".into(),
            name: "Help Scout Productivity Overall".into(),
            source: "helpscout".into(),
            data: serde_json::json!({ "repliesSent": replies }),
        }))
    }

    fn reset(&self) {
        let mut world = self
            .world
            .lock()
            .expect("FakeHelpScoutProvider mutex poisoned");
        *world = FakeWorld::demo();
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
        url: &str,
        events: &[String],
        _secret: &str,
        _label: &str,
    ) -> Result<i64> {
        let mut world = self.lock_world();
        let next_id = world
            .webhooks
            .iter()
            .map(|w| w.remote_id)
            .max()
            .unwrap_or(0)
            + 1;
        world.webhooks.push(HsWebhookConfig {
            remote_id: next_id,
            url: url.to_string(),
            events: events.to_vec(),
            status: "enabled".into(),
        });
        Ok(next_id)
    }

    async fn delete_webhook(&self, remote_id: i64) -> Result<bool> {
        let mut world = self.lock_world();
        let before = world.webhooks.len();
        world.webhooks.retain(|w| w.remote_id != remote_id);
        Ok(world.webhooks.len() < before)
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

    async fn list_doc_categories(&self, collection_id: i64) -> Result<Vec<HsDocCategory>> {
        let world = self.lock_world();
        Ok(world
            .doc_categories
            .iter()
            .filter(|c| c.collection_id == collection_id)
            .cloned()
            .collect())
    }

    async fn list_doc_articles(&self, collection_id: i64) -> Result<Vec<HsDocArticle>> {
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        input: CreateConversationInput,
    ) -> Result<ConversationCreated> {
        let mut guard = self.lock_world();
        let world: &mut FakeWorld = &mut guard;
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let next_conv_remote = world
            .conversations
            .iter()
            .map(|c| c.remote_id)
            .max()
            .unwrap_or(0)
            + 1;
        let next_thread_remote = world.threads.iter().map(|t| t.remote_id).max().unwrap_or(0) + 1;
        let next_number = world
            .conversations
            .iter()
            .map(|c| c.number)
            .max()
            .unwrap_or(0)
            + 1;

        let status = input
            .status
            .clone()
            .filter(|s| matches!(s.as_str(), "active" | "pending" | "closed" | "spam"))
            .unwrap_or_else(|| "active".to_string());

        let conv = HsConversation {
            remote_id: next_conv_remote,
            number: next_number,
            kind: Some("email".into()),
            source_type: None,
            source_via: None,
            subject: Some(input.subject.clone()),
            preview: Some(input.body.chars().take(200).collect::<String>()),
            status,
            state: Some("published".into()),
            mailbox_id: input.mailbox_id,
            assignee_id: None,
            assignee_type: None,
            assigned_team_id: None,
            customer_id: input.customer_id,
            priority: None,
            created_at: Some(now.clone()),
            updated_at: Some(now.clone()),
            closed_at: None,
            snoozed_until: None,
            thread_count: 1,
            merged_into: None,
            tags: input.tags.clone(),
            custom_fields: Vec::new(),
        };
        world.conversations.push(conv);

        let thread = HsThread {
            remote_id: next_thread_remote,
            conversation_id: next_conv_remote,
            kind: "customer".into(),
            status: None,
            state: Some("published".into()),
            body: Some(input.body.clone()),
            created_by_customer_id: Some(input.customer_id),
            created_by_user_id: None,
            assigned_to_id: None,
            created_at: Some(now.clone()),
            ..Default::default()
        };
        world.threads.push(thread);

        Ok(ConversationCreated {
            conversation_id: next_conv_remote,
            number: next_number,
            thread_id: next_thread_remote,
        })
    }

    fn submit_rating(
        &self,
        conversation_remote_id: i64,
        rating: &str,
        comments: Option<&str>,
    ) -> Option<HsRating> {
        self.submit_rating_in_world(conversation_remote_id, rating, comments)
    }

    fn customer_replies(&self, conversation_remote_id: i64, text: &str) {
        self.customer_replies_in_world(conversation_remote_id, text);
    }

    async fn update_tags(&self, conversation_id: i64, tags: Vec<String>) -> Result<bool> {
        let mut guard = self.lock_world();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
        let Some(t) = world.threads.iter_mut().find(|t| t.remote_id == thread_id) else {
            return Err(not_found_remote("PATCH"));
        };
        t.state = Some("published".into());
        let _ = conversation_id;
        Ok(true)
    }

    async fn delete_thread_schedule(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        let mut guard = self.lock_world();
        let world: &mut FakeWorld = &mut guard;
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
        let world: &mut FakeWorld = &mut guard;
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
        thread_id: i64,
        attachment_id: i64,
    ) -> Result<Option<crate::helpscout::AttachmentData>> {
        let guard = self.lock_world();
        let world: &FakeWorld = &guard;
        let t = world.threads.iter().find(|x| {
            x.conversation_id == conversation_id
                && x.attachments.iter().any(|a| a.remote_id == attachment_id)
        });
        let att = t.and_then(|t| t.attachments.iter().find(|a| a.remote_id == attachment_id));
        let Some(att) = att else {
            return Ok(None);
        };
        let content = format!(
            "Simulated attachment content for {} (thread {})\nGenerated by FakeHelpScoutProvider.\n",
            att.filename.clone().unwrap_or_default(),
            thread_id
        );
        Ok(Some(crate::helpscout::AttachmentData {
            data: content.into_bytes(),
            mime_type: att.mime_type.clone(),
            filename: att.filename.clone(),
        }))
    }

    async fn get_routing_configuration(
        &self,
        mailbox_id: i64,
    ) -> Result<Option<serde_json::Value>> {
        let guard = self.lock_world();
        let world: &FakeWorld = &guard;
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

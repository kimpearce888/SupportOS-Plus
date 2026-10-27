//! Hot-swappable provider cell (v1.4.0 R3, defect D2).
//!
//! The provider used to be built exactly once at boot from whatever
//! credentials resolved then: saving credentials mid-session changed
//! nothing until the next restart, which is the "restart ritual" every
//! v1.1.x owner had to perform. The `ProviderHandle` fixes the plumbing
//! underneath: boot wraps the chosen provider in one shared handle that
//! (a) the sync engine, the worker host and the loopback listener all
//! hold for the whole process, and (b) can swap its inner provider
//! in-place when credentials change mid-session. No Arc holder ever has
//! to be rebuilt, so activation is safe from the UI thread while a sync
//! is mid-flight — the running engine simply sees the new provider on
//! its next call.
//!
//! The watermark-integrity rules around a swap live in
//! [`crate::sync_engine::on_provider_kind_transition`] (R2): the swap
//! itself stays mechanism-only, the state machine policy is applied by
//! the caller that performs the activation.

use std::sync::Arc;
use std::sync::RwLock;

use crate::error::Result;
use crate::helpscout::{
    AttachmentData, ConversationCreated, ConversationPatch, ConversationQuery,
    CreateConversationInput, CreateThreadInput, CustomerQuery, HelpScoutProvider, HsBeaconChat,
    HsConversation, HsCustomer, HsDocArticle, HsDocCategory, HsDocCollection, HsField, HsFolder,
    HsMailbox, HsOrganization, HsPropertyDef, HsRating, HsReportRow, HsSavedReply, HsTag, HsTeam,
    HsThread, HsUser, HsUserStatus, HsWebhookConfig, HsWorkflow, Page, ThreadCreated,
};

/// What the handle currently serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Live Help Scout credentials resolved; the real provider serves
    /// every call.
    Real,
    /// No credentials: the inert provider serves nothing, and nothing
    /// is ever faked.
    Empty,
}

impl ProviderKind {
    /// The `provider_kind` setting value (also what `HelpScoutProvider::kind`
    /// reports through the handle).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Empty => "empty",
        }
    }

    /// Parse the stored `provider_kind` setting. `None` means the
    /// setting has never been written (a genuinely fresh database).
    #[must_use]
    pub fn from_setting(value: Option<&str>) -> Option<Self> {
        match value {
            Some("real") => Some(Self::Real),
            Some("empty") => Some(Self::Empty),
            // Unknown or never-written values are treated as absent;
            // boot decides the kind from the credentials, not the row.
            _ => None,
        }
    }
}

struct Inner {
    kind: ProviderKind,
    provider: Arc<dyn HelpScoutProvider>,
    /// The concrete real provider when [`Inner::kind`] is `Real`
    /// (settings and the loopback OAuth exchange need it).
    real: Option<Arc<crate::helpscout_real::RealHelpScoutProvider>>,
}

/// One shared, swappable provider for the whole process.
///
/// Constructed by [`crate::build_boot_services`]-equivalent boot code
/// (see `spp_shell::build_boot_services`); every long-lived consumer
/// (sync engine, worker manager, loopback listener, settings page) holds
/// the same `Arc<ProviderHandle>`, so [`ProviderHandle::activate_real`]
/// and [`ProviderHandle::deactivate`] rewire all of them at once.
pub struct ProviderHandle {
    inner: RwLock<Inner>,
}

impl ProviderHandle {
    /// Wrap a live provider.
    #[must_use]
    pub fn new_real(real: Arc<crate::helpscout_real::RealHelpScoutProvider>) -> Self {
        let provider: Arc<dyn HelpScoutProvider> = real.clone();
        Self {
            inner: RwLock::new(Inner {
                kind: ProviderKind::Real,
                provider,
                real: Some(real),
            }),
        }
    }

    /// Wrap the inert empty provider.
    #[must_use]
    pub fn new_empty() -> Self {
        Self {
            inner: RwLock::new(Inner {
                kind: ProviderKind::Empty,
                provider: Arc::new(crate::helpscout::EmptyHelpScoutProvider::new()),
                real: None,
            }),
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Inner> {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Inner> {
        self.inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Which provider the handle currently serves.
    #[must_use]
    pub fn kind(&self) -> ProviderKind {
        self.read().kind
    }

    /// True when live Help Scout credentials are active this session.
    #[must_use]
    pub fn is_real(&self) -> bool {
        self.kind() == ProviderKind::Real
    }

    /// The concrete real provider, `None` while the empty provider
    /// serves. Settings (connect test, Connection Doctor) and the
    /// loopback OAuth exchange call this at request time, so a
    /// mid-session activation is picked up without any rebuild.
    #[must_use]
    pub fn real(&self) -> Option<Arc<crate::helpscout_real::RealHelpScoutProvider>> {
        self.read().real.clone()
    }

    /// A clone of the current provider as the trait object the engine
    /// and workers were constructed with.
    #[must_use]
    pub fn provider(&self) -> Arc<dyn HelpScoutProvider> {
        Arc::clone(&self.read().provider)
    }

    /// Swap in a freshly built real provider (credentials were saved or
    /// changed mid-session). Returns the kind that was previously
    /// active so the caller can apply the
    /// [`crate::sync_engine::on_provider_kind_transition`] policy.
    pub fn activate_real(
        &self,
        real: Arc<crate::helpscout_real::RealHelpScoutProvider>,
    ) -> ProviderKind {
        let previous = self.kind();
        let provider: Arc<dyn HelpScoutProvider> = real.clone();
        {
            let mut guard = self.write();
            guard.kind = ProviderKind::Real;
            guard.provider = provider;
            guard.real = Some(real);
        }
        tracing::info!(previous = previous.as_str(), "provider activated (real)");
        previous
    }

    /// Swap back to the inert empty provider (credentials were
    /// cleared). Returns the previously active kind.
    pub fn deactivate(&self) -> ProviderKind {
        let previous = self.kind();
        {
            let mut guard = self.write();
            guard.kind = ProviderKind::Empty;
            guard.provider = Arc::new(crate::helpscout::EmptyHelpScoutProvider::new());
            guard.real = None;
        }
        tracing::info!(previous = previous.as_str(), "provider deactivated (empty)");
        previous
    }
}

// Every trait method delegates to whatever provider the handle is
// currently serving; that is the entire point of the indirection.
#[async_trait::async_trait]
impl HelpScoutProvider for ProviderHandle {
    fn kind(&self) -> &'static str {
        // Read once to avoid a torn read between slot and kind.
        self.read().kind.as_str()
    }

    async fn get_me(&self) -> Result<HsUser> {
        self.provider().get_me().await
    }

    async fn list_mailboxes(&self) -> Result<Vec<HsMailbox>> {
        self.provider().list_mailboxes().await
    }

    async fn list_users(&self) -> Result<Vec<HsUser>> {
        self.provider().list_users().await
    }

    async fn list_teams(&self) -> Result<Vec<HsTeam>> {
        self.provider().list_teams().await
    }

    async fn list_tags(&self) -> Result<Vec<HsTag>> {
        self.provider().list_tags().await
    }

    async fn list_conversations(&self, query: &ConversationQuery) -> Result<Page<HsConversation>> {
        self.provider().list_conversations(query).await
    }

    async fn list_customers(&self, query: &CustomerQuery) -> Result<Page<HsCustomer>> {
        self.provider().list_customers(query).await
    }

    async fn list_beacon_chats(&self) -> Result<Vec<HsBeaconChat>> {
        self.provider().list_beacon_chats().await
    }

    async fn list_docs(&self) -> Result<Vec<HsDocArticle>> {
        self.provider().list_docs().await
    }

    async fn list_ratings(&self) -> Result<Vec<HsRating>> {
        self.provider().list_ratings().await
    }

    async fn get_rating(&self, rating_id: i64) -> Result<Option<HsRating>> {
        self.provider().get_rating(rating_id).await
    }

    async fn get_report(&self, key: &str, start: &str, end: &str) -> Result<Option<HsReportRow>> {
        self.provider().get_report(key, start, end).await
    }

    async fn list_folders(&self, mailbox_id: i64) -> Result<Vec<HsFolder>> {
        self.provider().list_folders(mailbox_id).await
    }

    async fn list_inbox_fields(&self, mailbox_id: i64) -> Result<Vec<HsField>> {
        self.provider().list_inbox_fields(mailbox_id).await
    }

    async fn list_saved_replies(&self, mailbox_id: i64) -> Result<Vec<HsSavedReply>> {
        self.provider().list_saved_replies(mailbox_id).await
    }

    async fn list_workflows(&self) -> Result<Vec<HsWorkflow>> {
        self.provider().list_workflows().await
    }

    async fn list_webhooks(&self) -> Result<Vec<HsWebhookConfig>> {
        self.provider().list_webhooks().await
    }

    async fn create_webhook(
        &self,
        url: &str,
        events: &[String],
        secret: &str,
        label: &str,
    ) -> Result<i64> {
        self.provider()
            .create_webhook(url, events, secret, label)
            .await
    }

    async fn delete_webhook(&self, remote_id: i64) -> Result<bool> {
        self.provider().delete_webhook(remote_id).await
    }

    async fn list_customer_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        self.provider().list_customer_property_definitions().await
    }

    async fn list_organization_property_definitions(&self) -> Result<Vec<HsPropertyDef>> {
        self.provider()
            .list_organization_property_definitions()
            .await
    }

    async fn list_organizations(&self) -> Result<Vec<HsOrganization>> {
        self.provider().list_organizations().await
    }

    async fn get_conversation(&self, conversation_id: i64) -> Result<Option<HsConversation>> {
        self.provider().get_conversation(conversation_id).await
    }

    async fn list_threads(&self, conversation_id: i64) -> Result<Vec<HsThread>> {
        self.provider().list_threads(conversation_id).await
    }

    async fn get_customer(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        self.provider().get_customer(customer_id).await
    }

    // Audit fix (C4): delegate the channel sub-resource fetch — without
    // this the trait default answered Ok(None) and no customer ever got
    // their emails/phones backfilled.
    async fn get_customer_channels(&self, customer_id: i64) -> Result<Option<HsCustomer>> {
        self.provider().get_customer_channels(customer_id).await
    }

    async fn get_user_status(&self, user_id: i64) -> Result<Option<HsUserStatus>> {
        self.provider().get_user_status(user_id).await
    }

    async fn list_system_users(&self) -> Result<Vec<HsUser>> {
        self.provider().list_system_users().await
    }

    async fn list_doc_collections(&self) -> Result<Vec<HsDocCollection>> {
        self.provider().list_doc_collections().await
    }

    async fn list_doc_categories(&self, collection_id: String) -> Result<Vec<HsDocCategory>> {
        self.provider().list_doc_categories(collection_id).await
    }

    async fn list_doc_articles(&self, collection_id: String) -> Result<Vec<HsDocArticle>> {
        self.provider().list_doc_articles(collection_id).await
    }

    async fn get_doc_article(&self, remote_id: String) -> Result<Option<HsDocArticle>> {
        self.provider().get_doc_article(remote_id).await
    }

    async fn upsert_doc_article_draft(
        &self,
        input: &crate::helpscout::DocArticleDraftInput,
    ) -> Result<String> {
        self.provider().upsert_doc_article_draft(input).await
    }

    async fn publish_doc_article(&self, remote_id: String, name: &str, text: &str) -> Result<()> {
        self.provider()
            .publish_doc_article(remote_id, name, text)
            .await
    }

    async fn create_reply_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        self.provider().create_reply_thread(input).await
    }

    async fn create_note_thread(&self, input: CreateThreadInput) -> Result<ThreadCreated> {
        self.provider().create_note_thread(input).await
    }

    async fn update_conversation(
        &self,
        conversation_id: i64,
        patch: ConversationPatch,
    ) -> Result<bool> {
        self.provider()
            .update_conversation(conversation_id, patch)
            .await
    }

    async fn create_conversation(
        &self,
        input: CreateConversationInput,
    ) -> Result<ConversationCreated> {
        self.provider().create_conversation(input).await
    }

    fn submit_rating(
        &self,
        conversation_remote_id: i64,
        rating: &str,
        comments: Option<&str>,
    ) -> Option<HsRating> {
        self.provider()
            .submit_rating(conversation_remote_id, rating, comments)
    }

    fn customer_replies(&self, conversation_remote_id: i64, text: &str) {
        self.provider()
            .customer_replies(conversation_remote_id, text);
    }

    fn reset(&self) {
        self.provider().reset();
    }

    async fn update_tags(&self, conversation_id: i64, tags: Vec<String>) -> Result<bool> {
        self.provider().update_tags(conversation_id, tags).await
    }

    async fn update_custom_fields(
        &self,
        conversation_id: i64,
        fields: Vec<(i64, Option<String>)>,
    ) -> Result<bool> {
        self.provider()
            .update_custom_fields(conversation_id, fields)
            .await
    }

    async fn snooze_conversation(
        &self,
        conversation_id: i64,
        snoozed_until: String,
        unsnooze_on_customer_reply: bool,
    ) -> Result<bool> {
        self.provider()
            .snooze_conversation(conversation_id, snoozed_until, unsnooze_on_customer_reply)
            .await
    }

    async fn unsnooze_conversation(&self, conversation_id: i64) -> Result<bool> {
        self.provider().unsnooze_conversation(conversation_id).await
    }

    async fn schedule_thread(
        &self,
        conversation_id: i64,
        thread_id: i64,
        scheduled_for: String,
        unschedule_on_customer_reply: bool,
    ) -> Result<bool> {
        self.provider()
            .schedule_thread(
                conversation_id,
                thread_id,
                scheduled_for,
                unschedule_on_customer_reply,
            )
            .await
    }

    async fn publish_scheduled_thread(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        self.provider()
            .publish_scheduled_thread(conversation_id, thread_id)
            .await
    }

    fn docs_configured(&self) -> bool {
        self.provider().docs_configured()
    }

    // Saved-reply CRUD, workflow runs and attachment downloads are
    // enqueued by workers through the handle (operations library, post-
    // sync attachment passes). Without these delegations the trait's
    // rejecting defaults fired even with a live provider connected.
    async fn create_saved_reply(&self, mailbox_id: i64, name: &str, text: &str) -> Result<i64> {
        self.provider()
            .create_saved_reply(mailbox_id, name, text)
            .await
    }

    async fn update_saved_reply(
        &self,
        mailbox_id: i64,
        saved_reply_id: i64,
        name: &str,
        text: &str,
    ) -> Result<()> {
        self.provider()
            .update_saved_reply(mailbox_id, saved_reply_id, name, text)
            .await
    }

    async fn delete_saved_reply(&self, mailbox_id: i64, saved_reply_id: i64) -> Result<()> {
        self.provider()
            .delete_saved_reply(mailbox_id, saved_reply_id)
            .await
    }

    async fn delete_thread_schedule(&self, conversation_id: i64, thread_id: i64) -> Result<bool> {
        self.provider()
            .delete_thread_schedule(conversation_id, thread_id)
            .await
    }

    async fn run_workflow(&self, workflow_id: i64, conversation_id: i64) -> Result<bool> {
        self.provider()
            .run_workflow(workflow_id, conversation_id)
            .await
    }

    async fn get_attachment_data(
        &self,
        conversation_id: i64,
        thread_id: i64,
        attachment_id: i64,
    ) -> Result<Option<AttachmentData>> {
        self.provider()
            .get_attachment_data(conversation_id, thread_id, attachment_id)
            .await
    }

    async fn get_routing_configuration(
        &self,
        mailbox_id: i64,
    ) -> Result<Option<serde_json::Value>> {
        self.provider().get_routing_configuration(mailbox_id).await
    }

    async fn ping(&self) -> Result<bool> {
        self.provider().ping().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> crate::db::DbHandle {
        let dir = std::env::temp_dir().join(format!(
            "spp-provider-handle-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let mut conn = crate::db::open(&dir.join("handle.db")).expect("open db");
        crate::bootstrap::apply_all(&mut conn).expect("migrations");
        crate::db::DbHandle::new(conn)
    }

    fn real_provider(
        db: &crate::db::DbHandle,
    ) -> Arc<crate::helpscout_real::RealHelpScoutProvider> {
        use crate::helpscout_real::HsCredentials;
        let creds = HsCredentials {
            client_id: "cid".into(),
            client_secret: "secret".into(),
            ..HsCredentials::default()
        };
        Arc::new(crate::helpscout_real::RealHelpScoutProvider::new(
            db.clone(),
            creds,
        ))
    }

    #[test]
    fn empty_handle_reports_empty_and_serves_nothing() {
        let handle = ProviderHandle::new_empty();
        assert_eq!(handle.kind(), ProviderKind::Empty);
        assert!(!handle.is_real());
        assert!(handle.real().is_none());
        assert_eq!(handle.kind().as_str(), "empty");
        let provider: Arc<dyn HelpScoutProvider> = Arc::new(ProviderHandle::new_empty());
        assert_eq!(HelpScoutProvider::kind(provider.as_ref()), "empty");
    }

    #[test]
    fn activate_real_swaps_every_view_in_place() {
        let db = temp_db();
        let handle = Arc::new(ProviderHandle::new_empty());
        let provider_view: Arc<dyn HelpScoutProvider> = Arc::clone(&handle) as _;
        assert_eq!(HelpScoutProvider::kind(provider_view.as_ref()), "empty");

        let previous = handle.activate_real(real_provider(&db));
        assert_eq!(previous, ProviderKind::Empty);

        // The Arc held since before the swap sees the new provider.
        assert_eq!(HelpScoutProvider::kind(provider_view.as_ref()), "real");
        assert_eq!(handle.kind(), ProviderKind::Real);
        assert!(handle.real().is_some());
        assert!(handle.is_real());
    }

    #[test]
    fn deactivate_swaps_back_and_reports_the_previous_kind() {
        let db = temp_db();
        let handle = ProviderHandle::new_real(real_provider(&db));
        assert_eq!(handle.kind(), ProviderKind::Real);
        let previous = handle.deactivate();
        assert_eq!(previous, ProviderKind::Real);
        assert_eq!(handle.kind(), ProviderKind::Empty);
        assert!(handle.real().is_none());
    }

    #[test]
    fn setting_kind_round_trips() {
        assert_eq!(
            ProviderKind::from_setting(Some("real")),
            Some(ProviderKind::Real)
        );
        assert_eq!(
            ProviderKind::from_setting(Some("empty")),
            Some(ProviderKind::Empty)
        );
        assert_eq!(ProviderKind::from_setting(Some("demo")), None);
        assert_eq!(ProviderKind::from_setting(None), None);
    }

    #[test]
    fn real_handle_delegates_docs_configured_false_without_key() {
        let db = temp_db();
        let handle = ProviderHandle::new_real(real_provider(&db));
        assert!(
            !handle.docs_configured(),
            "no docs key in these test credentials"
        );
    }
}

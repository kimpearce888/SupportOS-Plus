use super::*;

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

    pub(super) fn lock_world(&self) -> std::sync::MutexGuard<'_, InertWorld> {
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

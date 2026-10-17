use rusqlite::Connection;

use crate::error::Result;

pub fn apply_m040(conn: &Connection) -> Result<()> {
    create_missing_reference_tables(conn)?;
    add_missing_reference_columns(conn)?;
    create_missing_reference_indexes(conn)?;
    seed_metric_definitions(conn)?;
    let _ = conn.execute("UPDATE app_state SET schema_version = 40 WHERE id = 1", []);
    Ok(())
}

fn create_missing_reference_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS system_users (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            remote_id        INTEGER UNIQUE NOT NULL,
            first_name       TEXT,
            last_name        TEXT,
            initials         TEXT,
            timezone         TEXT,
            role             TEXT,
            remote_created_at TEXT,
            remote_updated_at TEXT,
            raw_json         TEXT,
            last_synced_at   TEXT,
            deleted_at       TEXT
        );

        CREATE TABLE IF NOT EXISTS team_members (
            team_id INTEGER NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            PRIMARY KEY (team_id, user_id)
        );

        CREATE TABLE IF NOT EXISTS organization_properties (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            organization_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
            definition_id   INTEGER NOT NULL REFERENCES organization_property_definitions(id),
            value           TEXT,
            UNIQUE (organization_id, definition_id)
        );

        -- FK ADAPTED: reference targets threads(id); the port's thread mirror
        -- table is conversation_threads.
        CREATE TABLE IF NOT EXISTS thread_participants (
            thread_id       INTEGER NOT NULL REFERENCES conversation_threads(id) ON DELETE CASCADE,
            person_type     TEXT NOT NULL,
            person_local_id INTEGER,
            name            TEXT,
            email           TEXT,
            role            TEXT,
            PRIMARY KEY (thread_id, person_type, person_local_id, role)
        );

        -- FK ADAPTED: reference targets threads(id) (see above).
        CREATE TABLE IF NOT EXISTS thread_recipients (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            thread_id INTEGER NOT NULL REFERENCES conversation_threads(id) ON DELETE CASCADE,
            email     TEXT NOT NULL,
            type      TEXT NOT NULL CHECK (type IN ('to','cc','bcc'))
        );
        CREATE INDEX IF NOT EXISTS idx_thread_recipients_thread ON thread_recipients(thread_id);

        CREATE TABLE IF NOT EXISTS routing_configurations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            mailbox_local_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
            raw_json         TEXT,
            last_synced_at   TEXT
        );

        -- ---- migration 003_ai_knowledge.ts --------------------------------
        CREATE TABLE IF NOT EXISTS ai_drafts (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            run_id          INTEGER REFERENCES ai_runs(id),
            content         TEXT NOT NULL,
            mode            TEXT DEFAULT 'standard',
            model           TEXT,
            prompt_version  TEXT,
            state           TEXT DEFAULT 'generated',
            verification    TEXT,
            sources         TEXT,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT NOT NULL DEFAULT 'ai_generated'
        );
        CREATE INDEX IF NOT EXISTS idx_ai_drafts_conversation ON ai_drafts(conversation_id);

        CREATE TABLE IF NOT EXISTS ai_verifications (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            draft_id          INTEGER NOT NULL REFERENCES ai_drafts(id) ON DELETE CASCADE,
            run_id            INTEGER REFERENCES ai_runs(id),
            verified          INTEGER NOT NULL,
            unsupported_claims TEXT,
            missing_questions TEXT,
            internal_leakage  TEXT,
            conflicts         TEXT,
            warnings          TEXT,
            created_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS ai_feedback (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            draft_id         INTEGER NOT NULL REFERENCES ai_drafts(id) ON DELETE CASCADE,
            original_content TEXT,
            final_content    TEXT,
            edit_distance    INTEGER,
            was_sent         INTEGER DEFAULT 0,
            sent_at          TEXT,
            rating_after     TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- Queried directly by the port's issues.rs (impact) and copilot.rs
        -- (starter-questions) routes — previously a phantom table there.
        CREATE TABLE IF NOT EXISTS known_issue_conversations (
            known_issue_id  INTEGER NOT NULL REFERENCES known_issues(id) ON DELETE CASCADE,
            conversation_id INTEGER NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
            linked_at       TEXT NOT NULL DEFAULT (datetime('now')),
            source          TEXT DEFAULT 'human',
            PRIMARY KEY (known_issue_id, conversation_id)
        );
        -- migration 016 performance index (reference-exact).
        CREATE INDEX IF NOT EXISTS idx_known_issue_conversations_conversation
            ON known_issue_conversations(conversation_id);

        CREATE TABLE IF NOT EXISTS known_issue_refs (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            known_issue_id INTEGER NOT NULL REFERENCES known_issues(id) ON DELETE CASCADE,
            system         TEXT,
            reference_id   TEXT,
            url            TEXT,
            title          TEXT,
            status         TEXT,
            notes          TEXT
        );

        CREATE TABLE IF NOT EXISTS report_snapshots (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            report_key   TEXT NOT NULL,
            params       TEXT,
            generated_at TEXT NOT NULL,
            data_version TEXT,
            result       TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_report_snapshots ON report_snapshots(report_key, generated_at DESC);

        CREATE TABLE IF NOT EXISTS daily_metrics (
            metric_key TEXT NOT NULL,
            date       TEXT NOT NULL,
            value      REAL NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (metric_key, date)
        );

        CREATE TABLE IF NOT EXISTS metric_definitions (
            key          TEXT PRIMARY KEY,
            name         TEXT NOT NULL,
            description  TEXT,
            formula      TEXT,
            source       TEXT NOT NULL DEFAULT 'local',
            limitations  TEXT
        );

        CREATE TABLE IF NOT EXISTS release_events (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            version     TEXT,
            occurred_at TEXT NOT NULL,
            notes       TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- ---- migration 005_interaction_intelligence.ts ---------------------
        CREATE TABLE IF NOT EXISTS client_behavior_observations (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id      INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
            conversation_id  INTEGER REFERENCES conversations(id) ON DELETE SET NULL,
            thread_local_id  INTEGER,
            dimension        TEXT NOT NULL,
            value            TEXT NOT NULL,
            confidence       TEXT NOT NULL DEFAULT 'low',
            evidence_excerpt TEXT,
            source           TEXT NOT NULL DEFAULT 'heuristic',
            observed_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance       TEXT NOT NULL DEFAULT 'heuristic'
        );
        CREATE INDEX IF NOT EXISTS idx_client_observations_customer
            ON client_behavior_observations(customer_id, dimension);
        CREATE INDEX IF NOT EXISTS idx_client_observations_conversation
            ON client_behavior_observations(conversation_id);
        CREATE UNIQUE INDEX IF NOT EXISTS idx_client_observations_unique
            ON client_behavior_observations(conversation_id, dimension, source);

        CREATE TABLE IF NOT EXISTS client_behavior_baselines (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id       INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
            dimension         TEXT NOT NULL,
            typical_value     TEXT NOT NULL,
            confidence        TEXT NOT NULL DEFAULT 'low',
            observation_count INTEGER NOT NULL DEFAULT 0,
            last_observed     TEXT,
            profile_version   INTEGER NOT NULL DEFAULT 1,
            updated_at        TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (customer_id, dimension)
        );

        CREATE TABLE IF NOT EXISTS client_communication_preferences (
            id                    INTEGER PRIMARY KEY AUTOINCREMENT,
            customer_id           INTEGER NOT NULL REFERENCES customers(id) ON DELETE CASCADE,
            preference            TEXT NOT NULL,
            evidence_count        INTEGER NOT NULL DEFAULT 1,
            first_observed        TEXT,
            last_observed         TEXT,
            confidence            TEXT NOT NULL DEFAULT 'low',
            origin                TEXT NOT NULL DEFAULT 'ai_inferred',
            human_override_value  TEXT,
            human_override_reason TEXT,
            overridden_at         TEXT,
            provenance            TEXT NOT NULL DEFAULT 'ai_generated',
            UNIQUE (customer_id, preference)
        );
        CREATE INDEX IF NOT EXISTS idx_client_preferences_customer
            ON client_communication_preferences(customer_id);

        -- ---- migration 012_m2_collaboration.ts ------------------------------
        CREATE TABLE IF NOT EXISTS notification_prefs (
            notification_type TEXT PRIMARY KEY,
            enabled           INTEGER NOT NULL DEFAULT 1,
            updated_at        TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS side_thread_participants (
            side_thread_id         INTEGER NOT NULL REFERENCES side_threads(id) ON DELETE CASCADE,
            user_local_id          INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            added_by_user_local_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
            added_at               TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (side_thread_id, user_local_id)
        );
        CREATE INDEX IF NOT EXISTS idx_side_thread_participants_user
            ON side_thread_participants(user_local_id);

        CREATE TABLE IF NOT EXISTS side_thread_mentions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            side_thread_id  INTEGER NOT NULL REFERENCES side_threads(id) ON DELETE CASCADE,
            message_id      INTEGER NOT NULL REFERENCES side_thread_messages(id) ON DELETE CASCADE,
            user_local_id   INTEGER REFERENCES users(id) ON DELETE CASCADE,
            team_local_id   INTEGER REFERENCES teams(id) ON DELETE CASCADE,
            created_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_side_thread_mentions_user
            ON side_thread_mentions(user_local_id, created_at DESC);
        CREATE INDEX IF NOT EXISTS idx_side_thread_mentions_team
            ON side_thread_mentions(team_local_id);

        -- ---- migration 013_m3_copilot_attributes.ts -------------------------
        CREATE TABLE IF NOT EXISTS copilot_sessions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            title           TEXT NOT NULL,
            conversation_id INTEGER REFERENCES conversations(id) ON DELETE CASCADE,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_copilot_sessions_conversation
            ON copilot_sessions(conversation_id, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_copilot_sessions_updated
            ON copilot_sessions(updated_at DESC);

        CREATE TABLE IF NOT EXISTS copilot_messages (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL REFERENCES copilot_sessions(id) ON DELETE CASCADE,
            role       TEXT NOT NULL,
            content    TEXT NOT NULL,
            citations  TEXT,
            tool_name  TEXT,
            tool_calls INTEGER NOT NULL DEFAULT 0,
            latency_ms INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_copilot_messages_session
            ON copilot_messages(session_id, created_at);

        -- ---- migration 014_m4_intelligence_workspace.ts ---------------------
        CREATE TABLE IF NOT EXISTS incident_releases (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id   INTEGER NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
            version_label TEXT NOT NULL,
            notes         TEXT,
            released_at   TEXT,
            correlation   TEXT,
            created_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_incident_releases_incident ON incident_releases(incident_id);

        CREATE TABLE IF NOT EXISTS incident_notes (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id        INTEGER NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
            author_user_local_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
            body               TEXT NOT NULL,
            created_at         TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_incident_notes_incident
            ON incident_notes(incident_id, created_at);

        CREATE TABLE IF NOT EXISTS incident_events (
            id                 INTEGER PRIMARY KEY AUTOINCREMENT,
            incident_id        INTEGER NOT NULL REFERENCES incidents(id) ON DELETE CASCADE,
            event_type         TEXT NOT NULL,
            actor_user_local_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
            occurred_at        TEXT NOT NULL DEFAULT (datetime('now')),
            detail             TEXT,
            source             TEXT NOT NULL DEFAULT 'local',
            dedup_key          TEXT NOT NULL UNIQUE
        );
        CREATE INDEX IF NOT EXISTS idx_incident_events_incident
            ON incident_events(incident_id, occurred_at);

        CREATE TABLE IF NOT EXISTS knowledge_doc_usage (
            document_id  INTEGER PRIMARY KEY REFERENCES knowledge_documents(id) ON DELETE CASCADE,
            search_hits  INTEGER NOT NULL DEFAULT 0,
            last_hit_at  TEXT
        );

        -- ---- migration 015_m5_quality_translation_reports.ts ----------------
        CREATE TABLE IF NOT EXISTS translation_cache (
            cache_key        TEXT PRIMARY KEY,
            source_lang      TEXT NOT NULL,
            target_lang      TEXT NOT NULL,
            purpose          TEXT NOT NULL DEFAULT 'general'
                CHECK (purpose IN ('customer_inbound','agent_draft','general')),
            source_text      TEXT NOT NULL,
            translated_text  TEXT NOT NULL,
            model            TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_translation_cache_created
            ON translation_cache(created_at DESC);

        -- ---- migration 016_m6_graph_coaching_memory.ts ----------------------
        CREATE TABLE IF NOT EXISTS coaching_reviews (
            conversation_id INTEGER PRIMARY KEY REFERENCES conversations(id) ON DELETE CASCADE,
            draft_sha256    TEXT NOT NULL,
            draft_excerpt   TEXT NOT NULL,
            deterministic   TEXT NOT NULL DEFAULT '{}',
            ai              TEXT,
            ai_run_id       INTEGER,
            draft_chars     INTEGER NOT NULL DEFAULT 0,
            draft_words     INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            provenance      TEXT NOT NULL DEFAULT 'deterministic_local'
        );",
    )?;
    Ok(())
}

fn add_missing_reference_columns(conn: &Connection) -> Result<()> {
    crate::saved_views::ensure_inbox_views_table(conn)?;
    crate::interaction_current::ensure_client_current_signals_table(conn)?;

    add(conn, "mailboxes", "remote_created_at", "TEXT")?;
    add(conn, "mailboxes", "remote_updated_at", "TEXT")?;
    add(conn, "mailboxes", "raw_json", "TEXT")?;
    add(conn, "mailboxes", "raw_json_hash", "TEXT")?;
    add(conn, "mailboxes", "last_seen_at", "TEXT")?;
    add(conn, "mailboxes", "last_synced_at", "TEXT")?;
    add(conn, "mailboxes", "deleted_at", "TEXT")?;

    add(conn, "users", "alternate_emails", "TEXT")?;
    add(conn, "users", "remote_created_at", "TEXT")?;
    add(conn, "users", "remote_updated_at", "TEXT")?;
    add(conn, "users", "raw_json", "TEXT")?;
    add(conn, "users", "raw_json_hash", "TEXT")?;
    add(conn, "users", "last_seen_at", "TEXT")?;
    add(conn, "users", "last_synced_at", "TEXT")?;
    add(conn, "users", "deleted_at", "TEXT")?;

    add(conn, "teams", "raw_json", "TEXT")?;
    add(conn, "teams", "remote_created_at", "TEXT")?;
    add(conn, "teams", "remote_updated_at", "TEXT")?;
    add(conn, "teams", "last_synced_at", "TEXT")?;
    add(conn, "teams", "deleted_at", "TEXT")?;

    add(conn, "tags", "remote_created_at", "TEXT")?;
    add(conn, "tags", "remote_updated_at", "TEXT")?;
    add(conn, "tags", "raw_json", "TEXT")?;
    add(conn, "tags", "last_seen_at", "TEXT")?;
    add(conn, "tags", "last_synced_at", "TEXT")?;
    add(conn, "tags", "deleted_at", "TEXT")?;

    add(conn, "customers", "photo_url", "TEXT")?;
    add(conn, "customers", "raw_json", "TEXT")?;
    add(conn, "customers", "raw_json_hash", "TEXT")?;
    add(conn, "customers", "remote_created_at", "TEXT")?;
    add(conn, "customers", "remote_updated_at", "TEXT")?;
    add(conn, "customers", "last_seen_at", "TEXT")?;
    add(conn, "customers", "last_synced_at", "TEXT")?;
    add(
        conn,
        "customers",
        "local_updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(conn, "customers", "background", "TEXT")?;
    add(conn, "customers", "age", "TEXT")?;
    add(conn, "customers", "gender", "TEXT")?;
    add(conn, "customers", "location", "TEXT")?;

    add(conn, "customer_addresses", "raw_json", "TEXT")?;

    add(conn, "oauth_tokens", "revoked", "INTEGER DEFAULT 0")?;

    add(conn, "conversations", "state", "TEXT DEFAULT 'published'")?;
    add(conn, "conversations", "type", "TEXT")?;
    add(conn, "conversations", "folder_local_id", "INTEGER")?;
    add(conn, "conversations", "assigned_team_local_id", "INTEGER")?;
    add(conn, "conversations", "closed_by", "INTEGER")?;
    add(conn, "conversations", "thread_count", "INTEGER DEFAULT 0")?;
    add(conn, "conversations", "is_unread", "INTEGER DEFAULT 0")?;
    add(conn, "conversations", "hs_url", "TEXT")?;
    add(conn, "conversations", "remote_created_at", "TEXT")?;
    add(conn, "conversations", "remote_updated_at", "TEXT")?;
    add(
        conn,
        "conversations",
        "local_updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(conn, "conversations", "last_seen_at", "TEXT")?;
    add(conn, "conversations", "last_synced_at", "TEXT")?;
    add(conn, "conversations", "first_activity_at", "TEXT")?;
    add(conn, "conversations", "last_activity_at", "TEXT")?;
    add(conn, "conversations", "raw_json", "TEXT")?;
    add(conn, "conversations", "raw_json_hash", "TEXT")?;
    add(conn, "conversations", "source_type", "TEXT")?;
    add(conn, "conversations", "source_via", "TEXT")?;
    add(conn, "conversations", "last_system_response_at", "TEXT")?;
    add(conn, "conversations", "last_note_at", "TEXT")?;
    add(conn, "conversations", "last_status_change_at", "TEXT")?;
    add(conn, "conversations", "last_assignment_change_at", "TEXT")?;
    add(conn, "conversations", "last_tag_change_at", "TEXT")?;
    add(conn, "conversations", "last_custom_field_change_at", "TEXT")?;
    add(
        conn,
        "conversations",
        "activity_history_complete",
        "INTEGER NOT NULL DEFAULT 0",
    )?;

    add(
        conn,
        "conversation_threads",
        "state",
        "TEXT DEFAULT 'published'",
    )?;
    add(conn, "conversation_threads", "body_html", "TEXT")?;
    add(conn, "conversation_threads", "from_name", "TEXT")?;
    add(conn, "conversation_threads", "from_email", "TEXT")?;
    add(conn, "conversation_threads", "assigned_to_type", "TEXT")?;
    add(conn, "conversation_threads", "assigned_to_id", "INTEGER")?;
    add(
        conn,
        "conversation_threads",
        "saved_reply_local_id",
        "INTEGER",
    )?;
    add(conn, "conversation_threads", "action_type", "TEXT")?;
    add(conn, "conversation_threads", "action_text", "TEXT")?;
    add(conn, "conversation_threads", "to_list", "TEXT")?;
    add(conn, "conversation_threads", "cc_list", "TEXT")?;
    add(conn, "conversation_threads", "bcc_list", "TEXT")?;
    add(conn, "conversation_threads", "remote_created_at", "TEXT")?;
    add(conn, "conversation_threads", "remote_updated_at", "TEXT")?;
    add(
        conn,
        "conversation_threads",
        "local_created_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "conversation_threads",
        "local_updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(conn, "conversation_threads", "last_synced_at", "TEXT")?;
    add(conn, "conversation_threads", "raw_json", "TEXT")?;
    add(conn, "conversation_threads", "raw_json_hash", "TEXT")?;
    add(
        conn,
        "conversation_threads",
        "embedding_state",
        "TEXT DEFAULT 'not_indexed'",
    )?;

    add(conn, "ai_runs", "type", "TEXT NOT NULL DEFAULT 'analysis'")?;
    add(conn, "ai_runs", "conversation_id", "INTEGER")?;
    add(conn, "ai_runs", "status", "TEXT NOT NULL DEFAULT 'queued'")?;
    add(conn, "ai_runs", "input_refs", "TEXT")?;
    add(conn, "ai_runs", "error", "TEXT")?;
    add(conn, "ai_runs", "latency_ms", "INTEGER")?;
    add(conn, "ai_runs", "token_usage", "TEXT")?;
    add(conn, "ai_runs", "started_at", "TEXT")?;
    add(conn, "ai_runs", "completed_at", "TEXT")?;
    add(
        conn,
        "ai_runs",
        "provenance",
        "TEXT NOT NULL DEFAULT 'ai_generated'",
    )?;

    add(
        conn,
        "issue_clusters",
        "customer_count",
        "INTEGER DEFAULT 0",
    )?;
    add(
        conn,
        "issue_clusters",
        "created_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "issue_clusters",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "issue_clusters",
        "provenance",
        "TEXT DEFAULT 'ai_generated'",
    )?;

    add(
        conn,
        "issue_cluster_conversations",
        "assigned_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;

    add(conn, "known_issues", "first_seen_at", "TEXT")?;
    add(conn, "known_issues", "last_seen_at", "TEXT")?;
    add(
        conn,
        "known_issues",
        "conversation_count",
        "INTEGER DEFAULT 0",
    )?;

    add(
        conn,
        "known_issue_links",
        "linked_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(conn, "known_issue_links", "source", "TEXT DEFAULT 'human'")?;

    add(
        conn,
        "automation_rules",
        "conditions",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add(conn, "automation_rules", "priority", "INTEGER DEFAULT 100")?;
    add(
        conn,
        "automation_rules",
        "requires_approval",
        "INTEGER DEFAULT 1",
    )?;
    add(conn, "automation_rules", "last_run_at", "TEXT")?;
    add(conn, "automation_rules", "run_count", "INTEGER DEFAULT 0")?;

    add(
        conn,
        "automation_runs",
        "triggered_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "automation_runs",
        "status",
        "TEXT NOT NULL DEFAULT 'completed'",
    )?;
    add(conn, "automation_runs", "detail", "TEXT")?;

    add(conn, "interaction_overrides", "ai_value", "TEXT")?;
    add(
        conn,
        "interaction_overrides",
        "created_by",
        "TEXT NOT NULL DEFAULT 'user'",
    )?;
    add(
        conn,
        "interaction_overrides",
        "active",
        "INTEGER NOT NULL DEFAULT 1",
    )?;

    add(conn, "friction_scores", "customer_local_id", "INTEGER")?;
    add(conn, "friction_scores", "kind", "TEXT")?;
    add(conn, "friction_scores", "severity", "TEXT")?;
    add(
        conn,
        "friction_scores",
        "evidence",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add(conn, "friction_scores", "detail", "TEXT")?;
    add(
        conn,
        "friction_scores",
        "computed_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;

    add(conn, "docs", "preview", "TEXT")?;
    add(conn, "docs", "words", "INTEGER")?;
    add(conn, "docs", "content_hash", "TEXT")?;

    add(conn, "docs_collections", "visibility", "TEXT")?;
    add(conn, "docs_collections", "article_count", "INTEGER")?;

    add(
        conn,
        "docs_chunks",
        "embedding_attempts",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add(
        conn,
        "conversation_chunks",
        "embedding_attempts",
        "INTEGER NOT NULL DEFAULT 0",
    )?;

    add(conn, "saved_segments", "description", "TEXT")?;
    add(
        conn,
        "saved_segments",
        "version",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add(
        conn,
        "saved_segments",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;

    add(conn, "activity_events", "thread_local_id", "INTEGER")?;
    add(
        conn,
        "activity_events",
        "source",
        "TEXT NOT NULL DEFAULT 'sync'",
    )?;
    add(conn, "activity_events", "metadata", "TEXT")?;

    add(conn, "inbox_views", "description", "TEXT")?;
    add(
        conn,
        "inbox_views",
        "sort_order",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add(conn, "inbox_views", "folder", "TEXT")?;
    add(conn, "inbox_views", "version", "INTEGER NOT NULL DEFAULT 1")?;
    add(
        conn,
        "inbox_views",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;

    add(conn, "notifications", "title", "TEXT NOT NULL DEFAULT ''")?;
    add(conn, "notifications", "body", "TEXT")?;
    add(conn, "notifications", "actor_user_local_id", "INTEGER")?;
    add(conn, "notifications", "conversation_number", "INTEGER")?;
    add(conn, "notifications", "customer_local_id", "INTEGER")?;
    add(conn, "notifications", "issue_id", "INTEGER")?;
    add(conn, "notifications", "campaign_id", "INTEGER")?;
    add(conn, "notifications", "job_id", "INTEGER")?;
    add(conn, "notifications", "side_thread_id", "INTEGER")?;
    add(
        conn,
        "notifications",
        "dedup_key",
        "TEXT NOT NULL DEFAULT ''",
    )?;

    add(conn, "side_threads", "title", "TEXT NOT NULL DEFAULT ''")?;
    add(conn, "side_threads", "team_local_id", "INTEGER")?;
    add(
        conn,
        "side_threads",
        "status",
        "TEXT NOT NULL DEFAULT 'open'",
    )?;
    add(
        conn,
        "side_threads",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(conn, "side_threads", "resolved_at", "TEXT")?;

    add(conn, "incidents", "owner_user_local_id", "INTEGER")?;
    add(conn, "incidents", "feature", "TEXT")?;
    add(conn, "incidents", "internal_explanation", "TEXT")?;
    add(conn, "incidents", "customer_safe_explanation", "TEXT")?;
    add(conn, "incidents", "known_cause", "TEXT")?;
    add(conn, "incidents", "workaround", "TEXT")?;
    add(conn, "incidents", "resolution", "TEXT")?;
    add(conn, "incidents", "started_at", "TEXT")?;
    add(
        conn,
        "incidents",
        "provenance",
        "TEXT NOT NULL DEFAULT 'human_local'",
    )?;

    add(conn, "custom_object_types", "description", "TEXT")?;
    add(conn, "custom_object_types", "deleted_at", "TEXT")?;
    add(
        conn,
        "custom_object_types",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "custom_object_types",
        "provenance",
        "TEXT NOT NULL DEFAULT 'human_local'",
    )?;
    add(
        conn,
        "custom_object_fields",
        "label",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add(
        conn,
        "custom_object_fields",
        "sort_order",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add(
        conn,
        "custom_objects",
        "search_text",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add(
        conn,
        "custom_objects",
        "provenance",
        "TEXT NOT NULL DEFAULT 'human_local'",
    )?;
    add(conn, "custom_object_links", "note", "TEXT")?;

    add(conn, "knowledge_gap_candidates", "dedup_key", "TEXT")?;
    add(
        conn,
        "knowledge_gap_candidates",
        "evidence_conversation_ids",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add(
        conn,
        "knowledge_gap_candidates",
        "related_document_ids",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    add(conn, "knowledge_gap_candidates", "detail", "TEXT")?;
    add(
        conn,
        "knowledge_gap_candidates",
        "decided_by_user_local_id",
        "INTEGER",
    )?;
    add(
        conn,
        "knowledge_gap_candidates",
        "updated_at",
        "TEXT NOT NULL DEFAULT (datetime('now'))",
    )?;
    add(
        conn,
        "knowledge_gap_candidates",
        "provenance",
        "TEXT NOT NULL DEFAULT 'deterministic_local'",
    )?;

    add(
        conn,
        "post_resolution_qa",
        "deterministic",
        "TEXT NOT NULL DEFAULT '{}'",
    )?;
    add(conn, "post_resolution_qa", "ai", "TEXT")?;
    add(conn, "post_resolution_qa", "ai_run_id", "INTEGER")?;
    add(conn, "post_resolution_qa", "recomputed_at", "TEXT")?;
    add(
        conn,
        "post_resolution_qa",
        "provenance",
        "TEXT NOT NULL DEFAULT 'deterministic_local'",
    )?;

    add(
        conn,
        "customer_memory",
        "origin",
        "TEXT DEFAULT 'conversation'",
    )?;
    add(conn, "customer_memory", "first_seen_at", "TEXT")?;
    add(conn, "customer_memory", "last_seen_at", "TEXT")?;
    add(
        conn,
        "customer_memory",
        "confidence",
        "TEXT DEFAULT 'unknown'",
    )?;
    add(
        conn,
        "customer_memory",
        "provenance",
        "TEXT DEFAULT 'ai_generated'",
    )?;
    add(
        conn,
        "customer_memory",
        "kind",
        "TEXT NOT NULL DEFAULT 'fact'",
    )?;

    Ok(())
}

fn create_missing_reference_indexes(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
         CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
         CREATE INDEX IF NOT EXISTS idx_organizations_name ON organizations(name);
         CREATE INDEX IF NOT EXISTS idx_customers_remote ON customers(remote_id);
         CREATE INDEX IF NOT EXISTS idx_customers_name ON customers(last_name, first_name);
         CREATE INDEX IF NOT EXISTS idx_saved_replies_name ON saved_replies(name);
         CREATE INDEX IF NOT EXISTS idx_customer_properties_def ON customer_properties(definition_id);

         -- conversations (001 + 007 + 011); customer_local_id→customer_id adapted.
         CREATE INDEX IF NOT EXISTS idx_conversations_status ON conversations(status);
         CREATE INDEX IF NOT EXISTS idx_conversations_number ON conversations(number);
         CREATE INDEX IF NOT EXISTS idx_conversations_remote_updated ON conversations(remote_updated_at);
         CREATE INDEX IF NOT EXISTS idx_conversations_last_activity ON conversations(last_activity_at DESC);
         CREATE INDEX IF NOT EXISTS idx_conversations_first_response ON conversations(first_response_at);
         CREATE INDEX IF NOT EXISTS idx_conversations_waiting_since ON conversations(customer_waiting_since);
         CREATE INDEX IF NOT EXISTS idx_conversations_last_customer ON conversations(last_customer_reply_at);
         CREATE INDEX IF NOT EXISTS idx_conversations_priority ON conversations(supportos_priority);
         CREATE INDEX IF NOT EXISTS idx_conversations_state ON conversations(supportos_state_id);
         CREATE INDEX IF NOT EXISTS idx_conversations_status_change ON conversations(last_status_change_at);
         CREATE INDEX IF NOT EXISTS idx_conversations_type ON conversations(type);
         CREATE INDEX IF NOT EXISTS idx_conversations_source_via ON conversations(source_via);
         CREATE INDEX IF NOT EXISTS idx_conversations_customer_status ON conversations(customer_id, status);
         CREATE INDEX IF NOT EXISTS idx_conversations_customer_created
            ON conversations(customer_id, remote_created_at DESC);

         -- threads (001): remote-created + embedding state (conversation/type
         -- indexes already exist under the port's idx_conv_threads_* names).
         CREATE INDEX IF NOT EXISTS idx_threads_remote_created ON conversation_threads(remote_created_at);
         CREATE INDEX IF NOT EXISTS idx_threads_embedding ON conversation_threads(embedding_state);

         -- 016 performance indexes, adapted to the port's link-table names.
         CREATE INDEX IF NOT EXISTS idx_issue_cluster_conversations_conversation
            ON issue_cluster_conversations(conversation_id);

         -- notifications (012); target_user_local_id→target_user_id adapted.
         CREATE INDEX IF NOT EXISTS idx_notifications_target_unread
            ON notifications(target_user_id, read_at);
         CREATE INDEX IF NOT EXISTS idx_notifications_created ON notifications(created_at DESC);
         CREATE INDEX IF NOT EXISTS idx_notifications_conversation ON notifications(conversation_id);

         -- side_threads (012)
         CREATE INDEX IF NOT EXISTS idx_side_threads_conversation
            ON side_threads(conversation_id, updated_at DESC);
         CREATE INDEX IF NOT EXISTS idx_side_threads_status ON side_threads(status, updated_at DESC);

         -- incidents (014)
         CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents(status, severity);
         CREATE INDEX IF NOT EXISTS idx_incidents_updated ON incidents(updated_at DESC);

         -- friction_findings (015) shape on the port's friction_scores table
         CREATE INDEX IF NOT EXISTS idx_friction_findings_kind ON friction_scores(kind, severity);
         CREATE INDEX IF NOT EXISTS idx_friction_findings_customer ON friction_scores(customer_local_id);

         -- knowledge_candidates (015) shape on the port's gap-candidates table
         CREATE INDEX IF NOT EXISTS idx_knowledge_candidates_kind
            ON knowledge_gap_candidates(kind, status);
         CREATE INDEX IF NOT EXISTS idx_knowledge_candidates_status
            ON knowledge_gap_candidates(status, updated_at DESC);

         -- docs_articles (007) indexes on the port's docs table
         CREATE INDEX IF NOT EXISTS idx_docs_articles_collection ON docs(collection_local_id);
         CREATE INDEX IF NOT EXISTS idx_docs_articles_status ON docs(status);",
    )?;

    try_create_unique_index(
        conn,
        "idx_notifications_dedup",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_notifications_dedup ON notifications(dedup_key)",
        "CREATE INDEX IF NOT EXISTS idx_notifications_dedup ON notifications(dedup_key)",
    )?;
    try_create_unique_index(
        conn,
        "idx_knowledge_gap_candidates_dedup",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_gap_candidates_dedup
            ON knowledge_gap_candidates(dedup_key)",
        "CREATE INDEX IF NOT EXISTS idx_knowledge_gap_candidates_dedup
            ON knowledge_gap_candidates(dedup_key)",
    )?;
    try_create_unique_index(
        conn,
        "idx_friction_findings_conv_kind",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_friction_findings_conv_kind
            ON friction_scores(conversation_id, kind)",
        "CREATE INDEX IF NOT EXISTS idx_friction_findings_conv_kind
            ON friction_scores(conversation_id, kind)",
    )?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_audit_log_conversation
            ON audit_log(conversation_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_audit_log_action
            ON audit_log(action, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_application_errors_timestamp
            ON application_errors(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_outreach_events_campaign_event
            ON outreach_events(campaign_id, event, at DESC);
        CREATE INDEX IF NOT EXISTS idx_outreach_recipients_state
            ON outreach_recipients(campaign_id, state);
        CREATE INDEX IF NOT EXISTS idx_activity_events_dedup
            ON activity_events(dedup_key);",
    )?;
    let conv_has_dnc: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name = 'deleted_at'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if conv_has_dnc {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_conversations_deleted_at
                ON conversations(deleted_at) WHERE deleted_at IS NOT NULL;",
        )?;
    }
    let cust_has_dnc: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('customers') WHERE name = 'deleted_at'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if cust_has_dnc {
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_customers_deleted_at
                ON customers(deleted_at) WHERE deleted_at IS NOT NULL;",
        )?;
    }
    Ok(())
}

fn seed_metric_definitions(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO metric_definitions (key, name, description, formula, source, limitations)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    const DEFS: &[(&str, &str, &str, &str, &str, &str)] = &[
        (
            "new_conversations",
            "New conversations",
            "Count of conversations whose remote_created_at falls in range",
            "COUNT(conversations) WHERE remote_created_at IN range",
            "local",
            "Depends on sync coverage; conversations created before first sync are counted at their remote date",
        ),
        (
            "active_conversations",
            "Active conversations",
            "Count of conversations with status=active at query time",
            "COUNT WHERE status=active",
            "local",
            "Point-in-time snapshot, not historical",
        ),
        (
            "pending_conversations",
            "Pending conversations",
            "Count of conversations with status=pending at query time",
            "COUNT WHERE status=pending",
            "local",
            "Point-in-time snapshot",
        ),
        (
            "closed_conversations",
            "Closed conversations",
            "Count of conversations closed (closed_at) in range",
            "COUNT WHERE closed_at IN range",
            "local",
            "Requires closed_at present in sync data",
        ),
        (
            "unassigned",
            "Unassigned conversations",
            "Count of active conversations with no assignee",
            "COUNT WHERE status IN (active,pending) AND assignee IS NULL",
            "local",
            "Point-in-time snapshot",
        ),
        (
            "backlog",
            "Backlog",
            "Active conversations with no activity for 7+ days",
            "COUNT WHERE days_since(last_activity_at) >= 7",
            "local",
            "Based on last_activity_at maintained locally",
        ),
        (
            "first_response_time_local",
            "First response time (local)",
            "Average minutes between conversation first activity and first agent reply thread",
            "AVG(first_reply.created_at - first_activity_at)",
            "local",
            "Calculated per local implementation; may differ from Help Scout reports",
        ),
        (
            "resolution_time_local",
            "Resolution time (local)",
            "Average minutes between first activity and closed_at",
            "AVG(closed_at - first_activity_at)",
            "local",
            "Calculated per local implementation",
        ),
        (
            "replies_sent",
            "Replies sent",
            "Count of published reply threads by users in range",
            "COUNT(threads) WHERE type=reply AND created_at IN range",
            "local",
            "Only includes threads present in local mirror",
        ),
        (
            "ratings",
            "Ratings",
            "Count of satisfaction ratings by value",
            "COUNT(ratings) GROUP BY rating",
            "local",
            "Only ratings synced locally",
        ),
        (
            "ai_draft_acceptance",
            "AI draft acceptance",
            "Accepted drafts / total drafts with feedback",
            "accepted / (accepted + rejected + edited)",
            "local",
            "AI-derived operational metric",
        ),
    ];
    for d in DEFS {
        stmt.execute(*d)?;
    }
    Ok(())
}

fn add(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    if !table_exists(conn, table) {
        tracing::debug!(table, column, "m040: table absent, skipping column add");
        return Ok(());
    }
    let present: bool = conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |r| r.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|c| c == column);
    if !present {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
        [table],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

fn try_create_unique_index(
    conn: &Connection,
    name: &str,
    unique_ddl: &str,
    plain_ddl: &str,
) -> Result<()> {
    let already: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name = ?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if already {
        return Ok(());
    }
    match conn.execute_batch(unique_ddl) {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::warn!(index = name, error = %e, "m040: unique index impossible on legacy data, falling back to a plain index");
            conn.execute_batch(plain_ddl)?;
            Ok(())
        }
    }
}

// `deny` rather than `forbid`: vectorstore_sqlitevec registers the
// sqlite-vec extension via rusqlite FFI (`sqlite3_auto_extension`) — the
// single place unsafe code is allowed, and only there.
#![deny(unsafe_code, rust_2018_idioms)]
#![warn(clippy::all, missing_docs)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    missing_docs
)]

pub use spp_catalog;

pub mod activity;
pub mod ai_analysis;
pub mod ai_attributes;
pub mod ai_center;
pub mod ai_eval;
pub mod ai_evidence;
pub mod ai_features;
pub mod ai_hub;
pub mod ai_lm_studio;
pub mod ai_local;
pub mod ai_memory_guard;
pub mod ai_pipeline;
pub mod ai_prompts;
pub mod ai_provider;
pub mod ai_tools;
pub mod analytics;
pub mod api_queue;
pub mod audit;
pub mod automation;
pub mod backup_service;
pub mod bootstrap;
pub mod business_hours;
pub mod chats;
pub mod coaching;
pub mod config;
pub mod connection_doctor;
pub mod connectors;
pub mod conversation_ops;
pub mod copilot;
pub mod crypto;
pub mod custom_objects;
pub mod customer_events;
pub mod customers;
pub mod data_reset;
pub mod data_tools;
pub mod db;
pub mod db_breadth;
pub mod digest;
pub mod embeddings;
pub mod encrypted_sync;
pub mod error;
pub mod eventing;
pub mod events;
pub mod helpscout;
pub mod helpscout_real;
pub mod hybrid_search;
pub mod inbox;
pub mod incident_workspace;
pub mod intelligence;
pub mod intelligence_features;
pub mod interaction_current;
pub mod interaction_engine;
pub mod issue_impact;
pub mod jobs;
pub mod knowledge_store;
pub mod logging;
pub mod loopback;
pub mod maintenance;
pub mod mentions;
pub mod migrations;
pub mod mirror_parity;
pub mod mirror_readouts;
pub mod mirror_tables;
pub mod notification_prefs;
pub mod notification_sweep;
pub mod notifications;
pub mod oauth_state;
pub mod operations;
pub mod outbound;
pub mod outreach;
pub mod people_store;
pub mod provider_handle;
pub mod quality;
pub mod reports;
pub mod reports_api;
pub mod response_state_sql;
pub mod richtext;
/// Landlock + seccomp sandbox for untrusted document parsing.
/// Linux-only and feature-flagged (`--features sandbox`).
#[cfg(all(target_os = "linux", feature = "sandbox"))]
pub mod sandbox;
pub mod saved_views;
pub mod search;
pub mod security;
pub mod segment;
pub mod settings;
pub mod side_threads;
pub mod sla;
pub mod support_graph;
pub mod sync;
pub mod sync_engine;
pub mod sync_schema;
pub mod textutil;
pub mod ticket_ops;
pub mod ticket_states;
pub mod translation;
pub mod vectorstore;
pub mod vectorstore_sqlitevec;
pub mod version_check;
pub mod webhook;
pub mod webhook_handler;
pub mod webhook_health;
pub mod workers;
pub mod workload;

// Test-only integration-verification modules (TODO 45 / 46). Declared
// `#[cfg(test)]` so none of this compiles into release builds; see each
// file's header for what it covers.
#[cfg(test)]
pub mod bench;
#[cfg(test)]
pub mod it_helpscout;
#[cfg(test)]
pub mod it_lm_studio;
#[cfg(test)]
pub mod it_loopback;
#[cfg(test)]
pub mod it_sync;
#[cfg(test)]
pub mod mockserver;

pub use error::{Error, Result};

pub mod catalog {
    pub use spp_catalog::*;
}

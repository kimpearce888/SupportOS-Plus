#![deny(unsafe_code, rust_2018_idioms)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod activity;
pub mod ai_attributes;
pub mod ai_center;
pub mod ai_eval;
pub mod ai_evidence;
pub mod ai_hub;
pub mod ai_lm_studio;
pub mod ai_local;
pub(crate) mod ai_memory_guard;
pub mod ai_pipeline;
pub mod ai_prompts;
pub(crate) mod ai_provider;
pub(crate) mod ai_tools;
pub mod analytics;
pub(crate) mod api_queue;
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
pub(crate) mod crypto;
pub mod custom_objects;
pub mod customer_events;
pub mod customers;
pub mod data_reset;
pub mod db;
pub mod digest;
pub mod embeddings;
pub mod error;
pub mod event_bus;
pub mod events;
pub mod helpscout;
pub mod helpscout_real;
pub mod inbox;
pub mod incident_workspace;
pub mod intelligence_features;
pub mod interaction_engine;
pub(crate) mod interaction_signals;
pub(crate) mod issue_impact;
pub mod jobs;
pub mod knowledge_store;
pub mod logging;
pub mod loopback;
pub mod maintenance;
pub mod mentions;
pub mod migrations;
pub mod mirror;
pub(crate) mod notification_sweep;
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
pub(crate) mod rrf;
#[cfg(all(target_os = "linux", feature = "sandbox"))]
pub mod sandbox;
pub(crate) mod saved_views;
pub mod search;
pub(crate) mod security;
pub mod segment;
pub mod settings;
pub mod sla;
pub mod support_graph;
pub mod sync_engine;
pub(crate) mod sync_schema;
pub(crate) mod textutil;
pub mod ticket_ops;
pub mod ticket_states;
pub mod translation;
pub(crate) mod vectorstore;
pub mod vectorstore_sqlitevec;
pub mod version_check;
pub mod webhook;
pub(crate) mod webhook_handler;
pub mod webhook_health;
pub mod workers;
pub mod workload;

pub(crate) use migrations::{
    ai_analysis, ai_features, data_tools, db_breadth, intelligence, mirror_parity, mirror_tables,
    side_threads,
};

#[cfg(test)]
mod bench;
#[cfg(test)]
mod it_helpscout;
#[cfg(test)]
mod it_lm_studio;
#[cfg(test)]
mod it_loopback;
#[cfg(test)]
mod it_sync;
#[cfg(test)]
mod mockserver;

pub use error::{Error, Result};

pub mod catalog {
    pub use spp_catalog::*;
}

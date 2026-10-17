#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(clippy::all, missing_docs)]
#![allow(
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    missing_docs
)]

pub mod activity;
pub mod attributes;
pub mod copilot;
pub mod graph;
pub mod notifications;
pub mod operations;
pub mod reporting;
pub mod workspace;

pub use activity::{ActivityField, ConditionKind, DateMode, ResponseState};
pub use attributes::{AI_ATTRIBUTE_SCHEMA_VERSION, AiAttributeKey, AttributeValueType};
pub use copilot::CopilotTool;
pub use graph::{GraphHumanRelation, GraphNodeKind};
pub use notifications::NotificationType;
pub use operations::OperationsTileKey;
pub use reporting::{ReportDimensionKey, ReportMetricKey};
pub use workspace::{
    ConnectorAuthMode, ConnectorKind, CustomFieldType, IncidentSeverity, IncidentSource,
    IncidentStatus,
};

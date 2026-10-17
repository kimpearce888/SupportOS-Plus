use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Investigating,
    Identified,
    FixInProgress,
    Monitoring,
    Resolved,
}

impl IncidentStatus {
    pub const ALL: [Self; 5] = [
        Self::Investigating,
        Self::Identified,
        Self::FixInProgress,
        Self::Monitoring,
        Self::Resolved,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Sev1,
    Sev2,
    Sev3,
    Sev4,
}

impl IncidentSeverity {
    pub const ALL: [Self; 4] = [Self::Sev1, Self::Sev2, Self::Sev3, Self::Sev4];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSource {
    Manual,
    Cluster,
    KnownIssue,
}

impl IncidentSource {
    pub const ALL: [Self; 3] = [Self::Manual, Self::Cluster, Self::KnownIssue];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomFieldType {
    Text,
    LongText,
    Number,
    Date,
    Boolean,
    Select,
}

impl CustomFieldType {
    pub const ALL: [Self; 6] = [
        Self::Text,
        Self::LongText,
        Self::Number,
        Self::Date,
        Self::Boolean,
        Self::Select,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    LocalJson,
    Csv,
    Sqlite,
    Http,
}

impl ConnectorKind {
    pub const ALL: [Self; 4] = [Self::LocalJson, Self::Csv, Self::Sqlite, Self::Http];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorAuthMode {
    None,
    Header,
    Bearer,
}

impl ConnectorAuthMode {
    pub const ALL: [Self; 3] = [Self::None, Self::Header, Self::Bearer];
}

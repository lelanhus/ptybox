use crate::model::{Action, ErrorInfo, Observation};
use serde::{Deserialize, Serialize};

/// Driver request envelope for protocol v2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriverRequestV2 {
    /// Protocol version for request/response compatibility.
    pub protocol_version: u32,
    /// Client-provided request identifier echoed in the response.
    pub request_id: String,
    /// Action to execute.
    pub action: Action,
    /// Optional per-action timeout in milliseconds.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Driver response status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriverResponseStatus {
    /// Action executed successfully.
    Ok,
    /// Action failed.
    Error,
}

/// Per-action deterministic metrics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriverActionMetrics {
    /// Monotonic action sequence number (1-based).
    pub sequence: u64,
    /// Action execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Driver response envelope for protocol v2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriverResponseV2 {
    /// Protocol version for request/response compatibility.
    pub protocol_version: u32,
    /// Request identifier echoed from request.
    pub request_id: String,
    /// Response status.
    pub status: DriverResponseStatus,
    /// Observation after action execution on success.
    #[serde(default)]
    pub observation: Option<Observation>,
    /// Structured error on failure.
    #[serde(default)]
    pub error: Option<ErrorInfo>,
    /// Per-action metrics.
    #[serde(default)]
    pub action_metrics: Option<DriverActionMetrics>,
}

/// Artifact record for driver actions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriverActionRecord {
    /// Monotonic action sequence number (1-based).
    pub sequence: u64,
    /// Request identifier supplied by client.
    pub request_id: String,
    /// Action payload as executed.
    pub action: Action,
    /// Effective timeout used for this action.
    pub timeout_ms: u64,
    /// Action start timestamp (ms since run start).
    pub started_at_ms: u64,
    /// Action end timestamp (ms since run start).
    pub ended_at_ms: u64,
}

use crate::model::policy::Policy;
use crate::model::scenario::Action;
use crate::model::{Observation, RunId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub run_result_version: u32,
    pub protocol_version: u32,
    pub run_id: RunId,
    pub status: RunStatus,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub policy: Policy,
    pub scenario: Option<crate::model::Scenario>,
    pub steps: Option<Vec<StepResult>>,
    pub final_observation: Option<Observation>,
    pub exit_status: Option<ExitStatus>,
    pub error: Option<ErrorInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Passed,
    Failed,
    Errored,
    Canceled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: crate::model::StepId,
    pub name: String,
    pub status: StepStatus,
    pub attempts: u32,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub action: Action,
    pub assertions: Vec<AssertionResult>,
    pub error: Option<ErrorInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Passed,
    Failed,
    Errored,
    Skipped,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssertionResult {
    #[serde(rename = "type")]
    pub assertion_type: String,
    pub passed: bool,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub terminated_by_harness: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub code: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

pub const PROTOCOL_VERSION: u32 = 1;
pub const RUN_RESULT_VERSION: u32 = 1;

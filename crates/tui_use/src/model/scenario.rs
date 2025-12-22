use crate::model::policy::Policy;
use crate::model::terminal::TerminalSize;
use crate::model::{RunId, StepId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub scenario_version: u32,
    pub metadata: ScenarioMetadata,
    pub run: RunConfig,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub initial_size: TerminalSize,
    pub policy: PolicyRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyRef {
    Inline(Policy),
    File { path: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub id: StepId,
    pub name: String,
    pub action: Action,
    #[serde(default)]
    pub assert: Vec<Assertion>,
    pub timeout_ms: u64,
    pub retries: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Key,
    Text,
    Resize,
    Wait,
    Terminate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assertion {
    #[serde(rename = "type")]
    pub assertion_type: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    pub protocol_version: u32,
    pub run_id: RunId,
    pub session_id: crate::model::SessionId,
    pub timestamp_ms: u64,
    pub screen: crate::model::ScreenSnapshot,
    pub transcript_delta: Option<String>,
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
}

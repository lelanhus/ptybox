use serde::{Deserialize, Serialize};

pub const NORMALIZATION_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationFilter {
    SnapshotId,
    RunId,
    RunTimestamps,
    StepTimestamps,
    ObservationTimestamp,
    SessionId,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationRuleTarget {
    Transcript,
    SnapshotLines,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizationRule {
    pub target: NormalizationRuleTarget,
    pub pattern: String,
    pub replace: String,
    #[serde(default)]
    pub terminated_by_harness: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationSource {
    Default,
    Policy,
    Cli,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizationRecord {
    pub normalization_version: u32,
    pub filters: Vec<NormalizationFilter>,
    pub strict: bool,
    pub source: NormalizationSource,
    #[serde(default)]
    pub rules: Vec<NormalizationRule>,
}

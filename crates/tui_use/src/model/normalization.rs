use serde::{Deserialize, Serialize};

/// Normalization format version.
pub const NORMALIZATION_VERSION: u32 = 1;

/// Filter for normalizing nondeterministic fields during replay.
///
/// These filters ignore specific fields when comparing artifacts,
/// reducing false failures from timestamps, IDs, etc.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationFilter {
    /// Ignore snapshot IDs.
    SnapshotId,
    /// Ignore run IDs.
    RunId,
    /// Ignore run `started_at_ms`/`ended_at_ms`.
    RunTimestamps,
    /// Ignore step `started_at_ms`/`ended_at_ms`.
    StepTimestamps,
    /// Ignore observation `timestamp_ms`.
    ObservationTimestamp,
    /// Ignore session IDs.
    SessionId,
}

/// Target for regex-based normalization rules.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationRuleTarget {
    /// Apply to transcript output.
    Transcript,
    /// Apply to screen snapshot lines.
    SnapshotLines,
}

/// Regex-based normalization rule for variable output.
///
/// Use for timestamps, PIDs, paths, or other nondeterministic content.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NormalizationRule {
    /// Where to apply the rule.
    pub target: NormalizationRuleTarget,
    /// Regex pattern to match.
    pub pattern: String,
    /// Replacement string (can use capture groups).
    pub replace: String,
    /// Whether this rule relates to harness termination.
    #[serde(default)]
    pub terminated_by_harness: bool,
}

/// Source of normalization configuration.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationSource {
    /// Built-in default filters.
    Default,
    /// From policy configuration.
    Policy,
    /// From CLI flags.
    Cli,
    /// No normalization (strict mode).
    None,
}

/// Record of normalization applied during a run.
///
/// Written to `normalization.json` in artifacts for reproducibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NormalizationRecord {
    /// Format version.
    pub normalization_version: u32,
    /// Filters applied.
    pub filters: Vec<NormalizationFilter>,
    /// Whether strict mode was enabled.
    pub strict: bool,
    /// Source of normalization configuration.
    pub source: NormalizationSource,
    /// Regex rules applied.
    #[serde(default)]
    pub rules: Vec<NormalizationRule>,
}

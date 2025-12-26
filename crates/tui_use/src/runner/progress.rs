//! Progress callback for reporting run progress.
//!
//! This module provides a trait for receiving progress events during scenario execution.

use crate::model::{AssertionResult, RunId, StepId, StepStatus};

/// Event emitted during scenario execution for progress tracking.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Run has started.
    RunStarted {
        /// Unique run identifier.
        run_id: RunId,
        /// Total number of steps.
        total_steps: usize,
    },
    /// A step has started.
    StepStarted {
        /// Step ID.
        step_id: StepId,
        /// Current step index (1-based).
        step_index: usize,
        /// Step name.
        name: String,
    },
    /// A step has completed.
    StepCompleted {
        /// Step ID.
        step_id: StepId,
        /// Step name.
        name: String,
        /// Final status.
        status: StepStatus,
        /// Duration in milliseconds.
        duration_ms: u64,
        /// Assertion results if any.
        assertions: Vec<AssertionResult>,
    },
    /// Run has completed.
    RunCompleted {
        /// Unique run identifier.
        run_id: RunId,
        /// Whether all steps passed.
        success: bool,
        /// Total duration in milliseconds.
        duration_ms: u64,
    },
}

/// Trait for receiving progress events during execution.
///
/// Implementors can use this to display progress, log events, or collect metrics.
pub trait ProgressCallback: Send {
    /// Called for each progress event.
    fn on_progress(&self, event: &ProgressEvent);
}

/// A no-op progress callback that discards all events.
pub struct NoopProgress;

impl ProgressCallback for NoopProgress {
    fn on_progress(&self, _event: &ProgressEvent) {}
}

/// A progress callback that collects events for testing.
#[cfg(test)]
pub struct CollectingProgress {
    events: std::sync::Mutex<Vec<ProgressEvent>>,
}

#[cfg(test)]
impl Default for CollectingProgress {
    fn default() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl CollectingProgress {
    /// Create a new collecting progress callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get collected events.
    ///
    /// # Panics
    /// Panics if the mutex is poisoned (indicates a prior panic during event collection).
    #[allow(clippy::expect_used)]
    pub fn events(&self) -> Vec<ProgressEvent> {
        self.events
            .lock()
            .expect("progress mutex poisoned - prior panic during event collection")
            .clone()
    }
}

#[cfg(test)]
impl ProgressCallback for CollectingProgress {
    #[allow(clippy::expect_used)]
    fn on_progress(&self, event: &ProgressEvent) {
        self.events
            .lock()
            .expect("progress mutex poisoned")
            .push(event.clone());
    }
}

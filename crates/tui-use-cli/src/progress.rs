//! Verbose progress output using indicatif.

use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::sync::Mutex;
use tui_use::model::StepStatus;
use tui_use::runner::{ProgressCallback, ProgressEvent};

/// Progress callback that outputs step-by-step progress to stderr.
pub struct VerboseProgress {
    spinner: Mutex<Option<ProgressBar>>,
    total_steps: Mutex<usize>,
}

impl VerboseProgress {
    /// Create a new verbose progress callback.
    pub fn new() -> Self {
        Self {
            spinner: Mutex::new(None),
            total_steps: Mutex::new(0),
        }
    }
}

impl ProgressCallback for VerboseProgress {
    fn on_progress(&self, event: &ProgressEvent) {
        match event {
            ProgressEvent::RunStarted {
                run_id,
                total_steps,
            } => {
                if let Ok(mut ts) = self.total_steps.lock() {
                    *ts = *total_steps;
                }
                // Print to stderr without affecting stdout JSON output
                let _ = writeln!(
                    std::io::stderr(),
                    "run started: {run_id} ({total_steps} steps)"
                );
            }
            ProgressEvent::StepStarted {
                step_id: _,
                step_index,
                name,
            } => {
                let total = self.total_steps.lock().map(|g| *g).unwrap_or(0);
                // Create spinner for step
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.cyan} [{elapsed_precise}] {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.set_message(format!("[{step_index}/{total}] {name}"));
                pb.enable_steady_tick(std::time::Duration::from_millis(100));

                if let Ok(mut spinner) = self.spinner.lock() {
                    *spinner = Some(pb);
                }
            }
            ProgressEvent::StepCompleted {
                step_id: _,
                name,
                status,
                duration_ms,
                assertions,
            } => {
                if let Ok(mut spinner) = self.spinner.lock() {
                    if let Some(pb) = spinner.take() {
                        pb.finish_and_clear();
                    }
                }

                let status_icon = match status {
                    StepStatus::Passed => "\x1b[32m✓\x1b[0m",
                    StepStatus::Failed => "\x1b[31m✗\x1b[0m",
                    StepStatus::Errored => "\x1b[31m!\x1b[0m",
                    StepStatus::Skipped => "\x1b[33m-\x1b[0m",
                };

                let _ = writeln!(
                    std::io::stderr(),
                    "  {status_icon} {name} ({duration_ms}ms)"
                );

                // Show assertion results for failed steps
                if *status != StepStatus::Passed {
                    for assertion in assertions {
                        let icon = if assertion.passed { "✓" } else { "✗" };
                        let atype = &assertion.assertion_type;
                        let _ = write!(std::io::stderr(), "      {icon} {atype}");
                        if let Some(msg) = &assertion.message {
                            let _ = write!(std::io::stderr(), ": {msg}");
                        }
                        let _ = writeln!(std::io::stderr());
                    }
                }
            }
            ProgressEvent::RunCompleted {
                run_id: _,
                success,
                duration_ms,
            } => {
                let status_msg = if *success {
                    "\x1b[32mpassed\x1b[0m"
                } else {
                    "\x1b[31mfailed\x1b[0m"
                };
                let _ = writeln!(
                    std::io::stderr(),
                    "run {status_msg}: {duration_ms}ms total"
                );
            }
        }
    }
}

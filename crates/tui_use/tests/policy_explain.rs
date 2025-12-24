// Test module - relaxed lint rules
#![allow(clippy::default_trait_access)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::inefficient_to_string)]
#![allow(clippy::panic)]
#![allow(clippy::manual_assert)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(missing_docs)]

use tui_use::model::policy::{FsPolicy, NetworkPolicy, Policy, SandboxMode};
use tui_use::model::{RunConfig, TerminalSize};
use tui_use::policy::explain_policy_for_run_config;

#[test]
fn explain_policy_reports_denials() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: false,
        network: NetworkPolicy::Enabled,
        network_unsafe_ack: false,
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec!["/tmp/write".to_string()],
            working_dir: None,
        },
        fs_write_unsafe_ack: false,
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
    };

    let explanation = explain_policy_for_run_config(&policy, &run);
    assert!(!explanation.allowed);
    assert!(!explanation.errors.is_empty());
}

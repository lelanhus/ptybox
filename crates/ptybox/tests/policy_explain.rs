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

use ptybox::model::policy::{FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy, SandboxMode};
use ptybox::model::{RunConfig, TerminalSize};
use ptybox::policy::explain_policy_for_run_config;

#[test]
fn explain_policy_reports_denials() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: false },
        network: NetworkPolicy::Enabled { ack: false },
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec!["/tmp/write".to_string()],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(policy.clone()),
    };

    let explanation = explain_policy_for_run_config(&policy, &run);
    assert!(!explanation.allowed);
    assert!(!explanation.errors.is_empty());
}

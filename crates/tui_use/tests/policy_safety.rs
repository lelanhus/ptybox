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
use tui_use::policy::EffectivePolicy;
use tui_use::policy::{
    validate_artifacts_dir, validate_env_policy, validate_fs_policy, validate_network_policy,
    validate_policy_version, validate_sandbox_mode, validate_write_access,
};

#[test]
fn sandbox_none_requires_acknowledgement() {
    let err = validate_sandbox_mode(&SandboxMode::None, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("explicit acknowledgement"));
}

#[test]
fn sandbox_none_with_ack_is_allowed() {
    validate_sandbox_mode(&SandboxMode::None, true).unwrap();
}

#[test]
fn sandbox_seatbelt_requires_availability() {
    match validate_sandbox_mode(&SandboxMode::Seatbelt, true) {
        Ok(()) => {}
        Err(err) => assert_eq!(err.code, "E_SANDBOX_UNAVAILABLE"),
    }
}

#[test]
fn fs_policy_rejects_root_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec!["/".to_string()],
        allowed_write: Vec::new(),
        working_dir: None,
    };
    let err = validate_fs_policy(&fs, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("disallowed"));
}

#[test]
fn fs_policy_rejects_relative_allowlist_paths() {
    let fs = FsPolicy {
        allowed_read: vec!["relative".to_string()],
        allowed_write: vec![],
        working_dir: None,
    };
    let err = validate_fs_policy(&fs, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_relative_write_allowlist_paths() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["relative".to_string()],
        working_dir: None,
    };
    let err = validate_fs_policy(&fs, true).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_relative_working_dir() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp".to_string()],
        allowed_write: vec![],
        working_dir: Some("relative".to_string()),
    };
    let err = validate_fs_policy(&fs, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("absolute"));
}

#[test]
fn run_config_rejects_relative_cwd() {
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
        },
        exec: tui_use::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec![],
        cwd: Some("relative".to_string()),
        initial_size: TerminalSize::default(),
        policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("absolute"));
}

#[test]
fn run_config_rejects_cwd_outside_allowlist() {
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec!["/tmp/allowed".to_string()],
            allowed_write: vec![],
            working_dir: None,
        },
        exec: tui_use::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec![],
        cwd: Some("/tmp/blocked".to_string()),
        initial_size: TerminalSize::default(),
        policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("working directory"));
}

#[test]
fn exec_policy_rejects_relative_allowed_executable() {
    let policy = Policy {
        exec: tui_use::model::policy::ExecPolicy {
            allowed_executables: vec!["relative".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_home_allowlist() {
    let home = std::env::var("HOME").expect("HOME must be set for test");
    let fs = FsPolicy {
        allowed_read: vec![home.clone()],
        allowed_write: Vec::new(),
        working_dir: None,
    };
    let err = validate_fs_policy(&fs, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("disallowed"));
}

#[test]
fn fs_policy_rejects_system_allowlists() {
    let denied = [
        "/System",
        "/Library",
        "/Users/Shared",
        "/private",
        "/Volumes",
    ];
    for path in denied {
        let fs = FsPolicy {
            allowed_read: vec![path.to_string()],
            allowed_write: Vec::new(),
            working_dir: None,
        };
        let err = validate_fs_policy(&fs, false).unwrap_err();
        assert_eq!(err.code, "E_POLICY_DENIED");
        assert!(err.message.contains("disallowed"));
    }
}

#[test]
fn fs_policy_rejects_working_dir_with_traversal() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp/allowed".to_string()],
        allowed_write: Vec::new(),
        working_dir: Some("/tmp/allowed/../blocked".to_string()),
    };
    let err = validate_fs_policy(&fs, false).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("working_dir"));
}

#[test]
fn artifacts_dir_requires_write_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp".to_string()],
        allowed_write: Vec::new(),
        working_dir: None,
    };
    let err = validate_artifacts_dir(std::path::Path::new("/tmp/output"), &fs).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn artifacts_dir_denies_traversal_outside_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["/tmp/allowed".to_string()],
        working_dir: None,
    };
    let err =
        validate_artifacts_dir(std::path::Path::new("/tmp/allowed/../blocked"), &fs).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn artifacts_dir_requires_absolute_path() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["relative".to_string()],
        working_dir: None,
    };
    let err = validate_artifacts_dir(std::path::Path::new("relative/output"), &fs).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn strict_write_mode_requires_ack_for_sandbox_profile() {
    let policy = Policy {
        sandbox: SandboxMode::Seatbelt,
        sandbox_unsafe_ack: true,
        fs_strict_write: true,
        fs_write_unsafe_ack: false,
        ..Policy::default()
    };
    let err = validate_write_access(&policy, None).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("write access"));
}

#[test]
fn strict_write_mode_requires_ack_for_artifacts() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        fs_strict_write: true,
        fs_write_unsafe_ack: false,
        artifacts: tui_use::model::policy::ArtifactsPolicy {
            enabled: true,
            dir: Some("/tmp/artifacts".to_string()),
            overwrite: false,
        },
        ..Policy::default()
    };
    let err = validate_write_access(&policy, None).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("write access"));
}

#[test]
fn strict_write_mode_allows_ack() {
    let policy = Policy {
        sandbox: SandboxMode::Seatbelt,
        sandbox_unsafe_ack: true,
        fs_strict_write: true,
        fs_write_unsafe_ack: true,
        ..Policy::default()
    };
    validate_write_access(&policy, None).unwrap();
}

#[test]
fn env_policy_requires_allowlist_for_set() {
    let env = tui_use::model::policy::EnvPolicy {
        allowlist: vec![],
        set: [("SECRET".to_string(), "value".to_string())]
            .into_iter()
            .collect(),
        inherit: false,
    };
    let err = validate_env_policy(&env).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("allowlist"));
}

#[test]
fn network_enabled_requires_ack_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Enabled,
        network_unsafe_ack: false,
        ..Policy::default()
    };
    let err = validate_network_policy(&policy).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("network"));
}

#[test]
fn network_enabled_with_ack_is_allowed() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Enabled,
        network_unsafe_ack: true,
        ..Policy::default()
    };
    validate_network_policy(&policy).unwrap();
}

#[test]
fn network_disabled_requires_ack_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Disabled,
        network_unsafe_ack: false,
        ..Policy::default()
    };
    let err = validate_network_policy(&policy).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("network"));
}

#[test]
fn network_disabled_with_ack_is_allowed_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Disabled,
        network_unsafe_ack: true,
        ..Policy::default()
    };
    validate_network_policy(&policy).unwrap();
}

#[test]
fn write_allowlist_requires_explicit_ack() {
    let fs = FsPolicy {
        allowed_read: Vec::new(),
        allowed_write: vec!["/tmp/allowed".to_string()],
        working_dir: None,
    };
    let policy = Policy {
        fs,
        fs_write_unsafe_ack: false,
        ..Policy::default()
    };
    let err = validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack).unwrap_err();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("write"));
}

#[test]
fn write_allowlist_with_ack_is_allowed() {
    let fs = FsPolicy {
        allowed_read: Vec::new(),
        allowed_write: vec!["/tmp/allowed".to_string()],
        working_dir: None,
    };
    let policy = Policy {
        fs,
        fs_write_unsafe_ack: true,
        ..Policy::default()
    };
    validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack).unwrap();
}

#[test]
fn policy_version_mismatch_is_rejected() {
    let policy = Policy {
        policy_version: 999,
        ..Policy::default()
    };
    let err = validate_policy_version(&policy).unwrap_err();
    assert_eq!(err.code, "E_PROTOCOL");
    assert!(err.message.contains("policy_version"));
}

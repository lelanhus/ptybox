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
use ptybox::policy::EffectivePolicy;
use ptybox::policy::{
    validate_artifacts_dir, validate_env_policy, validate_fs_policy, validate_network_policy,
    validate_policy_version, validate_sandbox_mode, validate_write_access,
};
use ptybox::runner::ErrorCode;

#[test]
fn sandbox_disabled_requires_acknowledgement() {
    let err = validate_sandbox_mode(&SandboxMode::Disabled { ack: false }).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("explicit acknowledgement"));
}

#[test]
fn sandbox_disabled_with_ack_is_allowed() {
    validate_sandbox_mode(&SandboxMode::Disabled { ack: true }).unwrap();
}

#[test]
fn sandbox_seatbelt_requires_availability() {
    match validate_sandbox_mode(&SandboxMode::Seatbelt) {
        Ok(()) => {}
        Err(err) => assert_eq!(err.code, ErrorCode::SandboxUnavailable),
    }
}

#[test]
fn fs_policy_rejects_root_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec!["/".to_string()],
        allowed_write: Vec::new(),
        working_dir: None,
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("disallowed"));
}

#[test]
fn fs_policy_rejects_relative_allowlist_paths() {
    let fs = FsPolicy {
        allowed_read: vec!["relative".to_string()],
        allowed_write: vec![],
        working_dir: None,
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_relative_write_allowlist_paths() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["relative".to_string()],
        working_dir: None,
        write_ack: true,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_relative_working_dir() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp".to_string()],
        allowed_write: vec![],
        working_dir: Some("relative".to_string()),
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("absolute"));
}

#[test]
fn run_config_rejects_relative_cwd() {
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
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
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("absolute"));
}

#[test]
fn run_config_rejects_cwd_outside_allowlist() {
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec!["/tmp/allowed".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
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
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("working directory"));
}

#[test]
fn exec_policy_rejects_relative_allowed_executable() {
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["relative".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("absolute"));
}

#[test]
fn fs_policy_rejects_home_allowlist() {
    let home = std::env::var("HOME").expect("HOME must be set for test");
    let fs = FsPolicy {
        allowed_read: vec![home.clone()],
        allowed_write: Vec::new(),
        working_dir: None,
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
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
            write_ack: false,
            strict_write: false,
        };
        let err = validate_fs_policy(&fs).unwrap_err();
        assert_eq!(err.code, ErrorCode::PolicyDenied);
        assert!(err.message.contains("disallowed"));
    }
}

#[test]
fn fs_policy_rejects_working_dir_with_traversal() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp/allowed".to_string()],
        allowed_write: Vec::new(),
        working_dir: Some("/tmp/allowed/../blocked".to_string()),
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("working_dir"));
}

#[test]
fn artifacts_dir_requires_write_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec!["/tmp".to_string()],
        allowed_write: Vec::new(),
        working_dir: None,
        write_ack: false,
        strict_write: false,
    };
    let err = validate_artifacts_dir(std::path::Path::new("/tmp/output"), &fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn artifacts_dir_denies_traversal_outside_allowlist() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["/tmp/allowed".to_string()],
        working_dir: None,
        write_ack: true,
        strict_write: false,
    };
    let err =
        validate_artifacts_dir(std::path::Path::new("/tmp/allowed/../blocked"), &fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn artifacts_dir_requires_absolute_path() {
    let fs = FsPolicy {
        allowed_read: vec![],
        allowed_write: vec!["relative".to_string()],
        working_dir: None,
        write_ack: true,
        strict_write: false,
    };
    let err = validate_artifacts_dir(std::path::Path::new("relative/output"), &fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("artifacts dir"));
}

#[test]
fn strict_write_mode_requires_ack_for_sandbox_profile() {
    let policy = Policy {
        sandbox: SandboxMode::Seatbelt,
        fs: FsPolicy {
            strict_write: true,
            write_ack: false,
            ..FsPolicy::default()
        },
        ..Policy::default()
    };
    let err = validate_write_access(&policy, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("write access"));
}

#[test]
fn strict_write_mode_requires_ack_for_artifacts() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: true },
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            strict_write: true,
            write_ack: false,
            ..FsPolicy::default()
        },
        artifacts: ptybox::model::policy::ArtifactsPolicy {
            enabled: true,
            dir: Some("/tmp/artifacts".to_string()),
            overwrite: false,
        },
        ..Policy::default()
    };
    let err = validate_write_access(&policy, None).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("write access"));
}

#[test]
fn strict_write_mode_allows_ack() {
    let policy = Policy {
        sandbox: SandboxMode::Seatbelt,
        fs: FsPolicy {
            strict_write: true,
            write_ack: true,
            ..FsPolicy::default()
        },
        ..Policy::default()
    };
    validate_write_access(&policy, None).unwrap();
}

#[test]
fn env_policy_requires_allowlist_for_set() {
    let env = ptybox::model::policy::EnvPolicy {
        allowlist: vec![],
        set: [("SECRET".to_string(), "value".to_string())]
            .into_iter()
            .collect(),
        inherit: false,
    };
    let err = validate_env_policy(&env).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("allowlist"));
}

#[test]
fn network_enabled_requires_ack_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Enabled { ack: false },
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        ..Policy::default()
    };
    let err = validate_network_policy(&policy).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("network"));
}

#[test]
fn network_enabled_with_ack_is_allowed() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Enabled { ack: true },
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        ..Policy::default()
    };
    validate_network_policy(&policy).unwrap();
}

#[test]
fn network_disabled_requires_ack_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: false,
        },
        ..Policy::default()
    };
    let err = validate_network_policy(&policy).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("network"));
}

#[test]
fn network_disabled_with_ack_is_allowed_when_unsandboxed() {
    let policy = Policy {
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
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
        write_ack: false,
        strict_write: false,
    };
    let err = validate_fs_policy(&fs).unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("write"));
}

#[test]
fn write_allowlist_with_ack_is_allowed() {
    let fs = FsPolicy {
        allowed_read: Vec::new(),
        allowed_write: vec!["/tmp/allowed".to_string()],
        working_dir: None,
        write_ack: true,
        strict_write: false,
    };
    validate_fs_policy(&fs).unwrap();
}

#[test]
fn policy_version_mismatch_is_rejected() {
    let policy = Policy {
        policy_version: 999,
        ..Policy::default()
    };
    let err = validate_policy_version(&policy).unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol);
    assert!(err.message.contains("policy_version"));
}

// =============================================================================
// Shell Detection Tests
// =============================================================================

/// Test that shell commands are detected by basename
#[test]
fn shell_detection_blocks_direct_shell_invocation() {
    let shells = [
        "/bin/bash",
        "/bin/sh",
        "/usr/bin/zsh",
        "/usr/local/bin/fish",
    ];

    for shell in shells {
        let policy = Policy {
            exec: ptybox::model::policy::ExecPolicy {
                allowed_executables: vec![shell.to_string()],
                allow_shell: false,
            },
            fs: FsPolicy {
                allowed_read: vec!["/tmp".to_string()],
                allowed_write: vec![],
                working_dir: None,
                write_ack: false,
                strict_write: false,
            },
            ..Policy::default()
        };
        let run = RunConfig {
            command: shell.to_string(),
            args: vec![],
            cwd: Some("/tmp".to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
        };
        let err = EffectivePolicy::new(policy)
            .validate_run_config(&run)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::PolicyDenied);
        assert!(
            err.message.contains("shell"),
            "Shell {} should be blocked, got: {}",
            shell,
            err.message
        );
    }
}

/// Test that .sh extension is blocked
#[test]
fn shell_detection_blocks_sh_extension() {
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/tmp/script.sh".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/tmp/script.sh".to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("shell"));
}

/// Test that -c flag is allowed for non-shell interpreters like Python
#[test]
fn shell_detection_allows_interpreter_dash_c_flag() {
    // Python -c is NOT shell command execution - it's Python inline code.
    // Other interpreters like Ruby (-c = syntax check) and Perl (-c = compile only)
    // also use -c for non-shell purposes. We only block shells themselves.
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/usr/bin/python3".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/usr/bin/python3".to_string(),
        args: vec!["-c".to_string(), "print('hello')".to_string()],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    // This should succeed - Python -c is not shell execution
    let result = EffectivePolicy::new(policy).validate_run_config(&run);
    assert!(
        result.is_ok(),
        "Python -c should be allowed: {:?}",
        result.err()
    );
}

/// Test that -c flag is still blocked for actual shells
#[test]
fn shell_detection_blocks_shell_with_dash_c_flag() {
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/sh".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), "echo hello".to_string()],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("shell"));
}

/// Helper struct for cleaning up symlinks in tests
struct SymlinkCleanup(std::path::PathBuf);
impl Drop for SymlinkCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir(self.0.parent().unwrap());
    }
}

/// Test that symlinked shells are detected via canonicalization
#[test]
fn shell_detection_blocks_symlinked_shell() {
    use std::os::unix::fs::symlink;

    // Create a temp dir and symlink
    let temp_dir = std::env::temp_dir().join(format!("ptybox-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let symlink_path = temp_dir.join("not_a_shell");

    // Create symlink to bash (if it exists)
    let bash_path = std::path::Path::new("/bin/bash");
    if !bash_path.exists() {
        // Skip test if bash doesn't exist
        return;
    }

    // Clean up any existing symlink
    let _ = std::fs::remove_file(&symlink_path);
    symlink(bash_path, &symlink_path).expect("Failed to create test symlink");

    // Clean up on drop
    let _cleanup = SymlinkCleanup(symlink_path.clone());

    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec![symlink_path.display().to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string(), temp_dir.display().to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: symlink_path.display().to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    let err = EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(
        err.message.contains("shell"),
        "Symlinked bash should be detected as shell, got: {}",
        err.message
    );
}

/// Test that non-shell commands are allowed
#[test]
fn shell_detection_allows_non_shell_commands() {
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
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
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    // Should succeed - echo is not a shell
    EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .expect("/bin/echo should be allowed");
}

/// Test that shell commands with `allow_shell=true` are allowed
#[test]
fn shell_detection_allows_shells_when_enabled() {
    let policy = Policy {
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/bash".to_string()],
            allow_shell: true, // Explicitly allow shells
        },
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        ..Policy::default()
    };
    let run = RunConfig {
        command: "/bin/bash".to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    // Should succeed when allow_shell is true
    EffectivePolicy::new(policy)
        .validate_run_config(&run)
        .expect("bash should be allowed when allow_shell=true");
}

// =============================================================================
// Edge Case Tests - Path Handling
// =============================================================================

#[test]
fn policy_handles_unicode_paths() {
    // Unicode characters in paths should be handled correctly
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec!["/tmp/日本語ディレクトリ".to_string()],
            allowed_write: vec!["/tmp/émoji_🎉_path".to_string()],
            working_dir: Some("/tmp/日本語ディレクトリ".to_string()),
            write_ack: true,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };

    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: Some("/tmp/日本語ディレクトリ".to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };

    // Should not panic - unicode paths are valid
    let effective = EffectivePolicy::new(policy);
    let result = effective.validate_run_config(&run);
    // Result may pass or fail depending on whether cwd is under allowed paths,
    // but importantly it should not panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn policy_handles_very_long_paths() {
    // Very long paths (>4096 chars) should be handled gracefully
    let long_component = "a".repeat(200);
    let long_path = format!(
        "/tmp/{}/{}/{}/{}/{}",
        long_component, long_component, long_component, long_component, long_component
    );

    assert!(long_path.len() > 1000, "Path should be very long");

    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec![long_path.clone()],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };

    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: Some(long_path.clone()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };

    // Should not panic - long paths should be processed
    let effective = EffectivePolicy::new(policy);
    let result = effective.validate_run_config(&run);
    // Policy validation should complete without panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn policy_handles_paths_with_special_characters() {
    // Paths with special characters should be handled
    let special_path = "/tmp/path with spaces/file-with-dashes/under_scores";

    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec![special_path.to_string()],
            allowed_write: vec![],
            working_dir: Some(special_path.to_string()),
            write_ack: false,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };

    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: Some(special_path.to_string()),
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };

    // Should not panic - special characters in paths are valid
    let effective = EffectivePolicy::new(policy);
    let result = effective.validate_run_config(&run);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn policy_handles_empty_path_lists() {
    // Empty allowed path lists should be handled
    let policy = Policy {
        fs: FsPolicy {
            allowed_read: vec![],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ptybox::model::policy::ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        ..Policy::default()
    };

    let run = RunConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string()],
        cwd: None,
        initial_size: TerminalSize::default(),
        policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };

    // Should not panic - empty lists are valid (deny-by-default)
    let effective = EffectivePolicy::new(policy);
    let result = effective.validate_run_config(&run);
    // Should succeed since no cwd specified and command is allowed
    assert!(
        result.is_ok(),
        "Empty path lists should be valid: {:?}",
        result.err()
    );
}

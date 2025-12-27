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

use std::fs;

use tui_use::model::policy::{FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy, SandboxMode};
use tui_use::policy::sandbox::write_profile;
use tui_use::runner::ErrorCode;

fn temp_profile(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("tui-use-sandbox-{name}.sb"));
    path
}

fn base_policy() -> Policy {
    Policy {
        policy_version: tui_use::model::policy::POLICY_VERSION,
        sandbox: SandboxMode::Seatbelt,
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck::default(),
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec!["/tmp".to_string()],
            working_dir: None,
            write_ack: true,
            strict_write: false,
        },
        exec: Default::default(),
        env: Default::default(),
        budgets: Default::default(),
        artifacts: Default::default(),
        replay: Default::default(),
    }
}

#[test]
fn sandbox_profile_disables_network_by_default() {
    let policy = base_policy();
    let path = temp_profile("no-net");
    write_profile(&path, &policy).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert!(!contents.contains("network-outbound"));
}

#[test]
fn sandbox_profile_allows_network_when_enabled() {
    let mut policy = base_policy();
    policy.network = NetworkPolicy::Enabled { ack: true };
    let path = temp_profile("net");
    write_profile(&path, &policy).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert!(contents.contains("network-outbound"));
}

#[test]
fn sandbox_profile_includes_allowed_read_write_paths() {
    let policy = base_policy();
    let path = temp_profile("fs-allow");
    write_profile(&path, &policy).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert!(contents.contains("(allow file-read* (subpath \"/tmp\"))"));
    assert!(contents.contains("(allow file-write* (subpath \"/tmp\"))"));
}

#[test]
fn sandbox_profile_omits_read_rules_when_empty() {
    let mut policy = base_policy();
    policy.fs.allowed_read.clear();
    let path = temp_profile("fs-empty-read");
    write_profile(&path, &policy).unwrap();
    let contents = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_file(&path);
    assert!(!contents.contains("file-read* (subpath"));
}

// =============================================================================
// Sandbox Injection Tests
// =============================================================================

#[test]
fn sandbox_rejects_newline_injection() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp\n(allow default)".to_string()];
    let path = temp_profile("newline-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_rejects_carriage_return_injection() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp\r(allow default)".to_string()];
    let path = temp_profile("cr-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_rejects_quote_injection() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp\")(allow default)(file-read*\"".to_string()];
    let path = temp_profile("quote-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_rejects_open_paren_injection() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp(allow default)".to_string()];
    let path = temp_profile("open-paren-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_rejects_close_paren_injection() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp)".to_string()];
    let path = temp_profile("close-paren-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_rejects_null_byte_in_path() {
    let mut policy = base_policy();
    policy.fs.allowed_read = vec!["/tmp\0/evil".to_string()];
    let path = temp_profile("null-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_injection_in_allowed_write_paths() {
    let mut policy = base_policy();
    policy.fs.allowed_write = vec!["/tmp\")(allow network-outbound)\"".to_string()];
    let path = temp_profile("write-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_injection_in_allowed_executables() {
    let mut policy = base_policy();
    policy.exec.allowed_executables = vec!["/bin/sh\")(allow default)\"".to_string()];
    let path = temp_profile("exec-inject");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::PolicyDenied);
}

#[test]
fn sandbox_allows_safe_paths_with_special_chars() {
    let mut policy = base_policy();
    // These should all be allowed by the whitelist
    policy.fs.allowed_read = vec![
        "/tmp".to_string(),
        "/usr/local/bin".to_string(),
        "/var/folders/test-dir_123".to_string(),
        "/Users/name@domain.com/Documents".to_string(),
        "/Applications/My App.app".to_string(),
    ];
    let path = temp_profile("safe-paths");
    let result = write_profile(&path, &policy);
    let _ = fs::remove_file(&path);
    assert!(
        result.is_ok(),
        "Safe paths should be allowed: {:?}",
        result.err()
    );
}

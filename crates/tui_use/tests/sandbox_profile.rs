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

use tui_use::model::policy::{FsPolicy, NetworkPolicy, Policy, SandboxMode};
use tui_use::policy::sandbox::write_profile;

fn temp_profile(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("tui-use-sandbox-{name}.sb"));
    path
}

fn base_policy() -> Policy {
    Policy {
        sandbox: SandboxMode::Seatbelt,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Disabled,
        network_unsafe_ack: false,
        fs: FsPolicy {
            allowed_read: vec!["/tmp".to_string()],
            allowed_write: vec!["/tmp".to_string()],
            working_dir: None,
        },
        fs_write_unsafe_ack: true,
        fs_strict_write: false,
        exec: Default::default(),
        env: Default::default(),
        budgets: Default::default(),
        artifacts: Default::default(),
        replay: Default::default(),
        policy_version: tui_use::model::policy::POLICY_VERSION,
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
    policy.network = NetworkPolicy::Enabled;
    policy.network_unsafe_ack = true;
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

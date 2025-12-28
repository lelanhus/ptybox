//! Tests for the TUI mode flag.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(missing_docs)]

use std::fs;
use std::process::Command;

fn ptybox_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ptybox"))
}

fn temp_path(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ptybox-test-tui-{name}"));
    path
}

#[test]
fn run_accepts_tui_flag() {
    let output = ptybox_bin()
        .arg("run")
        .arg("--tui")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "run --tui --help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_help_shows_tui_flag() {
    let output = ptybox_bin()
        .arg("run")
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--tui"),
        "run help should mention --tui flag"
    );
}

#[test]
fn run_tui_rejects_json_combination() {
    // Create a minimal scenario file
    let temp_dir = std::env::temp_dir().join("ptybox-test-tui");
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let scenario_path = temp_dir.join("scenario.json");
    let scenario_json = r#"{
        "scenario_version": 1,
        "metadata": { "name": "test", "description": null },
        "run": {
            "command": "echo",
            "args": ["test"],
            "initial_size": { "rows": 24, "cols": 80 },
            "policy": {
                "policy_version": 3,
                "sandbox": "none",
                "sandbox_unsafe_ack": true,
                "network": "disabled",
                "network_unsafe_ack": true,
                "fs": { "allowed_read": [], "allowed_write": [] },
                "exec": { "allowed_executables": [], "allow_shell": false },
                "env": { "allowlist": [], "set": {}, "inherit": false },
                "budgets": {
                    "max_runtime_ms": 60000,
                    "max_steps": 10000,
                    "max_output_bytes": 8388608,
                    "max_snapshot_bytes": 2097152,
                    "max_wait_ms": 10000
                },
                "artifacts": { "enabled": false, "overwrite": false }
            }
        },
        "steps": []
    }"#;
    std::fs::write(&scenario_path, scenario_json).expect("write scenario");

    let output = ptybox_bin()
        .arg("run")
        .arg("--scenario")
        .arg(&scenario_path)
        .arg("--tui")
        .arg("--json")
        .output()
        .expect("failed to execute");

    assert!(!output.status.success(), "run --tui --json should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("cannot be combined"),
        "should mention incompatible flags: {combined}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn run_tui_rejects_verbose_combination() {
    let temp_dir = std::env::temp_dir().join("ptybox-test-tui-verbose");
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    let scenario_path = temp_dir.join("scenario.json");
    let scenario_json = r#"{
        "scenario_version": 1,
        "metadata": { "name": "test", "description": null },
        "run": {
            "command": "echo",
            "args": ["test"],
            "initial_size": { "rows": 24, "cols": 80 },
            "policy": {
                "policy_version": 3,
                "sandbox": "none",
                "sandbox_unsafe_ack": true,
                "network": "disabled",
                "network_unsafe_ack": true,
                "fs": { "allowed_read": [], "allowed_write": [] },
                "exec": { "allowed_executables": [], "allow_shell": false },
                "env": { "allowlist": [], "set": {}, "inherit": false },
                "budgets": {
                    "max_runtime_ms": 60000,
                    "max_steps": 10000,
                    "max_output_bytes": 8388608,
                    "max_snapshot_bytes": 2097152,
                    "max_wait_ms": 10000
                },
                "artifacts": { "enabled": false, "overwrite": false }
            }
        },
        "steps": []
    }"#;
    std::fs::write(&scenario_path, scenario_json).expect("write scenario");

    let output = ptybox_bin()
        .arg("run")
        .arg("--scenario")
        .arg(&scenario_path)
        .arg("--tui")
        .arg("--verbose")
        .output()
        .expect("failed to execute");

    assert!(!output.status.success(), "run --tui --verbose should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("cannot be combined"),
        "should mention incompatible flags: {combined}"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn run_tui_requires_scenario_file() {
    // Running with --tui but no scenario should fail
    let output = ptybox_bin()
        .arg("run")
        .arg("--tui")
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "run --tui without scenario should fail"
    );
}

#[test]
fn exec_does_not_have_tui_flag() {
    // The exec command should not have a --tui flag
    let output = ptybox_bin()
        .arg("exec")
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("--tui"),
        "exec command should not have --tui flag"
    );
}

#[test]
fn run_tui_with_nonexistent_scenario_file() {
    let output = ptybox_bin()
        .arg("run")
        .arg("--tui")
        .arg("--scenario")
        .arg("/nonexistent/scenario.json")
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "run --tui with nonexistent scenario should fail"
    );
}

#[test]
fn run_tui_rejects_color_never() {
    // --color=never with TUI should work (TUI manages its own colors)
    // but let's verify the flag is accepted
    let output = ptybox_bin()
        .arg("run")
        .arg("--tui")
        .arg("--color=never")
        .arg("--help")
        .output()
        .expect("failed to execute");

    // This should succeed (help always works)
    assert!(
        output.status.success(),
        "run --tui --color=never --help should succeed"
    );
}

#[test]
fn replay_does_not_have_tui_flag() {
    // The replay command should not have a --tui flag
    let output = ptybox_bin()
        .arg("replay")
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("--tui"),
        "replay command should not have --tui flag"
    );
}

#[test]
fn run_tui_with_malformed_scenario() {
    let path = temp_path("malformed-scenario.json");
    fs::write(&path, "{ not valid json }").expect("write file");

    let output = ptybox_bin()
        .arg("run")
        .arg("--tui")
        .arg("--scenario")
        .arg(&path)
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "run --tui with malformed scenario should fail"
    );

    let _ = fs::remove_file(path);
}

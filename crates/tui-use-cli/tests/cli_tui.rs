//! Tests for the TUI mode flag.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::process::Command;

fn tui_use_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tui-use"))
}

#[test]
fn run_accepts_tui_flag() {
    let output = tui_use_bin()
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
    let output = tui_use_bin()
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
    let temp_dir = std::env::temp_dir().join("tui-use-test-tui");
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

    let output = tui_use_bin()
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
    let temp_dir = std::env::temp_dir().join("tui-use-test-tui-verbose");
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

    let output = tui_use_bin()
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

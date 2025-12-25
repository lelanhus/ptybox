//! Tests for the trace viewer command.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn tui_use_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tui-use"))
}

fn create_mock_artifacts(dir: &std::path::Path) {
    // Create run.json with complete Policy structure
    let run_json = r#"{
        "run_result_version": 1,
        "protocol_version": 1,
        "run_id": "00000000-0000-0000-0000-000000000001",
        "status": "passed",
        "started_at_ms": 1000,
        "ended_at_ms": 2000,
        "command": "echo",
        "args": ["hello"],
        "cwd": "/tmp",
        "policy": {
            "policy_version": 3,
            "sandbox": "seatbelt",
            "network": "disabled",
            "fs": {
                "allowed_read": [],
                "allowed_write": []
            },
            "exec": {
                "allowed_executables": [],
                "allow_shell": false
            },
            "env": {
                "allowlist": [],
                "set": {},
                "inherit": false
            },
            "budgets": {
                "max_runtime_ms": 60000,
                "max_steps": 10000,
                "max_output_bytes": 8388608,
                "max_snapshot_bytes": 2097152,
                "max_wait_ms": 10000
            },
            "artifacts": {
                "enabled": false,
                "overwrite": false
            }
        },
        "steps": [
            {
                "step_id": "00000000-0000-0000-0000-000000000002",
                "name": "Send text",
                "status": "passed",
                "attempts": 1,
                "started_at_ms": 1000,
                "ended_at_ms": 1500,
                "action": { "type": "text", "payload": {"text": "hello"} },
                "assertions": [
                    {"type": "screen_contains", "passed": true}
                ]
            }
        ]
    }"#;
    fs::write(dir.join("run.json"), run_json).expect("write run.json");

    // Create snapshots directory and a snapshot
    let snapshots_dir = dir.join("snapshots");
    fs::create_dir_all(&snapshots_dir).expect("create snapshots dir");

    let snapshot_json = r#"{
        "snapshot_version": 1,
        "snapshot_id": "00000000-0000-0000-0000-000000000003",
        "rows": 24,
        "cols": 80,
        "cursor": {"row": 0, "col": 5, "visible": true},
        "alternate_screen": false,
        "lines": ["hello", "$"],
        "cells": null
    }"#;
    fs::write(snapshots_dir.join("0001.json"), snapshot_json).expect("write snapshot");

    // Create transcript
    fs::write(dir.join("transcript.log"), "hello\n").expect("write transcript");
}

#[test]
fn trace_generates_valid_html() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    let output_file = artifacts_dir.path().join("trace.html");

    let output = tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(artifacts_dir.path())
        .arg("--output")
        .arg(&output_file)
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "trace should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(output_file.exists(), "output file should be created");

    let html = fs::read_to_string(&output_file).expect("read html");
    assert!(html.contains("<!DOCTYPE html>"), "should be valid HTML");
    assert!(html.contains("tui-use Trace Viewer"), "should have title");
}

#[test]
fn trace_embeds_run_metadata() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    let output_file = artifacts_dir.path().join("trace.html");

    tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(artifacts_dir.path())
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("failed to execute");

    let html = fs::read_to_string(&output_file).expect("read html");

    // Check run ID is embedded
    assert!(
        html.contains("00000000-0000-0000-0000-000000000001"),
        "should contain run ID"
    );
    // Check status
    assert!(html.contains("Passed") || html.contains("passed"), "should contain status");
}

#[test]
fn trace_embeds_all_snapshots() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    // Add a second snapshot
    let snapshots_dir = artifacts_dir.path().join("snapshots");
    let snapshot2_json = r#"{
        "snapshot_version": 1,
        "snapshot_id": "00000000-0000-0000-0000-000000000004",
        "rows": 24,
        "cols": 80,
        "cursor": {"row": 1, "col": 0, "visible": true},
        "alternate_screen": false,
        "lines": ["hello", "world", "$"],
        "cells": null
    }"#;
    fs::write(snapshots_dir.join("0002.json"), snapshot2_json).expect("write snapshot 2");

    let output_file = artifacts_dir.path().join("trace.html");

    tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(artifacts_dir.path())
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("failed to execute");

    let html = fs::read_to_string(&output_file).expect("read html");

    // Check that both snapshots are embedded (in the SNAPSHOTS JSON array)
    assert!(
        html.contains("00000000-0000-0000-0000-000000000003"),
        "should contain first snapshot ID"
    );
    assert!(
        html.contains("00000000-0000-0000-0000-000000000004"),
        "should contain second snapshot ID"
    );
}

#[test]
fn trace_embeds_step_results() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    let output_file = artifacts_dir.path().join("trace.html");

    tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(artifacts_dir.path())
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("failed to execute");

    let html = fs::read_to_string(&output_file).expect("read html");

    // Check step data is embedded
    assert!(html.contains("Send text"), "should contain step name");
    assert!(
        html.contains("screen_contains"),
        "should contain assertion type"
    );
}

#[test]
fn trace_renders_ansi_colors_as_css() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    // Create snapshot with cells including color styling
    let snapshots_dir = artifacts_dir.path().join("snapshots");
    let snapshot_with_cells = r#"{
        "snapshot_version": 1,
        "snapshot_id": "00000000-0000-0000-0000-000000000005",
        "rows": 24,
        "cols": 80,
        "cursor": {"row": 0, "col": 1, "visible": true},
        "alternate_screen": false,
        "lines": ["R"],
        "cells": [[
            {
                "ch": "R",
                "width": 1,
                "style": {
                    "fg": {"ansi16": 1},
                    "bg": "default",
                    "bold": true,
                    "italic": false,
                    "underline": false,
                    "inverse": false
                }
            }
        ]]
    }"#;
    fs::write(snapshots_dir.join("0001.json"), snapshot_with_cells).expect("write snapshot");

    let output_file = artifacts_dir.path().join("trace.html");

    tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(artifacts_dir.path())
        .arg("-o")
        .arg(&output_file)
        .output()
        .expect("failed to execute");

    let html = fs::read_to_string(&output_file).expect("read html");

    // Check that color conversion code is present
    assert!(
        html.contains("colorToCss"),
        "should contain color conversion function"
    );
    // Check that ANSI colors are defined
    assert!(
        html.contains("#cd0000") || html.contains("cd0000"),
        "should contain ANSI red color"
    );
}

#[test]
fn trace_fails_gracefully_on_missing_artifacts() {
    let non_existent = PathBuf::from("/tmp/non-existent-artifacts-12345");

    let output = tui_use_bin()
        .arg("trace")
        .arg("--artifacts")
        .arg(&non_existent)
        .arg("-o")
        .arg("/tmp/trace.html")
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "should fail on missing artifacts"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read") || stderr.contains("No such file"),
        "should report missing file: {stderr}"
    );
}

#[test]
fn trace_defaults_output_to_trace_html() {
    let artifacts_dir = tempdir().expect("create temp dir");
    create_mock_artifacts(artifacts_dir.path());

    // Run from the temp dir so trace.html is created there
    let output = tui_use_bin()
        .current_dir(artifacts_dir.path())
        .arg("trace")
        .arg("--artifacts")
        .arg(".")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "trace should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let default_output = artifacts_dir.path().join("trace.html");
    assert!(
        default_output.exists(),
        "should create trace.html in current directory"
    );
}

#[test]
fn trace_help_shows_options() {
    let output = tui_use_bin()
        .arg("trace")
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--artifacts"), "should show --artifacts option");
    assert!(stdout.contains("--output") || stdout.contains("-o"), "should show --output option");
}

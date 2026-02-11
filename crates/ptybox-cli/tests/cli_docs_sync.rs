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
use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "help command failed: {:?}",
        output.status
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_flags_documented(help: &str, docs: &str, flags: &[&str], command: &str) {
    for flag in flags {
        assert!(
            help.contains(flag),
            "{command} help missing expected flag {flag}"
        );
        assert!(
            docs.contains(flag),
            "docs missing {command} flag {flag}; update docs/src/reference/cli.md"
        );
    }
}

#[test]
fn cli_reference_tracks_help_flags() {
    let root = workspace_root();
    let cli_docs = fs::read_to_string(root.join("docs/src/reference/cli.md")).unwrap();

    let exec_help = run_help(&["exec", "--help"]);
    let run_help_text = run_help(&["run", "--help"]);
    let driver_help = run_help(&["driver", "--help"]);

    let exec_flags = [
        "--json",
        "--policy",
        "--explain-policy",
        "--cwd",
        "--artifacts",
        "--overwrite",
        "--no-sandbox",
        "--ack-unsafe-sandbox",
        "--enable-network",
        "--ack-unsafe-network",
        "--strict-write",
        "--ack-unsafe-write",
    ];
    assert_flags_documented(&exec_help, &cli_docs, &exec_flags, "exec");

    let run_flags = [
        "--json",
        "--scenario",
        "--explain-policy",
        "--verbose",
        "--tui",
        "--artifacts",
        "--overwrite",
        "--no-sandbox",
        "--ack-unsafe-sandbox",
        "--enable-network",
        "--ack-unsafe-network",
        "--strict-write",
        "--ack-unsafe-write",
    ];
    assert_flags_documented(&run_help_text, &cli_docs, &run_flags, "run");

    let driver_flags = [
        "--stdio",
        "--json",
        "--policy",
        "--cwd",
        "--artifacts",
        "--overwrite",
        "--no-sandbox",
        "--ack-unsafe-sandbox",
        "--enable-network",
        "--ack-unsafe-network",
        "--strict-write",
        "--ack-unsafe-write",
    ];
    assert_flags_documented(&driver_help, &cli_docs, &driver_flags, "driver");
}

#[test]
fn protocol_docs_track_protocol_help_json() {
    let root = workspace_root();
    let protocol_docs = fs::read_to_string(root.join("docs/src/reference/protocol.md")).unwrap();
    let agent_docs = fs::read_to_string(root.join("docs/src/guides/ai-agents.md")).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let protocol_version = value["protocol_version"].as_u64().unwrap();

    let schemas = value["schemas"]
        .as_object()
        .expect("protocol-help must include schemas object");
    assert!(schemas.contains_key("DriverRequestV2"));
    assert!(schemas.contains_key("DriverResponseV2"));

    assert!(
        protocol_docs.contains(&format!("Current protocol version: `{protocol_version}`")),
        "protocol docs must declare current protocol version"
    );

    for token in [
        "DriverRequestV2",
        "DriverResponseV2",
        "request_id",
        "status",
        "action_metrics",
    ] {
        assert!(
            protocol_docs.contains(token),
            "protocol docs missing token {token}"
        );
    }

    for token in [
        "request_id",
        "\"protocol_version\":2",
        "driver-actions.jsonl",
    ] {
        assert!(
            agent_docs.contains(token),
            "ai-agents guide missing {token}"
        );
    }
}

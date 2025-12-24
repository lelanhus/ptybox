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

use std::process::Command;

fn assert_help_contains(args: &[&str], needle: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(needle), "missing help: {needle}\n{stdout}");
}

#[test]
fn top_level_help_mentions_subcommands() {
    assert_help_contains(&["--help"], "exec");
    assert_help_contains(&["--help"], "run");
    assert_help_contains(&["--help"], "driver");
}

#[test]
fn exec_help_mentions_unsafe_flags() {
    assert_help_contains(&["exec", "--help"], "--no-sandbox");
    assert_help_contains(&["exec", "--help"], "--ack-unsafe-sandbox");
    assert_help_contains(&["exec", "--help"], "--enable-network");
    assert_help_contains(&["exec", "--help"], "--ack-unsafe-network");
    assert_help_contains(&["exec", "--help"], "--strict-write");
    assert_help_contains(&["exec", "--help"], "--ack-unsafe-write");
    assert_help_contains(&["exec", "--help"], "--artifacts");
    assert_help_contains(&["exec", "--help"], "--overwrite");
}

#[test]
fn run_help_mentions_unsafe_flags() {
    assert_help_contains(&["run", "--help"], "--no-sandbox");
    assert_help_contains(&["run", "--help"], "--ack-unsafe-sandbox");
    assert_help_contains(&["run", "--help"], "--enable-network");
    assert_help_contains(&["run", "--help"], "--ack-unsafe-network");
    assert_help_contains(&["run", "--help"], "--strict-write");
    assert_help_contains(&["run", "--help"], "--ack-unsafe-write");
    assert_help_contains(&["run", "--help"], "--artifacts");
    assert_help_contains(&["run", "--help"], "--overwrite");
}

#[test]
fn driver_help_mentions_write_flags() {
    assert_help_contains(&["driver", "--help"], "--strict-write");
    assert_help_contains(&["driver", "--help"], "--ack-unsafe-write");
}

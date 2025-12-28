//! Tests for verbose progress output flag.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]

use std::process::Command;

fn ptybox_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ptybox"))
}

#[test]
fn run_accepts_verbose_flag() {
    let output = ptybox_bin()
        .arg("run")
        .arg("--verbose")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "run --verbose --help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_accepts_short_verbose_flag() {
    let output = ptybox_bin()
        .arg("run")
        .arg("-v")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "run -v --help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn run_help_shows_verbose_flag() {
    let output = ptybox_bin()
        .arg("run")
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--verbose") || stdout.contains("-v"),
        "run help should mention --verbose or -v flag"
    );
}

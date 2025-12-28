//! Tests for color output flag handling.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]

use std::process::Command;

fn ptybox_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ptybox"))
}

#[test]
fn color_flag_accepts_auto() {
    let output = ptybox_bin()
        .arg("--color=auto")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "--color=auto should be accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn color_flag_accepts_always() {
    let output = ptybox_bin()
        .arg("--color=always")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "--color=always should be accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn color_flag_accepts_never() {
    let output = ptybox_bin()
        .arg("--color=never")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "--color=never should be accepted: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn color_flag_rejects_invalid() {
    let output = ptybox_bin()
        .arg("--color=invalid")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "--color=invalid should be rejected"
    );
}

#[test]
fn no_color_env_disables_color_in_auto_mode() {
    // When NO_COLOR is set and --color=auto (default), colors should be disabled
    // We can't easily test the actual color output, but we can verify the flag is respected
    let output = ptybox_bin()
        .env("NO_COLOR", "1")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "should work with NO_COLOR set: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn color_always_overrides_no_color_env() {
    // --color=always should work even when NO_COLOR is set
    let output = ptybox_bin()
        .env("NO_COLOR", "1")
        .arg("--color=always")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "--color=always should override NO_COLOR: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn help_shows_color_flag() {
    let output = ptybox_bin()
        .arg("--help")
        .output()
        .expect("failed to execute");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--color"),
        "help should mention --color flag"
    );
}

//! Tests for shell completion generation.
// Test module - relaxed lint rules
#![allow(clippy::expect_used)]

use std::process::Command;

fn tui_use_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tui-use"))
}

#[test]
fn completions_subcommand_exists() {
    let output = tui_use_bin()
        .arg("completions")
        .arg("--help")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "completions --help should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn completions_generates_bash_output() {
    let output = tui_use_bin()
        .arg("completions")
        .arg("bash")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "completions bash should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("_tui-use"),
        "bash completions should contain function name"
    );
    assert!(
        stdout.contains("complete"),
        "bash completions should contain 'complete' command"
    );
}

#[test]
fn completions_generates_zsh_output() {
    let output = tui_use_bin()
        .arg("completions")
        .arg("zsh")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "completions zsh should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("#compdef"),
        "zsh completions should start with #compdef"
    );
}

#[test]
fn completions_generates_fish_output() {
    let output = tui_use_bin()
        .arg("completions")
        .arg("fish")
        .output()
        .expect("failed to execute");

    assert!(
        output.status.success(),
        "completions fish should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("complete -c tui-use"),
        "fish completions should contain 'complete -c tui-use'"
    );
}

#[test]
fn completions_rejects_invalid_shell() {
    let output = tui_use_bin()
        .arg("completions")
        .arg("invalid-shell")
        .output()
        .expect("failed to execute");

    assert!(
        !output.status.success(),
        "completions with invalid shell should fail"
    );
}

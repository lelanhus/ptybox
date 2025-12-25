# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`tui-use` is a security-focused harness for driving terminal UI (TUI) applications with a stable JSON/NDJSON protocol. It enables automated agents (including LLMs) to interact with TUI apps via keys/text/resize/wait and verify behavior via deterministic terminal screen snapshots and transcripts. Designed for macOS-first with Linux container support.

## Build & Test Commands

```bash
# Build all crates
cargo build --workspace

# Run all tests
cargo test --workspace --all-features

# Run a single test
cargo test --workspace test_name

# Lint and format (CI enforces zero warnings)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings

# CLI example: run a command under policy
tui-use exec --json --policy spec/examples/policy.json --artifacts /tmp/artifacts -- /bin/echo hello

# Container smoke test
scripts/container-smoke.sh
```

## Architecture

### Workspace Structure
- `crates/tui_use` — Core library: PTY session management, terminal emulation, policy enforcement, artifacts
- `crates/tui-use-cli` — CLI binary (`tui-use`): exec, run, replay, driver commands
- `crates/tui-use-fixtures` — Test fixtures and helpers

### Library Modules (tui_use)
- `session` — PTY lifecycle, spawn/read/write/resize/terminate
- `terminal` — ANSI/VT parsing via vt100, produces canonical `ScreenSnapshot`
- `policy` — Deny-by-default policy validation, sandbox profile generation (Seatbelt on macOS)
- `runner` — Step execution, scenario loading, wait conditions
- `artifacts` — Transcript, snapshots, checksums, run summary to disk
- `replay` — Replay comparison with normalization filters
- `scenario` — Scenario/policy file parsing (JSON/YAML)
- `assertions` — Assertion engine for screen/transcript verification
- `model` — All domain types: Policy, Scenario, RunResult, Observation, etc.

### Key Entry Points
- Library: `tui_use::run::run_scenario()`, `tui_use::run::run_exec()`
- Session API: `Session::spawn()`, `Session::send()`, `Session::observe()`

## Spec-First Discipline

**Source of truth documents:**
- `spec/data-model.md` — Canonical types, protocols, error codes
- `spec/plan.md` — Architecture, milestones, design principles
- `spec/feature-list.json` — Feature completeness tracking

**Change control:** Any public type, CLI protocol, or default behavior change must update:
1. `spec/data-model.md`
2. `spec/feature-list.json`
3. `CHANGELOG.md`

## Code Standards

- **No warnings**: Treat warnings as errors
- **No `unwrap()`/`expect()` outside tests**: Propagate errors with context
- **`#![forbid(unsafe_code)]`**: Library uses no unsafe code
- **Fail fast and loud**: Explicit errors with stable codes (e.g., `E_POLICY_DENIED`)
- **Deny-by-default**: Policy must explicitly allow any privilege

## Security Model

- Sandbox (Seatbelt) enabled by default on macOS; requires explicit `--no-sandbox --ack-unsafe-sandbox` to disable
- Network disabled by default; requires `--enable-network --ack-unsafe-network`
- Filesystem allowlists must be absolute paths; rejects `/`, home dir, system roots
- Write access requires explicit acknowledgement when `allowed_write` is non-empty

## Stable Exit Codes

| Code | Meaning |
|------|---------|
| 2 | Policy denied |
| 3 | Sandbox unavailable |
| 4 | Timeout/budget exceeded |
| 5 | Assertion failed |
| 6 | Process exited unsuccessfully |
| 7 | Terminal parse failure |
| 8 | Protocol version mismatch |
| 9 | Protocol error |
| 10 | I/O failure |
| 11 | Replay mismatch |

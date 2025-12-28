# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`ptybox` is a security-focused harness for driving terminal UI (TUI) applications with a stable JSON/NDJSON protocol. It enables automated agents (including LLMs) to interact with TUI apps via keys/text/resize/wait and verify behavior via deterministic terminal screen snapshots and transcripts. Designed for macOS-first with Linux container support.

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
ptybox exec --json --policy spec/examples/policy.json --artifacts /tmp/artifacts -- /bin/echo hello

# Container smoke test
scripts/container-smoke.sh
```

## Architecture

### Workspace Structure
- `crates/ptybox` — Core library: PTY session management, terminal emulation, policy enforcement, artifacts
- `crates/ptybox-cli` — CLI binary (`ptybox`): exec, run, replay, driver commands
- `crates/ptybox-fixtures` — Test fixtures and helpers

### Library Modules (ptybox)
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

**Run module** (primary entry points):
```rust
ptybox::run::run_scenario(scenario) -> RunnerResult<RunResult>
ptybox::run::run_scenario_with_options(scenario, options) -> RunnerResult<RunResult>
ptybox::run::run_exec(command, args, cwd, policy) -> RunnerResult<RunResult>
ptybox::run::run_exec_with_options(command, args, cwd, policy, options) -> RunnerResult<RunResult>
```

**Session API** (lower-level PTY control):
```rust
Session::spawn(config: SessionConfig) -> Result<Session, RunnerError>
Session::send(action: &Action) -> Result<(), RunnerError>
Session::observe(timeout: Duration) -> Result<Observation, RunnerError>
Session::terminate() -> Result<(), RunnerError>  // Sends SIGTERM
Session::terminate_process_group(grace: Duration) -> Result<Option<ExitStatus>, RunnerError>
Session::wait_for_exit(timeout: Duration) -> Result<Option<ExitStatus>, RunnerError>
```

**Important**: `Session::wait_for()` does NOT exist. Wait conditions are handled internally by the runner via `wait_for_condition()`.

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

## CLI Commands

| Command | Purpose | Key Flags |
|---------|---------|-----------|
| `exec` | Run single command under policy | `--policy`, `--json`, `--artifacts` |
| `run` | Execute scenario file | `--json`, `--artifacts`, `--normalize` |
| `replay` | Compare run against baseline | `--baseline`, `--normalize` |
| `replay-report` | Generate HTML diff report | `--baseline`, `--output` |
| `driver` | Interactive NDJSON protocol | `--policy` |
| `protocol-help` | Show protocol documentation | (none) |
| `trace` | Generate HTML trace viewer | `--output`, `--open` |
| `completions` | Generate shell completions | `--shell bash\|zsh\|fish` |

## Error Codes

| Code | Exit | Factory Method | Description |
|------|------|----------------|-------------|
| `E_POLICY_DENIED` | 2 | `policy_denied()` | Policy validation failed |
| `E_SANDBOX_UNAVAILABLE` | 3 | `sandbox_unavailable()` | Sandbox not available on platform |
| `E_TIMEOUT` | 4 | `timeout()` | Budget or step timeout exceeded |
| `E_ASSERTION_FAILED` | 5 | `assertion_failed()` | Assertion did not pass |
| `E_PROCESS_EXIT` | 6 | `process_exit()` | Process exited with non-zero code |
| `E_TERMINAL_PARSE` | 7 | `terminal_parse()` | Terminal output parsing failed |
| `E_PROTOCOL_VERSION` | 8 | `protocol()` | Protocol version mismatch |
| `E_PROTOCOL` | 9 | `protocol()` | Generic protocol error |
| `E_IO` | 10 | `io()` | I/O operation failed |
| `E_REPLAY_MISMATCH` | 11 | `replay_mismatch()` | Replay comparison failed |
| `E_CLI_INVALID_ARG` | 12 | `protocol()` | Invalid CLI argument |

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
| 12 | Invalid CLI argument |

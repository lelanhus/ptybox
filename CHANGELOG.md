# Changelog

All notable changes to this project are documented in this file.

This project aims to follow “Keep a Changelog” style entries and Semantic Versioning once releases begin.

## [Unreleased]

### Added
- Driver protocol v2 envelopes: `DriverRequestV2` and `DriverResponseV2` with `request_id` correlation and per-action metrics.
- New `ptybox::driver` library module with typed `DriverConfig` and reusable `run_driver()` engine.
- Driver artifact action log `driver-actions.jsonl`, plus generated replay-compatible `scenario.json` for driver sessions.
- JSON schemas for driver protocol v2: `spec/schemas/driver-request-v2.schema.json` and `spec/schemas/driver-response-v2.schema.json`.
- Cargo workspace with library (`crates/ptybox`) and CLI (`crates/ptybox-cli`).
- Core data model types matching the spec (policy, scenario, actions, observations, run results).
- PTY session runner with key/text/resize/terminate support and canonical terminal snapshots.
- Scenario loader (JSON/YAML), runner loop, basic assertions, wait conditions, and budgets.
- Artifacts writer for run.json, snapshots, transcript, policy, and scenario.
- Sandbox profile generation and `sandbox-exec` wrapping for Seatbelt mode.
- CLI commands: `exec`, `run`, and `driver --stdio --json`.
- Basic tests for assertions and scenario JSON/YAML loading.
- Policy field `sandbox_unsafe_ack` to explicitly acknowledge running with `sandbox: none`.
- Policy fields `network_unsafe_ack` and `fs_write_unsafe_ack` for explicit acknowledgements.
- CLI `--explain-policy` for exec/run to report allow/deny without running.
- Policy-driven artifacts configuration: `artifacts.enabled` and `artifacts.dir` can enable artifacts without CLI flags.
- Release verification helper script (`scripts/release-verify.sh`) to validate expected tarballs, binary contents, and checksum integrity.

### Changed
- `PROTOCOL_VERSION` bumped from `1` to `2` (driver cutover to v2 envelope protocol).
- `ptybox driver` CLI now supports the same security/artifact controls as `exec`/`run` (`--policy`, `--cwd`, `--artifacts`, `--overwrite`, sandbox/network/write acknowledgements).
- Driver startup now validates full run policy (including executable allowlist and cwd checks) before spawning sessions.
- Session/runner wait loops now use deadline-based polling instead of fixed sleeps for more deterministic behavior.
- Driver observations now populate structured runtime events (`pty_output`, `pty_eof`) instead of empty defaults.
- Key handling now includes `F1`-`F12` and `Ctrl+<char>` in protocol-consistent parsing.
- Release workflow now enforces artifact guardrails (expected tarballs, non-empty checksums, and checksum verification) before publishing.
- Install docs now prioritize GitHub release binaries with checksum verification, then crates.io install, then source build fallback.
- Crate metadata now includes `readme` and `documentation` fields for publish quality in `ptybox` and `ptybox-cli`.
- Observation messages now include `protocol_version`.
- Sandbox availability check uses inline profile (`sandbox-exec -p`) to verify capability.
- Docs now define Linux container compatibility requirements and make policy language OS-agnostic.
- Docs now include guidance to avoid broad filesystem allowlists like `/` or home directories.
- Filesystem policy validation now rejects allowlisting `/` or the current home directory.
- Filesystem policy validation now rejects allowlisting system roots like `/System`, `/Library`, `/Users`, `/private`, and `/Volumes`.
- Filesystem policy validation now normalizes paths to prevent `..` traversal bypasses.
- Artifacts directory is now enforced to be within `fs.allowed_write` allowlists.
- Network enablement now requires explicit acknowledgement.
- Unsandboxed runs now require explicit acknowledgement that network policy cannot be enforced.
- Filesystem write allowlists now require explicit acknowledgement.
- Artifacts policy validation now requires a directory when artifacts are enabled.
- Artifacts directory paths now must be absolute.
- Added `fs_strict_write` to require explicit acknowledgement for any write access (including artifacts and sandbox profile writes).
- Filesystem allowlists and working_dir now require absolute paths.
- RunConfig cwd now requires an absolute path.
- Driver mode now supports strict-write and enforces write acknowledgements.
- Exec allowlist paths now require absolute paths.
- CLI rejects relative cwd values before running.
- CLI preflight validation now returns E_CLI_INVALID_ARG with a dedicated exit code.
- CLI replay flag conflicts now return E_CLI_INVALID_ARG.
- Added tests to ensure sandbox profiles reflect network policy.
- Added tests to ensure sandbox profiles reflect filesystem allowlists.
- Added tests to ensure artifacts are written on timeouts and assertion failures.
- Driver mode now requires protocol versioned input messages.
- CLI exit codes now map to error classes.
- Replay determinism checks now compare snapshots, transcript, and normalized run results (ignoring non-deterministic IDs/timestamps).
- Replay now records applied normalization filters in artifacts and exposes `--strict` to disable normalization.
- Artifacts now include `events.jsonl` with NDJSON observations, and replay can compare event streams.
- Replay adds `--normalize <filter>` to select normalization filters explicitly.
- Replay accepts `--normalize all` as a shorthand for default normalization filters.
- Replay accepts `--normalize none` to explicitly disable normalization without using `--strict`.
- Replay normalization defaults can be configured via policy (`replay.strict` / `replay.normalization_filters`).
- Policy schema version bumped to 3 to cover replay settings and normalization rules.
- Artifacts now include `checksums.json`, and replay validates checksums for integrity.
- Replay writes `replay.json` and `diff.json` to summarize mismatches.
- Replay supports `--require-events` and `--require-checksums` to enforce artifact presence.
- JSON mode now exits non-zero when a RunResult is `Failed`, mapping exit codes to the failure class while still emitting the run result.
- Scenario timeout errors now include step/action context for deterministic diagnostics.
- Driver protocol mismatch responses now include supported/provided protocol versions for remediation.
- Terminal output now fails fast with `E_TERMINAL_PARSE` when invalid UTF-8 is emitted, and artifacts are preserved for debugging.
- Replay supports `--explain` for resolved normalization settings.
- Replay supports regex-based normalization rules for transcript/snapshot lines (policy-only).
- Replay report command added to read latest replay summary/diff.
- Shell completions for bash, zsh, and fish (`ptybox completions <shell>`).
- Colored output with `--color={auto|always|never}` global flag and `NO_COLOR` support.
- New assertion types: `line_equals`, `line_contains`, `line_matches`, `not_contains`, `screen_empty`, `cursor_visible`, `cursor_hidden`.
- Verbose progress output with `--verbose` flag showing step-by-step progress to stderr.
- Cell-level styling extraction from terminal snapshots via `snapshot_with_cells()` method.
- Static HTML trace viewer (`ptybox trace --artifacts <dir>`) for debugging and visualizing runs.
- Interactive TUI mode (`ptybox run --tui`) for live terminal output and step progress visualization.
- JSON schemas for spec validation: `spec/schemas/scenario.schema.json`, `run-result.schema.json`, `observation.schema.json`.
- Comprehensive rustdoc documentation for all public types in `ptybox::model`, `session`, and `runner` modules.
- Test fixtures crate (`ptybox-fixtures`) with purpose-built TUI programs for testing:
  - `ptybox-echo-keys`: echoes keypresses with byte values for input testing.
  - `ptybox-show-size`: displays terminal dimensions, updates on resize.
  - `ptybox-delay-output`: outputs text after delay for wait condition testing.
  - `ptybox-exit-code`: exits with specified code for exit handling testing.
  - `ptybox-alt-screen`: uses alternate screen buffer for screen mode testing.
  - `ptybox-unicode-test`: prints Unicode/CJK/emoji for charset testing.
- Fixture-based integration tests (`cli_fixtures.rs`) covering Unicode handling, resize actions, exit codes, wait conditions, and driver protocol.
- GitHub Actions release workflow (`.github/workflows/release.yml`) for automated binary releases on tag push.
- Dual MIT/Apache-2.0 licensing with LICENSE-MIT and LICENSE-APACHE files.
- Cargo.toml workspace metadata for crates.io distribution readiness (repository, homepage, keywords, categories).
- CONTRIBUTING.md with contribution guidelines.

### Security
- Fixed Seatbelt profile injection vulnerability (VULN-1): paths containing `"`, `(`, `)`, `\n` are now rejected with `E_POLICY_DENIED`.
- Fixed path traversal normalization (VULN-3): `canonicalize_for_policy()` now tracks depth and prevents escaping root via `..` sequences.
- Fixed symlink bypass vulnerability (VULN-6): policy paths are validated to reject symlinks (with allowlist for system-managed symlinks like `/tmp`).

### Fixed
- Path normalization edge case where `/../etc` incorrectly normalized to `/etc`.
- Symlink-based policy bypass where `/tmp -> /private/tmp` could escape sandbox restrictions.

### Notes
- Fixture-based CLI tests now validate key features (Unicode, resize, exit codes, wait conditions, driver protocol).
- Linux container support is documented; implementation and validation are pending.
- JSON schemas added but full jsonschema validation not integrated (schemas can be used externally).

### Next
- Define and implement a Linux-container sandbox strategy (external sandbox acknowledgement or Linux backend).
- Add container-based compatibility tests and CI coverage.
- Align default policy selection and documentation for macOS vs Linux container runtime behavior.

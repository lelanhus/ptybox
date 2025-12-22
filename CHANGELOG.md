# Changelog

All notable changes to this project are documented in this file.

This project aims to follow “Keep a Changelog” style entries and Semantic Versioning once releases begin.

## [Unreleased]

### Added
- Cargo workspace with library (`crates/tui_use`) and CLI (`crates/tui-use-cli`).
- Core data model types matching the spec (policy, scenario, actions, observations, run results).
- PTY session runner with key/text/resize/terminate support and canonical terminal snapshots.
- Scenario loader (JSON/YAML), runner loop, basic assertions, wait conditions, and budgets.
- Artifacts writer for run.json, snapshots, transcript, policy, and scenario.
- Sandbox profile generation and `sandbox-exec` wrapping for Seatbelt mode.
- CLI commands: `exec`, `run`, and `driver --stdio --json`.
- Basic tests for assertions and scenario JSON/YAML loading.

### Changed
- Observation messages now include `protocol_version`.
- Sandbox availability check uses inline profile (`sandbox-exec -p`) to verify capability.
- Docs now define Linux container compatibility requirements and make policy language OS-agnostic.
- Docs now include guidance to avoid broad filesystem allowlists like `/` or home directories.

### Fixed
- None.

### Notes
- Many acceptance criteria in `spec/feature-list.json` remain unvalidated; fixture-based CLI tests are still pending.
- Linux container support is now a documented requirement; implementation and validation are still pending.

### Next
- Define and implement a Linux-container sandbox strategy (external sandbox acknowledgement or Linux backend).
- Add container-based compatibility tests and CI coverage.
- Align default policy selection and documentation for macOS vs Linux container runtime behavior.

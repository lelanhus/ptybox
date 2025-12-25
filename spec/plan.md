# Plan

Build `tui-use`: a macOS-first, security-focused harness that lets automated agents (including LLMs) interact with TUI applications “like a human” (keys/text/resize/wait) and verify behavior via deterministic terminal screen snapshots + transcripts. It must also run inside Linux containers used for sandboxed LLMs or agent orchestrations. This repo is spec-first: `spec/data-model.md` is the source of truth for types and APIs, `spec/feature-list.json` defines completeness, and `CHANGELOG.md` records changes.

## Guiding principles
- **Secure by default**: deny-by-default policies, sandbox enabled by default, no implicit privilege expansion.
- **Fail fast and loud**: clear error codes, non-zero exit codes, no silent fallbacks (especially around sandboxing).
- **No surprises**: explicit configuration; stable defaults; deterministic, reproducible outputs; explicit “unsafe mode” opt-in.
- **Typesafe**: library-first Rust API; strongly typed internal model; stable, versioned JSON/NDJSON protocol for external callers.
- **Vertical slices**: feature-based modules where each slice owns its types, errors, tests, and docs.
- **Least privilege**: policy validation rejects broad path allowlists (e.g., `/`, the current home directory, or system roots like `/System` and `/Users`).

## Deliverables (v1)
- Rust crate: `tui_use` (programmatic API).
- CLI: `tui-use` (script/LLM friendly).
- PTY-driven session runner:
  - spawn a command with argv (no shell by default)
  - send actions (keys/text/resize/wait/terminate)
  - capture observations (screen snapshots, cursor, transcript deltas)
- Scenario runner:
  - load a scenario file (JSON/YAML)
  - step actions + assertions (with timeouts/retries)
  - deterministic artifacts (snapshots, transcript, run summary)
- Guardrails:
  - strict `Policy` model (filesystem/network/env/exec allowlists)
  - macOS sandbox backend (Seatbelt via `sandbox-exec`) enabled by default
  - Linux container compatibility with explicit external-sandbox acknowledgment when no host backend exists
  - explicit acknowledgements for network enablement, unsandboxed runs (network unenforceable), and any filesystem write allowlists
  - optional extra-strict mode: any write access requires explicit acknowledgement
  - filesystem allowlists and working_dir must use absolute paths
  - CLI supports strict write mode and explicit write acknowledgements
  - RunConfig cwd must be absolute
  - CLI rejects relative cwd values early
  - CLI preflight errors return structured error codes
  - CLI rejects conflicting replay normalization flags with structured errors
  - Sandbox profile generation reflects network policy
  - Sandbox profile generation reflects filesystem allowlists
  - Artifacts are written on run failures for debugging
  - Driver mode enforces strict write mode and explicit acknowledgements
  - Exec allowlist paths must be absolute
  - hard budgets (max runtime, max steps, max output bytes, etc.)
  - explicit `--no-sandbox` / “unsafe mode” gates (off by default)
- Stable programmatic interface:
  - library API (Rust)
  - CLI JSON output and optional interactive NDJSON `--stdio` driver mode

## Non-goals (v1)
- No MCP server or MCP-based integration.
- No Windows support.
- Full Linux host support outside containers is not a v1 goal (containerized Linux is in scope).
- No mouse support; keyboard + terminal semantics only.
- No claim of perfect containment on macOS; document isolation guarantees and residual risks.

## Milestones
### M0 — Specs and conventions
- Establish living docs: `spec/app_spec.txt`, `spec/data-model.md`, `spec/plan.md`, `spec/feature-list.json`.
- Establish change control: update data model + changelog for any API change.

### M1 — PTY session + terminal model
- PTY spawn/read/write/resize/terminate with strict time/output limits.
- Terminal emulator that produces a canonical `ScreenSnapshot`.
- Snapshot normalization hooks to reduce test flake (opt-in, explicit).

### M2 — Scenario runner and assertions
- Scenario format parsing, step execution, “wait until condition” primitives.
- Assertions over snapshots/transcripts + structured step results.

### M3 — Guardrails and sandboxing
- `Policy` evaluation (deny-by-default) enforced at config load and at runtime.
- macOS sandbox adapter enabled by default; error if unavailable unless explicitly overridden.
- Budgets enforced consistently; policy violations are explicit failures.
- Linux container path documented and tested (no macOS-only assumptions; explicit external-sandbox opt-in).

### M4 — Recording, replay, and artifacts
- Capture full transcript + snapshots, with an index for replay.
- Replay runner for determinism/regression testing.
- Record observation streams (`events.jsonl`) and replay normalization filters (`normalization.json`).
- Add artifact integrity checks (`checksums.json`) and replay mismatch reporting (`replay.json`, `diff.json`).
- Add replay flags to require events/checksums for stricter CI enforcement.

### Operations / CI
- Container usage guidance lives in `spec/ci.md`.
- Release safety checklist lives in `spec/audit-checklist.md`.
- Support normalization rules (regex) for nondeterministic transcript/snapshot output (opt-in).

### M5 — API stabilization and DX
- Pin and version the JSON/NDJSON protocol; publish schemas.
- Library API ergonomics: simple “run scenario” and “drive session” entrypoints.
- Human-friendly CLI UX: great `--help`, precise error messages, `--explain-policy`.

### M6 — Hardening and release
- Repeated-run determinism checks in CI on macOS and in Linux containers.
- Security review of defaults; document threat model and residual risks.
- First tagged release.

## Vertical-slice structure (intended)
Each slice owns its own:
- public and internal types
- error types (with stable codes where appropriate)
- tests (unit + fixture-based integration)
- docs updates (data model + changelog for API changes)

Example slices (names illustrative):
- `features/policy` (policy model + evaluation)
- `features/sandbox` (macOS sandbox adapters)
- `features/compat` (Linux container runtime compatibility + detection)
- `features/session` (PTY lifecycle + I/O)
- `features/terminal` (ANSI/VT parsing + canonical snapshots)
- `features/scenario` (scenario DSL + parser)
- `features/runner` (step execution + wait conditions)
- `features/assertions` (assertions engine + reporting)
- `features/artifacts` (transcripts, snapshots, replay)
- `features/protocol` (JSON/NDJSON wire types + versioning)
- `features/cli` (CLI glue, UX, exit codes)

## Validation and “definition of done”
- `spec/feature-list.json` items are marked passing (automated where possible).
- Running unsafe actions without explicit opt-in fails with an actionable error.
- Sandbox is on by default; sandbox downgrade requires explicit user action.
- Determinism: fixture scenarios pass on repeated runs (within defined limits).

## Notes on macOS isolation
Seatbelt sandboxing (e.g. via `sandbox-exec`) is useful but not a perfect boundary and may change across macOS versions. The tool must:
- clearly document what it enforces
- never silently run unsandboxed if the sandbox is unavailable
- offer a stronger-isolation roadmap option (e.g., VM-backed) for high-risk use cases

## Notes on Linux container isolation
Linux containers provide an external sandbox boundary but are not a substitute for explicit policy enforcement. The tool must:
- avoid macOS-only assumptions when running in containers
- require explicit acknowledgement when relying on container isolation
- document container runtime requirements and limitations

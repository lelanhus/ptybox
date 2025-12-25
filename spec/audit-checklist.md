# Production Audit Checklist

Use this checklist before releases or major changes.

## Safety / Guardrails
- Sandbox defaults to enabled; failures produce `E_SANDBOX_UNAVAILABLE`.
- `sandbox: none` requires `sandbox_unsafe_ack: true` and is recorded.
- Network is disabled by default; `network_unsafe_ack: true` required when enabled or when sandbox is none.
- Filesystem allowlists are absolute and reject `/`, home, and system roots.
- Write allowlists require `fs_write_unsafe_ack: true`.
- `fs_strict_write` forces explicit write acknowledgement for any write access.
- Artifacts directory validated against `fs.allowed_write` and path normalization.

## Budgets / Termination
- Runtime, output, snapshot, wait, and step budgets are enforced.
- Timeouts include structured context (step/action/condition).
- Termination kills the process group to avoid leaks.

## Protocol / CLI
- JSON mode emits only JSON/NDJSON to stdout; diagnostics go to stderr.
- Exit codes map to error classes, including failed RunResults.
- Protocol version mismatches return `E_PROTOCOL_VERSION_MISMATCH` with supported/provided versions.
- Malformed JSON/NDJSON returns `E_PROTOCOL` without crashing.
- Container smoke test (optional) passes when Docker is available (`scripts/container-smoke.sh`).

## Artifacts / Replay
- Artifacts include run.json, policy.json, scenario.json, transcript, snapshots, events.jsonl, normalization.json, checksums.json.
- Replay compares snapshots/transcript/run results and produces replay.json/diff.json on mismatch.
- Normalization filters are recorded and versioned.

## Determinism
- Repeated runs of the same scenario produce identical canonical snapshots (within normalization rules).
- Tests cover Unicode/wide characters and alt-screen snapshots.

## Docs / Spec
- spec/data-model.md, spec/app_spec.txt, spec/plan.md, spec/feature-list.json are consistent.
- CHANGELOG.md documents all public behavior changes.
- spec/ci.md reflects current container usage guidance.

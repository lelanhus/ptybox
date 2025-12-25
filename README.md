# tui-use

A security-focused harness for driving terminal UI (TUI) applications with a stable JSON/NDJSON protocol.

## Docs
- Spec and architecture: `spec/plan.md`
- Data model and protocol: `spec/data-model.md`
- CI/container usage: `spec/ci.md`
- Production audit checklist: `spec/audit-checklist.md`
- Feature status: `spec/feature-list.json`

## Quick start (local)
```
# Example: run a command under policy with artifacts enabled
# (See spec/examples/policy.json for a baseline policy.)

tui-use exec --json --policy spec/examples/policy.json --artifacts /tmp/artifacts -- /bin/echo hello
```

## Container smoke test (optional)
```
scripts/container-smoke.sh
```

# CI / Container Usage

This document describes how to run `ptybox` in CI and inside Linux containers.

## Goals
- Keep sandboxing **explicit**: no silent downgrade.
- Keep filesystem writes **scoped**: artifacts only within allowlisted directories.
- Keep network **off by default**.

## Linux container checklist
- `/dev/pts` must be mounted (PTY support).
- The container user must have access to the target executable path.
- Provide an explicit policy with `sandbox: none` and `sandbox_unsafe_ack: true`.
- Because a host sandbox is not available, set `network_unsafe_ack: true` even when network is disabled.
- Allowlist the working directory and artifacts directory explicitly.

Example policy (JSON):
```
{
  "policy_version": 3,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_unsafe_ack": true,
  "fs_write_unsafe_ack": true,
  "fs_strict_write": false,
  "fs": {
    "allowed_read": ["/work"],
    "allowed_write": ["/work/artifacts"],
    "working_dir": "/work"
  },
  "exec": {
    "allowed_executables": ["/bin/echo"],
    "allow_shell": false
  },
  "env": {
    "allowlist": [],
    "set": {},
    "inherit": false
  },
  "budgets": {
    "max_runtime_ms": 60000,
    "max_steps": 10000,
    "max_output_bytes": 8388608,
    "max_snapshot_bytes": 2097152,
    "max_wait_ms": 10000
  },
  "artifacts": {
    "enabled": true,
    "dir": "/work/artifacts",
    "overwrite": true
  },
  "replay": {
    "strict": false,
    "normalization_filters": ["snapshot_id"],
    "normalization_rules": []
  }
}
```

Example CI command:
```
mkdir -p /work/artifacts

ptybox exec --json --policy /work/policy.json --artifacts /work/artifacts -- /bin/echo hello
```

## Container smoke test
Use `scripts/container-smoke.sh` to build and run a minimal exec path inside a Linux container.
This script expects Docker to be available and uses `spec/examples/policy-container.json`.

## Failure modes to expect in CI
- `E_SANDBOX_UNAVAILABLE`: sandbox requested but not available. Use `sandbox: none` with `sandbox_unsafe_ack: true`.
- `E_POLICY_DENIED`: policy missing explicit allowlists or acknowledgements.
- `E_TIMEOUT`: runtime or wait budgets exceeded.
- `E_TERMINAL_PARSE`: invalid terminal output emitted by the target.

## CI hygiene
- Use a dedicated artifacts directory and enable overwrite only when safe.
- Keep `fs.allowed_write` as narrow as possible.
- Keep network disabled unless the test requires it.

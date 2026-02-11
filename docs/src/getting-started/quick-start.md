# Quick Start

## 1) Create a minimal policy

`ptybox` is deny-by-default. Start with an explicit policy:

```json
{
  "policy_version": 4,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_unsafe_ack": true,
  "fs": {
    "allowed_read": ["/tmp"],
    "allowed_write": ["/tmp/ptybox-artifacts"],
    "working_dir": "/tmp"
  },
  "fs_write_unsafe_ack": true,
  "fs_strict_write": false,
  "exec": {
    "allowed_executables": ["/bin/echo", "/bin/cat"],
    "allow_shell": false
  },
  "env": {
    "allowlist": [],
    "set": {},
    "inherit": false
  },
  "budgets": {
    "max_runtime_ms": 30000,
    "max_steps": 100,
    "max_output_bytes": 8388608,
    "max_snapshot_bytes": 2097152,
    "max_wait_ms": 10000
  },
  "artifacts": {
    "enabled": false,
    "dir": null,
    "overwrite": false
  },
  "replay": {
    "strict": false,
    "normalization_filters": null,
    "normalization_rules": null
  }
}
```

Save as `policy.json`.

## 2) Run a command

```bash
ptybox exec --json --policy ./policy.json -- /bin/echo "Hello, ptybox"
```

## 3) Start interactive driver mode

```bash
ptybox driver --stdio --json --policy ./policy.json -- /bin/cat
```

Send NDJSON requests:

```json
{"protocol_version":2,"request_id":"req-1","action":{"type":"text","payload":{"text":"hello"}}}
{"protocol_version":2,"request_id":"req-2","action":{"type":"terminate","payload":{}}}
```

## 4) Save artifacts for replay

```bash
ptybox exec --json --policy ./policy.json --artifacts /tmp/ptybox-artifacts --overwrite -- /bin/echo done
ptybox replay --json --artifacts /tmp/ptybox-artifacts
```

## Next

- [Your First Scenario](first-scenario.md)
- [Scenario Guide](../guides/scenarios.md)
- [AI Agents](../guides/ai-agents.md)
- [Protocol Reference](../reference/protocol.md)

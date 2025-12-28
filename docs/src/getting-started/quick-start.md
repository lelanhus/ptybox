# Quick Start

## Run a Simple Command

The simplest way to use ptybox is with the `exec` command:

```bash
ptybox exec --json -- /bin/echo "Hello, TUI"
```

This runs `/bin/echo` in a PTY and returns structured JSON output including:
- Exit status
- Runtime duration
- Final screen snapshot
- Transcript

## Using a Policy File

For security, ptybox requires explicit policies. Create a minimal policy:

```json
{
  "policy_version": 4,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_unsafe_ack": true,
  "fs": {
    "allowed_read": ["/tmp"],
    "allowed_write": [],
    "working_dir": "/tmp"
  },
  "exec": {
    "allowed_executables": ["/bin/echo"],
    "allow_shell": false
  }
}
```

Save this as `policy.json` and run:

```bash
ptybox exec --json --policy policy.json -- /bin/echo "Secured!"
```

## Interactive Driver Mode

For agent-style interaction, use driver mode:

```bash
ptybox driver --stdio --json -- /bin/cat
```

Then send NDJSON commands via stdin:

```json
{"protocol_version":1,"action":{"type":"text","payload":{"text":"hello"}}}
{"protocol_version":1,"action":{"type":"terminate","payload":{}}}
```

## Next Steps

- [Your First Scenario](first-scenario.md) - Create a complete test scenario
- [Policies](../guides/policies.md) - Learn about security policies
- [CLI Commands](../reference/cli.md) - Full CLI reference

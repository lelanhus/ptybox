# ptybox

**Playwright for Terminal UIs** - A security-focused harness for driving TUI applications with a stable JSON protocol.

## What is ptybox?

ptybox enables automated testing and interaction with terminal UI applications through a structured JSON/NDJSON protocol. It's designed for:

- **AI/LLM agents** that need to interact with CLI tools
- **Integration testing** of TUI applications
- **Automation scripts** requiring deterministic terminal control
- **CI/CD pipelines** validating TUI behavior

## Key Features

| Feature | Description |
|---------|-------------|
| **JSON Protocol** | Stable, versioned NDJSON for agent-friendly automation |
| **Security Sandbox** | Deny-by-default policy with Seatbelt (macOS) enforcement |
| **Deterministic Replay** | Record and replay sessions with normalization filters |
| **Screen Snapshots** | Canonical terminal state with cursor, colors, and Unicode |
| **Assertions** | Built-in `screen_contains`, `regex_match`, `cursor_at` |
| **Stable Exit Codes** | Distinct codes for policy denial, timeout, assertion failure |

## Quick Example

```bash
# Run a command with JSON output
ptybox exec --json -- /bin/echo "Hello, TUI"

# Run a scenario file
ptybox run --json --scenario scenario.yaml

# Interactive driver mode for agents
ptybox driver --stdio --json -- /bin/cat
```

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (x86_64, aarch64) | Full support with Seatbelt sandbox |
| Linux (x86_64, aarch64) | Full support (container-friendly) |
| Windows | Not supported |

## License

Licensed under either of Apache License 2.0 or MIT License, at your option.

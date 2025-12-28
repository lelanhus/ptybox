# ptybox

**Playwright for Terminal UIs** - A security-focused harness for driving TUI applications with a stable JSON protocol.

[![CI](https://github.com/ptybox-rs/ptybox/actions/workflows/ci.yml/badge.svg)](https://github.com/ptybox-rs/ptybox/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

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

## Installation

### From source

```bash
git clone https://github.com/ptybox-rs/ptybox
cd ptybox
cargo build --release
./target/release/ptybox --help
```

### Shell completions

```bash
# Bash
ptybox completions bash > ~/.bash_completion.d/ptybox

# Zsh
ptybox completions zsh > ~/.zfunc/_ptybox

# Fish
ptybox completions fish > ~/.config/fish/completions/ptybox.fish
```

## Quick Start

### 1. Run a command with JSON output

```bash
ptybox exec --json -- /bin/echo "Hello, TUI"
```

### 2. Use a policy file for security

```bash
# Create a minimal policy
cat > policy.json << 'EOF'
{
  "policy_version": 3,
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
EOF

ptybox exec --json --policy policy.json -- /bin/echo "Secured!"
```

### 3. Interactive driver mode

```bash
# Start interactive session
ptybox driver --stdio --json -- /bin/cat

# Send actions via stdin (NDJSON)
{"protocol_version":1,"action":{"type":"text","payload":{"text":"hello"}}}
{"protocol_version":1,"action":{"type":"terminate","payload":{}}}
```

### 4. Run a scenario file

```yaml
# scenario.yaml
scenario_version: 1
metadata:
  name: echo-test
run:
  command: /bin/cat
  args: []
  cwd: /tmp
  initial_size: { rows: 24, cols: 80 }
  policy:
    policy_version: 3
    sandbox: none
    sandbox_unsafe_ack: true
    network: disabled
    network_unsafe_ack: true
    exec:
      allowed_executables: [/bin/cat]
      allow_shell: false
steps:
  - id: type-hello
    name: type hello
    action: { type: text, payload: { text: hello } }
    assert:
      - { type: screen_contains, payload: { text: hello } }
    timeout_ms: 1000
    retries: 0
  - id: terminate
    name: terminate
    action: { type: terminate, payload: {} }
    timeout_ms: 1000
    retries: 0
```

```bash
ptybox run --json --scenario scenario.yaml
```

## Comparison

| Feature | ptybox | Pexpect | VHS | Expectrl |
|---------|---------|---------|-----|----------|
| JSON Protocol | Yes | No | No | No |
| Security Sandbox | Yes | No | No | No |
| Deterministic Replay | Yes | No | No | No |
| Screen Snapshots | Yes | No | Visual only | No |
| Assertions | Yes | No | No | No |
| Agent/LLM Friendly | Yes | No | No | No |
| Stable Exit Codes | Yes | No | No | No |

## Exit Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | Success | Run completed successfully |
| 2 | E_POLICY_DENIED | Policy validation failed |
| 3 | E_SANDBOX_UNAVAILABLE | Sandbox backend unavailable |
| 4 | E_TIMEOUT | Timeout or budget exceeded |
| 5 | E_ASSERTION_FAILED | Assertion did not pass |
| 6 | E_PROCESS_FAILED | Target process exited non-zero |
| 7 | E_TERMINAL_PARSE | Terminal output parse failure |
| 8 | E_PROTOCOL_VERSION_MISMATCH | Incompatible protocol version |
| 9 | E_PROTOCOL | Malformed protocol message |
| 10 | E_IO | I/O failure |
| 11 | E_REPLAY_MISMATCH | Replay comparison failed |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        ptybox CLI                          │
│  exec │ run │ driver │ replay │ replay-report │ completions │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                      ptybox Library                         │
├──────────┬──────────┬──────────┬──────────┬─────────────────┤
│ session  │ terminal │ policy   │ runner   │ artifacts       │
│ (PTY)    │ (VT100)  │ (sandbox)│ (steps)  │ (snapshots)     │
└──────────┴──────────┴──────────┴──────────┴─────────────────┘
```

## Documentation

- **[Data Model & Protocol](spec/data-model.md)** - Types, JSON schemas, error codes
- **[Architecture & Plan](spec/plan.md)** - Design principles, milestones
- **[CI & Containers](spec/ci.md)** - Running in Docker/Podman
- **[Changelog](CHANGELOG.md)** - Version history

## Platform Support

| Platform | Status |
|----------|--------|
| macOS (x86_64, aarch64) | Full support with Seatbelt sandbox |
| Linux (x86_64, aarch64) | Full support (container-friendly) |
| Windows | Not supported |

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Development

This project was developed entirely using AI coding assistants:

- **[Claude Code](https://claude.ai/code)** - Anthropic's CLI for Claude
- **[Codex CLI](https://github.com/openai/codex)** - OpenAI's coding assistant

The entire codebase, tests, documentation, and CI configuration were written through AI pair programming.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

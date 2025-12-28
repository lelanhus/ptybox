# CLI Commands

## ptybox exec

Run a single command under policy control.

```bash
ptybox exec [OPTIONS] -- <COMMAND> [ARGS]...
```

### Options

| Flag | Description |
|------|-------------|
| `--json` | Output structured JSON |
| `--policy <FILE>` | Policy file path |
| `--artifacts <DIR>` | Write artifacts to directory |
| `--cwd <DIR>` | Working directory |
| `--timeout <MS>` | Maximum runtime in milliseconds |
| `--no-sandbox` | Disable sandbox |
| `--ack-unsafe-sandbox` | Acknowledge unsafe sandbox mode |
| `--enable-network` | Enable network access |
| `--ack-unsafe-network` | Acknowledge unsafe network mode |

### Example

```bash
ptybox exec --json --policy policy.json --artifacts ./out -- /bin/echo hello
```

---

## ptybox run

Execute a scenario file.

```bash
ptybox run [OPTIONS] --scenario <FILE>
```

### Options

| Flag | Description |
|------|-------------|
| `--scenario <FILE>` | Scenario file (YAML/JSON) |
| `--json` | Output structured JSON |
| `--artifacts <DIR>` | Write artifacts to directory |
| `--normalize <FILTER>` | Apply normalization filter |
| `--verbose` / `-v` | Verbose output |
| `--tui` | Show TUI progress display |

### Example

```bash
ptybox run --json --scenario test.yaml --artifacts ./results
```

---

## ptybox driver

Interactive NDJSON protocol mode.

```bash
ptybox driver [OPTIONS] -- <COMMAND> [ARGS]...
```

### Options

| Flag | Description |
|------|-------------|
| `--stdio` | Use stdin/stdout for protocol |
| `--json` | JSON output mode |
| `--policy <FILE>` | Policy file path |

### Protocol

Send actions as NDJSON on stdin:

```json
{"protocol_version":1,"action":{"type":"text","payload":{"text":"hello"}}}
```

Receive observations on stdout.

---

## ptybox replay

Compare run against baseline.

```bash
ptybox replay [OPTIONS] --baseline <DIR> --artifacts <DIR>
```

### Options

| Flag | Description |
|------|-------------|
| `--baseline <DIR>` | Baseline artifacts directory |
| `--artifacts <DIR>` | Current run artifacts |
| `--normalize <FILTER>` | Apply normalization |
| `--strict` | Disable all normalization |
| `--require-events` | Require events.jsonl |
| `--require-checksums` | Require checksums.json |

---

## ptybox replay-report

Generate HTML diff report.

```bash
ptybox replay-report --baseline <DIR> --output <FILE>
```

---

## ptybox trace

Generate HTML trace viewer.

```bash
ptybox trace --artifacts <DIR> --output <FILE> [--open]
```

---

## ptybox protocol-help

Show protocol documentation.

```bash
ptybox protocol-help
```

---

## ptybox completions

Generate shell completions.

```bash
ptybox completions <SHELL>
```

Shells: `bash`, `zsh`, `fish`

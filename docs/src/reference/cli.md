# CLI Commands

## `ptybox exec`

Run a single command under policy control.

```bash
ptybox exec [OPTIONS] -- <COMMAND> [ARGS]...
```

### Key options

| Flag | Description |
|---|---|
| `--json` | Emit machine-readable JSON output |
| `--policy <FILE>` | Load policy JSON from file |
| `--explain-policy` | Validate/describe policy without running command |
| `--cwd <DIR>` | Override policy working directory (absolute path) |
| `--artifacts <DIR>` | Write artifacts bundle to directory |
| `--overwrite` | Allow overwriting an existing artifacts directory |
| `--no-sandbox` + `--ack-unsafe-sandbox` | Disable sandboxing explicitly |
| `--enable-network` + `--ack-unsafe-network` | Enable network explicitly |
| `--strict-write` + `--ack-unsafe-write` | Enable strict write mode and acknowledge write risk |

### Example

```bash
ptybox exec --json --policy ./policy.json --artifacts ./out -- /bin/echo hello
```

---

## `ptybox run`

Execute a scenario (`.json`, `.yaml`, or `.yml`).

```bash
ptybox run [OPTIONS] --scenario <FILE>
```

### Key options

| Flag | Description |
|---|---|
| `--json` | Emit machine-readable JSON output |
| `--scenario <FILE>` | Scenario file path |
| `--explain-policy` | Validate/describe scenario policy without running |
| `--verbose` / `-v` | Print step-by-step progress to stderr |
| `--tui` | Show live interactive TUI progress |
| `--artifacts <DIR>` | Write artifacts bundle |
| `--overwrite` | Allow artifacts overwrite |
| `--no-sandbox` + `--ack-unsafe-sandbox` | Disable sandboxing explicitly |
| `--enable-network` + `--ack-unsafe-network` | Enable network explicitly |
| `--strict-write` + `--ack-unsafe-write` | Enable strict write mode and acknowledge write risk |

### Example

```bash
ptybox run --json --scenario ./scenario.yaml --artifacts ./artifacts
```

---

## `ptybox driver`

Interactive NDJSON control loop for agentic use.

```bash
ptybox driver --stdio --json [OPTIONS] -- <COMMAND> [ARGS]...
```

### Key options

| Flag | Description |
|---|---|
| `--stdio` | Required: read/write NDJSON via stdin/stdout |
| `--json` | Required: JSON protocol mode |
| `--policy <FILE>` | Load policy JSON from file (recommended) |
| `--cwd <DIR>` | Override policy working directory (absolute path) |
| `--artifacts <DIR>` | Write artifacts bundle (includes `driver-actions.jsonl`) |
| `--overwrite` | Allow artifacts overwrite |
| `--no-sandbox` + `--ack-unsafe-sandbox` | Disable sandboxing explicitly |
| `--enable-network` + `--ack-unsafe-network` | Enable network explicitly |
| `--strict-write` + `--ack-unsafe-write` | Enable strict write mode and acknowledge write risk |

### Protocol (v2)

Driver input is `DriverRequestV2` NDJSON:

```json
{"protocol_version":2,"request_id":"req-1","action":{"type":"text","payload":{"text":"hello"}},"timeout_ms":250}
```

Driver output is `DriverResponseV2` NDJSON:

```json
{"protocol_version":2,"request_id":"req-1","status":"ok","observation":{...},"error":null,"action_metrics":{"sequence":1,"duration_ms":5}}
```

---

## `ptybox replay`

Re-run the scenario captured in an artifacts directory and compare outputs deterministically.

```bash
ptybox replay [OPTIONS] --artifacts <DIR>
```

### Key options

| Flag | Description |
|---|---|
| `--json` | Emit machine-readable JSON/error output |
| `--artifacts <DIR>` | Artifacts directory to replay against |
| `--strict` | Disable normalization filters |
| `--normalize <FILTER>` | Override normalization filters (`all`, `none`, `snapshot_id`, `run_id`, `run_timestamps`, `step_timestamps`, `observation_timestamp`, `session_id`) |
| `--explain` | Print resolved normalization settings and exit |
| `--require-events` | Require `events.jsonl` in original and replay artifacts |
| `--require-checksums` | Require and validate `checksums.json` |

---

## `ptybox replay-report`

Read the latest replay summary from an artifacts directory.

```bash
ptybox replay-report [--json] --artifacts <DIR>
```

---

## `ptybox trace`

Generate an HTML trace from artifacts.

```bash
ptybox trace --artifacts <DIR> [-o <FILE>]
```

---

## `ptybox protocol-help`

Emit protocol documentation for agents.

```bash
ptybox protocol-help [--json]
```

---

## `ptybox completions`

Generate shell completions.

```bash
ptybox completions <bash|zsh|fish>
```

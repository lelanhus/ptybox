# Scenarios

Scenarios define deterministic TUI workflows for `ptybox run`.

## Structure

```yaml
scenario_version: 1
metadata:
  name: my-test
  description: Optional description

run:
  command: /path/to/executable
  args: ["arg1", "arg2"]
  cwd: /absolute/working/dir
  initial_size: { rows: 24, cols: 80 }
  policy:
    path: /absolute/path/to/policy.json

steps:
  - id: step-1
    name: Type input
    action:
      type: text
      payload: { text: "hello" }
    assert:
      - type: screen_contains
        payload: { text: "hello" }
    timeout_ms: 5000
    retries: 0
```

## `run.policy` forms

`run.policy` is an untagged union:

1. Inline policy object (`Policy`)
2. File reference object with `path`

### File reference example

```yaml
run:
  policy:
    path: /absolute/path/to/policy.json
```

### Inline policy example

```yaml
run:
  policy:
    policy_version: 4
    sandbox: none
    sandbox_unsafe_ack: true
    network: disabled
    network_unsafe_ack: true
    fs:
      allowed_read: [/tmp]
      allowed_write: [/tmp/artifacts]
      working_dir: /tmp
    fs_write_unsafe_ack: true
    fs_strict_write: false
    exec:
      allowed_executables: [/bin/cat]
      allow_shell: false
    env:
      allowlist: []
      set: {}
      inherit: false
    budgets:
      max_runtime_ms: 30000
      max_steps: 100
      max_output_bytes: 8388608
      max_snapshot_bytes: 2097152
      max_wait_ms: 10000
    artifacts:
      enabled: false
      dir: null
      overwrite: false
    replay:
      strict: false
      normalization_filters: null
      normalization_rules: null
```

## Action types

| Type | Payload | Description |
|---|---|---|
| `text` | `{ "text": "..." }` | Send text input |
| `key` | `{ "key": "Enter" }` | Send key input |
| `resize` | `{ "rows": 40, "cols": 120 }` | Resize terminal |
| `wait` | `{ "condition": { ... } }` | Wait for condition |
| `terminate` | `{}` | Terminate process |

## Wait conditions

`wait` action condition types:

- `screen_contains` with `payload.text`
- `screen_matches` with `payload.pattern` (Rust regex)
- `cursor_at` with `payload.row` and `payload.col`
- `process_exited` with empty payload

Example:

```yaml
action:
  type: wait
  payload:
    condition:
      type: screen_contains
      payload:
        text: "Ready"
```

Use step-level `timeout_ms` and `retries` to control wait budget and retries.

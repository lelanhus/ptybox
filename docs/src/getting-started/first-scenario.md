# Your First Scenario

Scenarios define reproducible TUI interactions with assertions.

## Create `scenario.yaml`

```yaml
scenario_version: 1
metadata:
  name: echo-test
  description: Verify cat echoes typed text

run:
  command: /bin/cat
  args: []
  cwd: /tmp
  initial_size: { rows: 24, cols: 80 }
  policy:
    policy_version: 4
    sandbox: none
    sandbox_unsafe_ack: true
    network: disabled
    network_unsafe_ack: true
    fs:
      allowed_read: [/tmp]
      allowed_write: [/tmp/ptybox-artifacts]
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

steps:
  - id: type-hello
    name: Type hello
    action:
      type: text
      payload: { text: "hello" }
    assert:
      - type: screen_contains
        payload: { text: "hello" }
    timeout_ms: 1000
    retries: 0

  - id: terminate
    name: Terminate process
    action:
      type: terminate
      payload: {}
    timeout_ms: 1000
    retries: 0
```

## Run the scenario

```bash
ptybox run --json --scenario scenario.yaml
```

## Capture artifacts

```bash
ptybox run --json --scenario scenario.yaml --artifacts /tmp/ptybox-artifacts --overwrite
```

Artifacts include:

- `run.json`
- `events.jsonl`
- `transcript.log`
- `snapshots/`
- `policy.json`
- `scenario.json`
- `checksums.json`

## Example result shape

```json
{
  "run_result_version": 1,
  "protocol_version": 2,
  "run_id": "...",
  "status": "passed",
  "steps": [
    {
      "name": "Type hello",
      "status": "passed"
    }
  ],
  "error": null
}
```

## Next

- [Scenarios Guide](../guides/scenarios.md)
- [Replay Guide](../guides/replay.md)

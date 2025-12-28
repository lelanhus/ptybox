# Your First Scenario

Scenarios define complete test cases with steps, actions, and assertions.

## Create a Scenario File

Create `scenario.yaml`:

```yaml
scenario_version: 1
metadata:
  name: echo-test
  description: Test that cat echoes input

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
    exec:
      allowed_executables: [/bin/cat]
      allow_shell: false

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
    name: Terminate
    action:
      type: terminate
      payload: {}
    timeout_ms: 1000
    retries: 0
```

## Run the Scenario

```bash
ptybox run --json --scenario scenario.yaml
```

## With Artifacts

Save snapshots and transcripts for debugging:

```bash
ptybox run --json --scenario scenario.yaml --artifacts ./artifacts
```

This creates:
- `artifacts/run.json` - Run result
- `artifacts/transcript.log` - Full terminal output
- `artifacts/events.jsonl` - Observation stream
- `artifacts/snapshots/` - Screen snapshots

## Understanding the Output

The JSON output includes:

```json
{
  "protocol_version": 1,
  "run_id": "...",
  "passed": true,
  "steps": [
    {
      "step_id": "type-hello",
      "passed": true,
      "duration_ms": 42
    }
  ],
  "exit_status": 0
}
```

## Next Steps

- [Scenarios Guide](../guides/scenarios.md) - Advanced scenario features
- [Assertions](../guides/assertions.md) - All assertion types
- [Replay](../guides/replay.md) - Record and replay

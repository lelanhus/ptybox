# Scenarios

Scenarios are YAML or JSON files that define complete test cases.

## Structure

```yaml
scenario_version: 1
metadata:
  name: my-test
  description: Optional description

run:
  command: /path/to/executable
  args: ["arg1", "arg2"]
  cwd: /working/directory
  initial_size: { rows: 24, cols: 80 }
  policy: { ... }  # Inline or file reference

steps:
  - id: step-1
    name: Human-readable name
    action: { type: text, payload: { text: "input" } }
    assert:
      - { type: screen_contains, payload: { text: "expected" } }
    timeout_ms: 5000
    retries: 2
```

## Action Types

| Type | Payload | Description |
|------|---------|-------------|
| `text` | `{ text: "..." }` | Send text input |
| `key` | `{ key: "Enter" }` | Send special key |
| `resize` | `{ rows: 40, cols: 120 }` | Resize terminal |
| `wait` | `{ duration_ms: 100 }` | Wait fixed duration |
| `terminate` | `{}` | Send SIGTERM |

## Wait Conditions

Instead of fixed waits, use wait conditions:

```yaml
action:
  type: wait_for_text
  payload:
    text: "Ready"
    timeout_ms: 5000
```

Available conditions:
- `wait_for_text` - Wait until screen contains text
- `wait_for_regex` - Wait until screen matches pattern
- `wait_for_cursor` - Wait until cursor at position

## Policy Reference

Reference an external policy file:

```yaml
run:
  policy_file: ./policy.json
```

Or inline the policy directly in the scenario.

# Example Scenarios

## Basic Echo Test

Test that input is echoed:

```yaml
scenario_version: 1
metadata:
  name: echo-test

run:
  command: /bin/cat
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

steps:
  - id: type-hello
    name: Type hello
    action: { type: text, payload: { text: "hello" } }
    assert:
      - { type: screen_contains, payload: { text: "hello" } }
    timeout_ms: 1000

  - id: terminate
    name: Terminate
    action: { type: terminate, payload: {} }
    timeout_ms: 1000
```

## Wait for Prompt

Wait until a specific prompt appears:

```yaml
steps:
  - id: wait-prompt
    name: Wait for prompt
    action:
      type: wait_for_text
      payload:
        text: "$ "
        timeout_ms: 5000
    assert:
      - { type: screen_contains, payload: { text: "$ " } }
```

## Resize Terminal

Test application response to resize:

```yaml
steps:
  - id: resize
    name: Resize to 40x120
    action:
      type: resize
      payload: { rows: 40, cols: 120 }
    timeout_ms: 1000

  - id: verify-size
    name: Verify size reported
    action: { type: wait, payload: { duration_ms: 100 } }
    assert:
      - { type: screen_contains, payload: { text: "40" } }
```

## Key Navigation

Test arrow key navigation:

```yaml
steps:
  - id: down-arrow
    name: Press down arrow
    action: { type: key, payload: { key: "Down" } }
    timeout_ms: 500

  - id: enter
    name: Press enter
    action: { type: key, payload: { key: "Enter" } }
    timeout_ms: 500
```

## Error Detection

Verify no errors appear:

```yaml
steps:
  - id: run-command
    name: Run command
    action: { type: text, payload: { text: "some-command\n" } }
    assert:
      - { type: not_contains, payload: { text: "Error" } }
      - { type: not_contains, payload: { text: "error:" } }
    timeout_ms: 5000
```

## More Examples

See the `spec/examples/` directory in the repository for additional scenario files.

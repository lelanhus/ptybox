# Assertions

Assertions verify terminal state after each step.

## Assertion Types

### screen_contains

Check if text appears anywhere on screen:

```yaml
assert:
  - type: screen_contains
    payload: { text: "Welcome" }
```

### not_contains

Check that text does NOT appear:

```yaml
assert:
  - type: not_contains
    payload: { text: "Error" }
```

### regex_match

Match screen content against a regex:

```yaml
assert:
  - type: regex_match
    payload: { pattern: "Version \\d+\\.\\d+" }
```

### line_equals

Check exact line content:

```yaml
assert:
  - type: line_equals
    payload: { line: 0, text: "Header Line" }
```

### line_contains

Check if a line contains text:

```yaml
assert:
  - type: line_contains
    payload: { line: 5, text: "Status:" }
```

### line_matches

Match a line against regex:

```yaml
assert:
  - type: line_matches
    payload: { line: 0, pattern: "^\\[\\d+\\]" }
```

### cursor_at

Verify cursor position:

```yaml
assert:
  - type: cursor_at
    payload: { row: 10, col: 0 }
```

### cursor_visible / cursor_hidden

Check cursor visibility:

```yaml
assert:
  - type: cursor_visible
  - type: cursor_hidden
```

### screen_empty

Verify screen has no content:

```yaml
assert:
  - type: screen_empty
```

## Multiple Assertions

Steps can have multiple assertions (all must pass):

```yaml
assert:
  - type: screen_contains
    payload: { text: "Ready" }
  - type: not_contains
    payload: { text: "Error" }
  - type: cursor_visible
```

## Retries

For flaky assertions, use retries:

```yaml
steps:
  - id: wait-for-ready
    action: { type: wait, payload: { duration_ms: 100 } }
    assert:
      - type: screen_contains
        payload: { text: "Ready" }
    timeout_ms: 5000
    retries: 10  # Retry up to 10 times
```

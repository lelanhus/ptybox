# Protocol Reference

ptybox uses NDJSON (newline-delimited JSON) for agent communication.

## Protocol Version

Current version: `1`

All messages include `protocol_version`:

```json
{"protocol_version": 1, ...}
```

## Actions

### Text Input

```json
{
  "protocol_version": 1,
  "action": {
    "type": "text",
    "payload": { "text": "hello world" }
  }
}
```

### Key Press

```json
{
  "protocol_version": 1,
  "action": {
    "type": "key",
    "payload": { "key": "Enter" }
  }
}
```

Supported keys:
- `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`
- `Up`, `Down`, `Left`, `Right`
- `Home`, `End`, `PageUp`, `PageDown`
- `F1`-`F12`
- `Ctrl+<char>` (e.g., `Ctrl+C`)

### Resize

```json
{
  "protocol_version": 1,
  "action": {
    "type": "resize",
    "payload": { "rows": 40, "cols": 120 }
  }
}
```

### Wait

```json
{
  "protocol_version": 1,
  "action": {
    "type": "wait",
    "payload": { "duration_ms": 100 }
  }
}
```

### Terminate

```json
{
  "protocol_version": 1,
  "action": {
    "type": "terminate",
    "payload": {}
  }
}
```

## Observations

After each action, receive an observation:

```json
{
  "protocol_version": 1,
  "observation": {
    "screen": {
      "lines": ["line 1", "line 2", ...],
      "rows": 24,
      "cols": 80,
      "cursor": { "row": 0, "col": 5 },
      "cursor_visible": true,
      "alternate_screen": false
    },
    "transcript_delta": "new output...",
    "exit_status": null
  }
}
```

## Screen Snapshot

```json
{
  "lines": ["visible text lines..."],
  "rows": 24,
  "cols": 80,
  "cursor": {
    "row": 0,
    "col": 0
  },
  "cursor_visible": true,
  "alternate_screen": false,
  "cells": [...]  // Optional: detailed cell data with colors
}
```

## Run Result

```json
{
  "protocol_version": 1,
  "run_id": "uuid",
  "passed": true,
  "steps": [
    {
      "step_id": "step-1",
      "passed": true,
      "duration_ms": 42,
      "assertions": [...]
    }
  ],
  "exit_status": 0,
  "runtime_ms": 150,
  "error": null
}
```

## Error Response

```json
{
  "protocol_version": 1,
  "error": {
    "code": "E_POLICY_DENIED",
    "message": "Executable not in allowlist",
    "context": { "executable": "/bin/sh" }
  }
}
```

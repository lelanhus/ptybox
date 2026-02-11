# Protocol Reference

`ptybox driver` uses NDJSON (one JSON object per line) with a request/response envelope.

## Version

Current protocol version: `2`.

Every driver request and response includes `protocol_version`.

## DriverRequestV2

Send one `DriverRequestV2` object per line on stdin.

```json
{
  "protocol_version": 2,
  "request_id": "req-1",
  "action": {
    "type": "text",
    "payload": { "text": "hello world" }
  },
  "timeout_ms": 250
}
```

### Fields

- `protocol_version` (`u32`): must equal `2`
- `request_id` (`string`): caller-defined id echoed in the response
- `action` (`Action`): action to perform
- `timeout_ms` (`u64`, optional): per-action timeout override

## DriverResponseV2

The driver emits one `DriverResponseV2` line per request.

```json
{
  "protocol_version": 2,
  "request_id": "req-1",
  "status": "ok",
  "observation": { "...": "..." },
  "error": null,
  "action_metrics": {
    "sequence": 1,
    "duration_ms": 5
  }
}
```

### Fields

- `protocol_version` (`u32`)
- `request_id` (`string`): copied from request
- `status` (`"ok" | "error"`)
- `observation` (`Observation | null`)
- `error` (`ErrorInfo | null`)
- `action_metrics` (`{ sequence: u64, duration_ms: u64 } | null`)

## Actions

### `text`

```json
{ "type": "text", "payload": { "text": "hello" } }
```

### `key`

```json
{ "type": "key", "payload": { "key": "Enter" } }
```

Supported key forms:

- named keys: `Enter`, `Tab`, `Escape`, `Backspace`, `Delete`
- arrows/navigation: `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PageUp`, `PageDown`
- function keys: `F1`-`F12`
- control chords: `Ctrl+<char>` (for example `Ctrl+C`)
- single-character keys (for example `a`)

### `resize`

```json
{ "type": "resize", "payload": { "rows": 40, "cols": 120 } }
```

### `wait`

```json
{
  "type": "wait",
  "payload": {
    "condition": {
      "type": "screen_contains",
      "payload": { "text": "Ready" }
    }
  }
}
```

Wait condition types:

- `screen_contains` (`payload.text`)
- `screen_matches` (`payload.pattern`, Rust regex)
- `cursor_at` (`payload.row`, `payload.col`)
- `process_exited` (empty payload)

### `terminate`

```json
{ "type": "terminate", "payload": {} }
```

## Observation shape

`observation` in `DriverResponseV2` matches `Observation`:

```json
{
  "protocol_version": 2,
  "run_id": "...",
  "session_id": "...",
  "timestamp_ms": 123,
  "screen": {
    "snapshot_version": 1,
    "snapshot_id": "...",
    "rows": 24,
    "cols": 80,
    "cursor": { "row": 0, "col": 0, "visible": true },
    "alternate_screen": false,
    "lines": ["..."]
  },
  "transcript_delta": "...",
  "events": [
    { "type": "pty_output", "message": "read from pty", "details": { "bytes": 14 } }
  ]
}
```

## Error response example

```json
{
  "protocol_version": 2,
  "request_id": "req-2",
  "status": "error",
  "observation": null,
  "error": {
    "code": "E_PROTOCOL_VERSION_MISMATCH",
    "message": "unsupported protocol version",
    "context": { "provided_version": 1, "supported_version": 2 }
  },
  "action_metrics": null
}
```

## Exit-code mapping

Common stable exits:

- `2` => `E_POLICY_DENIED`
- `4` => `E_TIMEOUT`
- `8` => `E_PROTOCOL_VERSION_MISMATCH`
- `9` => `E_PROTOCOL`
- `10` => `E_IO`

# AI Agent Integration

This guide covers using `ptybox driver` as an agent loop for TUIs.

## Why `driver`

`driver` gives agents a deterministic request/response protocol over NDJSON:

- explicit `request_id` correlation
- structured `status` / `error` for each action
- stable `observation` snapshots
- per-action metrics (`sequence`, `duration_ms`)
- optional artifact capture for replay/debug

## Recommended startup

Use an explicit policy and artifacts directory.

```bash
ptybox driver --stdio --json \
  --policy ./policy.json \
  --artifacts ./artifacts \
  --overwrite \
  -- ./your-tui-app
```

## Request/response contract (v2)

Request (`DriverRequestV2`):

```json
{"protocol_version":2,"request_id":"req-1","action":{"type":"text","payload":{"text":"help"}},"timeout_ms":500}
```

Response (`DriverResponseV2`):

```json
{"protocol_version":2,"request_id":"req-1","status":"ok","observation":{...},"error":null,"action_metrics":{"sequence":1,"duration_ms":7}}
```

On failure:

```json
{"protocol_version":2,"request_id":"req-2","status":"error","observation":null,"error":{"code":"E_TIMEOUT","message":"wait condition timed out","context":{"condition":"screen_contains"}},"action_metrics":{"sequence":2,"duration_ms":500}}
```

## Action set

- `text`: type/paste text (`payload.text`)
- `key`: send key (`payload.key`)
- `resize`: set PTY size (`payload.rows`, `payload.cols`)
- `wait`: wait on condition (`screen_contains`, `screen_matches`, `cursor_at`, `process_exited`)
- `terminate`: end session

## Minimal Python loop

```python
import json
import subprocess

class PtyboxDriver:
    def __init__(self, command, policy_path):
        self.seq = 0
        self.proc = subprocess.Popen(
            [
                "ptybox", "driver", "--stdio", "--json",
                "--policy", policy_path,
                "--", *command,
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )

    def send(self, action, timeout_ms=None):
        self.seq += 1
        request_id = f"req-{self.seq}"
        req = {
            "protocol_version": 2,
            "request_id": request_id,
            "action": action,
        }
        if timeout_ms is not None:
            req["timeout_ms"] = timeout_ms

        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()

        resp = json.loads(self.proc.stdout.readline())
        assert resp["request_id"] == request_id
        return resp

    def close(self):
        self.send({"type": "terminate", "payload": {}})
        self.proc.wait()
```

## Deterministic loop pattern

1. Send one action.
2. Require one response with matching `request_id`.
3. If `status == "error"`, branch on `error.code`.
4. If `status == "ok"`, decide next action from `observation.screen.lines` + `events`.
5. End with `terminate`.

## Artifact/replay flow for agents

When `--artifacts` is enabled, driver sessions include replay inputs:

- `driver-actions.jsonl`
- `scenario.json` (generated from actions)
- `run.json`, `events.jsonl`, `snapshots/`, `transcript.log`, `checksums.json`

This allows deterministic regression checks via:

```bash
ptybox replay --json --artifacts ./artifacts
ptybox replay-report --json --artifacts ./artifacts
```

## Troubleshooting

- `E_POLICY_DENIED`: executable/cwd/filesystem permissions not allowlisted.
- `E_PROTOCOL_VERSION_MISMATCH`: request used unsupported protocol version.
- `E_PROTOCOL`: malformed NDJSON or invalid action payload.
- `E_TIMEOUT`: wait/runtime/output/snapshot/action budget exceeded.

Use `ptybox protocol-help --json` for machine-readable schemas.

# Agent Integration Tutorial

For reference documentation on the driver protocol, see the [AI Agent Integration](ai-agents.md) guide.

This tutorial walks through building an automated agent that drives a TUI application
using the `ptybox driver` NDJSON protocol. You will start with a single request/response
exchange, build a reusable Python wrapper, integrate an LLM for decision-making, and
set up replay-based regression testing.

---

## Part 1: First Agent Session

### Create a policy

Every driver session requires a security policy. Create `policy.json` for driving
`/bin/cat`, a simple program that echoes its input:

```json
{
  "policy_version": 3,
  "sandbox": "none",
  "sandbox_unsafe_ack": true,
  "network": "disabled",
  "network_unsafe_ack": true,
  "fs": {
    "allowed_read": ["/tmp"],
    "allowed_write": ["/tmp"],
    "working_dir": "/tmp"
  },
  "fs_write_unsafe_ack": true,
  "exec": {
    "allowed_executables": ["/bin/cat"],
    "allow_shell": false
  },
  "env": {
    "allowlist": [],
    "set": {},
    "inherit": false
  },
  "budgets": {
    "max_runtime_ms": 30000,
    "max_steps": 100,
    "max_output_bytes": 1048576,
    "max_snapshot_bytes": 2097152,
    "max_wait_ms": 5000
  },
  "artifacts": { "enabled": false },
  "replay": { "strict": false }
}
```

Sandbox is disabled here for simplicity. In production, prefer `"sandbox": "seatbelt"`
on macOS for process isolation.

### Start the driver

Launch a driver session. The `--stdio` flag uses stdin/stdout for NDJSON communication:

```bash
ptybox driver --stdio --json --policy ./policy.json -- /bin/cat
```

The driver is now waiting for requests on stdin.

### Send a text action

Write this JSON (followed by a newline) to the driver's stdin:

```json
{"protocol_version":2,"request_id":"req-1","action":{"type":"text","payload":{"text":"hello\n"}},"timeout_ms":500}
```

The driver responds with a single NDJSON line:

```json
{
  "protocol_version": 2,
  "request_id": "req-1",
  "status": "ok",
  "observation": {
    "protocol_version": 2,
    "run_id": "...",
    "session_id": "...",
    "timestamp_ms": 42,
    "screen": {
      "snapshot_version": 1,
      "snapshot_id": "...",
      "rows": 24,
      "cols": 80,
      "cursor": { "row": 1, "col": 0, "visible": true },
      "alternate_screen": false,
      "lines": ["hello", "", "", "..."],
      "cells": null
    },
    "transcript_delta": "hello\r\n",
    "events": []
  },
  "error": null,
  "action_metrics": { "sequence": 1, "duration_ms": 7 }
}
```

Key fields in the response:

- **`screen.lines`** -- Array of strings, one per terminal row. Trailing spaces are trimmed. This is the canonical view of terminal content.
- **`screen.cursor`** -- Row and column (0-based) plus visibility. Row 0 is the top of the screen.
- **`transcript_delta`** -- Raw bytes written to the terminal since the last observation. Contains ANSI escape sequences. Useful for logging, not for assertions.
- **`events`** -- Notable occurrences such as title changes. Usually empty.
- **`action_metrics.sequence`** -- Monotonic counter. The first action is sequence 1.

### Terminate the session

Send a terminate action to cleanly end the session:

```json
{"protocol_version":2,"request_id":"req-2","action":{"type":"terminate","payload":{}},"timeout_ms":500}
```

The driver sends a final response and exits.

---

## Part 2: Agent Loop Pattern

The driver protocol follows a strict send-observe-decide loop:

1. Send one action as an NDJSON line to stdin.
2. Read one response from stdout with the matching `request_id`.
3. If `status` is `"error"`, handle the error based on `error.code`.
4. If `status` is `"ok"`, inspect `observation.screen.lines` and decide the next action.
5. Repeat until done, then send `terminate`.

### Python agent class

Here is a complete, reusable wrapper around the driver subprocess:

```python
import json
import subprocess
import sys
from typing import Any


class PtyboxAgent:
    """Drives a TUI application through the ptybox driver protocol v2."""

    def __init__(self, command: list[str], policy_path: str):
        self._seq = 0
        self._proc = subprocess.Popen(
            [
                "ptybox", "driver", "--stdio", "--json",
                "--policy", policy_path,
                "--", *command,
            ],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )

    def send_action(
        self,
        action: dict[str, Any],
        timeout_ms: int = 500,
    ) -> dict[str, Any]:
        """Send an action and return the validated response."""
        self._seq += 1
        request_id = f"req-{self._seq}"

        request = {
            "protocol_version": 2,
            "request_id": request_id,
            "action": action,
            "timeout_ms": timeout_ms,
        }

        assert self._proc.stdin is not None
        assert self._proc.stdout is not None

        self._proc.stdin.write(json.dumps(request) + "\n")
        self._proc.stdin.flush()

        line = self._proc.stdout.readline()
        if not line:
            raise RuntimeError("Driver process closed stdout unexpectedly")

        response = json.loads(line)

        # Validate request_id correlation
        if response["request_id"] != request_id:
            raise RuntimeError(
                f"Request ID mismatch: sent {request_id}, "
                f"got {response['request_id']}"
            )

        # Check for errors
        if response["status"] == "error":
            err = response["error"]
            raise DriverError(err["code"], err["message"], err.get("context"))

        return response

    def send_text(self, text: str, timeout_ms: int = 500) -> dict[str, Any]:
        """Type text into the terminal."""
        return self.send_action(
            {"type": "text", "payload": {"text": text}},
            timeout_ms=timeout_ms,
        )

    def send_key(self, key: str, timeout_ms: int = 500) -> dict[str, Any]:
        """Press a key (e.g., 'Enter', 'Tab', 'Escape', 'a')."""
        return self.send_action(
            {"type": "key", "payload": {"key": key}},
            timeout_ms=timeout_ms,
        )

    def wait_for(
        self,
        text: str,
        timeout_ms: int = 5000,
    ) -> dict[str, Any]:
        """Wait until the screen contains the given text."""
        return self.send_action(
            {
                "type": "wait",
                "payload": {
                    "condition": {
                        "type": "screen_contains",
                        "payload": {"text": text},
                    }
                },
            },
            timeout_ms=timeout_ms,
        )

    def get_screen_text(self, response: dict[str, Any]) -> str:
        """Extract the screen as a single string from a response.

        Joins screen.lines with newlines and strips trailing whitespace
        from each line. This gives a clean, readable view of terminal
        content suitable for assertions or LLM prompts.
        """
        lines = response["observation"]["screen"]["lines"]
        return "\n".join(line.rstrip() for line in lines)

    def close(self) -> int:
        """Terminate the session and return the driver exit code."""
        try:
            self.send_action(
                {"type": "terminate", "payload": {}},
                timeout_ms=1000,
            )
        except (DriverError, RuntimeError):
            pass  # Session may already be closed
        return self._proc.wait(timeout=5)


class DriverError(Exception):
    """Error returned by the ptybox driver."""

    def __init__(self, code: str, message: str, context: Any = None):
        self.code = code
        self.message = message
        self.context = context
        super().__init__(f"{code}: {message}")
```

### Wait conditions

The `wait` action supports four condition types:

| Condition | Payload | Use case |
|-----------|---------|----------|
| `screen_contains` | `{"text": "Ready"}` | Wait for specific text to appear on screen |
| `screen_matches` | `{"pattern": "v\\d+\\.\\d+"}` | Wait for text matching a regex pattern |
| `cursor_at` | `{"row": 0, "col": 5}` | Wait for cursor to reach a specific position |
| `process_exited` | `{}` | Wait for the process to terminate on its own |

`screen_contains` is the most common choice. It checks whether the given text appears
anywhere in the joined screen lines. Use `screen_matches` when you need pattern
flexibility (versions, counters, dynamic labels).

Example -- wait for a regex match:

```python
response = agent.send_action(
    {
        "type": "wait",
        "payload": {
            "condition": {
                "type": "screen_matches",
                "payload": {"pattern": "Connected to .+:\\d+"},
            }
        },
    },
    timeout_ms=10000,
)
```

### Error recovery

When `status` is `"error"`, the response includes an `error` object with `code`,
`message`, and optional `context`. Map error codes to recovery strategies:

```python
def handle_with_retry(agent: PtyboxAgent, action: dict, retries: int = 2):
    """Send an action with error-aware retry logic."""
    for attempt in range(retries + 1):
        try:
            return agent.send_action(action)
        except DriverError as e:
            if e.code == "E_TIMEOUT" and attempt < retries:
                # Increase timeout or simplify the wait condition
                action["timeout_ms"] = action.get("timeout_ms", 500) * 2
                continue
            elif e.code == "E_PROCESS_EXIT":
                # Process exited -- cannot recover, inspect exit status
                print(f"Process exited: {e.context}", file=sys.stderr)
                raise
            elif e.code == "E_PROTOCOL":
                # Fix the request format -- do not retry
                raise
            else:
                raise
```

---

## Part 3: LLM Integration

An LLM can decide what actions to send based on the current screen. The pattern is:

1. Get the latest screen state from a driver response.
2. Format it into a prompt for the LLM.
3. Parse the LLM output into a driver action.
4. Send the action through the agent.

### Formatting screen state for the LLM

```python
def format_screen_for_llm(response: dict[str, Any]) -> str:
    """Build a text representation of terminal state for an LLM prompt."""
    screen = response["observation"]["screen"]
    lines = screen["lines"]
    cursor = screen["cursor"]

    # Trim trailing empty lines to reduce token usage
    while lines and not lines[-1].strip():
        lines = lines[:-1]

    screen_text = "\n".join(lines)
    return (
        f"Current terminal screen ({screen['rows']}x{screen['cols']}):\n"
        f"```\n{screen_text}\n```\n"
        f"Cursor at row {cursor['row']}, col {cursor['col']}."
    )
```

### System prompt template

```python
SYSTEM_PROMPT = """\
You are controlling a TUI application via a terminal driver.

{screen_state}

Available actions (respond with exactly one JSON object):
- Type text: {{"action": "text", "text": "your text here"}}
- Press key: {{"action": "key", "key": "Enter"}}
- Wait for text: {{"action": "wait", "text": "expected text"}}
- Done: {{"action": "done"}}

Decide the next action based on the current screen. Respond with only
the JSON object, no explanation.
"""
```

### Parsing LLM output into driver actions

```python
import json


def llm_output_to_action(llm_json: str) -> dict[str, Any] | None:
    """Convert an LLM decision into a ptybox driver action.

    Returns None if the LLM chose "done".
    Raises ValueError if the output is invalid.
    """
    try:
        decision = json.loads(llm_json.strip())
    except json.JSONDecodeError as e:
        raise ValueError(f"LLM output is not valid JSON: {e}") from e

    action_type = decision.get("action")

    if action_type == "text":
        text = decision.get("text")
        if not isinstance(text, str):
            raise ValueError("text action requires a 'text' string field")
        return {"type": "text", "payload": {"text": text}}

    elif action_type == "key":
        key = decision.get("key")
        if not isinstance(key, str):
            raise ValueError("key action requires a 'key' string field")
        return {"type": "key", "payload": {"key": key}}

    elif action_type == "wait":
        text = decision.get("text")
        if not isinstance(text, str):
            raise ValueError("wait action requires a 'text' string field")
        return {
            "type": "wait",
            "payload": {
                "condition": {
                    "type": "screen_contains",
                    "payload": {"text": text},
                }
            },
        }

    elif action_type == "done":
        return None

    else:
        raise ValueError(f"Unknown action type from LLM: {action_type}")
```

### Example: LLM-driven menu navigation

```python
def llm_agent_loop(
    agent: PtyboxAgent,
    call_llm,  # function(prompt: str) -> str
    max_steps: int = 20,
):
    """Run an LLM-driven agent loop against a TUI application.

    call_llm is a callable that takes a prompt string and returns the
    LLM response string (the JSON action decision).
    """
    # Get initial screen state
    response = agent.send_action(
        {"type": "wait", "payload": {"condition": {"type": "screen_contains", "payload": {"text": ""}}}},
        timeout_ms=2000,
    )

    for step in range(max_steps):
        screen_state = format_screen_for_llm(response)
        prompt = SYSTEM_PROMPT.format(screen_state=screen_state)

        llm_output = call_llm(prompt)
        action = llm_output_to_action(llm_output)

        if action is None:
            break  # LLM decided it is done

        response = agent.send_action(action, timeout_ms=5000)

    agent.close()
```

### Safety: validating LLM output

Always validate LLM-generated actions before sending them to the driver. The
`llm_output_to_action` function above already enforces that:

- The action type is one of the allowed values (`text`, `key`, `wait`, `done`).
- Required payload fields are present and have the correct types.
- Unknown action types are rejected with a clear error.

Never pass raw LLM output directly into the driver request. The validation layer
is your defense against malformed or unexpected LLM responses.

---

## Part 4: Replay for Regression

Once your agent works, capture artifacts to create a deterministic baseline for
regression testing.

### Recording a session

Pass `--artifacts` when starting the driver to capture everything:

```bash
ptybox driver --stdio --json \
  --policy ./policy.json \
  --artifacts ./artifacts \
  --overwrite \
  -- /bin/cat
```

The artifacts directory will contain:

- `driver-actions.jsonl` -- Every action sent during the session
- `scenario.json` -- Generated scenario from the recorded actions
- `run.json` -- Run result with status, timing, and exit code
- `events.jsonl` -- Event stream
- `snapshots/` -- Screen snapshots after each action
- `transcript.log` -- Full terminal transcript
- `checksums.json` -- Integrity checksums for all artifacts

### Replaying against the baseline

Compare a new run against the recorded baseline:

```bash
ptybox replay --json --artifacts ./artifacts
```

If the output has changed, the replay exits with code 11 (`E_REPLAY_MISMATCH`)
and reports differences in snapshots, transcript, or events.

Generate an HTML diff report for visual inspection:

```bash
ptybox replay-report --json --artifacts ./artifacts --output ./diff-report.html
```

### Normalization for non-deterministic output

Real applications include timestamps, PIDs, and other values that change between
runs. Use normalization to handle these.

**Default filters** strip common non-deterministic fields (IDs, timestamps):

```bash
ptybox replay --json --artifacts ./artifacts --normalize all
```

**Custom regex rules** in the policy handle application-specific patterns. Add
them to the `replay` section of your policy:

```json
{
  "replay": {
    "strict": false,
    "normalization_filters": ["snapshot_id", "run_id", "run_timestamps"],
    "normalization_rules": [
      {
        "target": "transcript",
        "pattern": "PID \\d+",
        "replace": "PID XXX"
      },
      {
        "target": "snapshot_lines",
        "pattern": "\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}",
        "replace": "YYYY-MM-DDTHH:MM:SS"
      }
    ]
  }
}
```

Available normalization filter names: `snapshot_id`, `run_id`, `run_timestamps`,
`step_timestamps`, `observation_timestamp`, `session_id`, `events`.

Rule targets are `transcript` (raw output) and `snapshot_lines` (screen content).

---

## Part 5: Common Pitfalls

### Timing and stale state

Do not use `time.sleep()` to wait for terminal output. The driver protocol is
request-response: every action returns the latest screen state. Use `wait`
actions with conditions instead:

```python
# Wrong -- introduces flaky timing
import time
agent.send_text("start-server\n")
time.sleep(2)
response = agent.send_text("")  # hope the server is ready

# Right -- deterministic wait
agent.send_text("start-server\n")
response = agent.wait_for("Listening on port", timeout_ms=10000)
```

### ANSI escapes in transcript vs clean screen lines

`transcript_delta` contains the raw byte stream written to the terminal, including
ANSI escape codes for colors, cursor movement, and screen clearing. It looks like
`\x1b[32mOK\x1b[0m` rather than `OK`.

`screen.lines` is the parsed, clean text on each row after the terminal emulator
has processed all escape sequences. Always use `screen.lines` for assertions and
LLM prompts. Reserve `transcript_delta` for debug logging.

### Non-determinism sources

Common sources of non-deterministic output that break replay:

- **PIDs** in process listings or log messages
- **Timestamps** in log output or status bars
- **Async startup ordering** when multiple components print to the terminal concurrently

Handle all of these with normalization rules in your policy (see Part 4).

### Unicode and wide characters

CJK characters and some emoji occupy 2 terminal columns but appear as a single
character in `screen.lines`. The `lines` array reflects logical characters, not
column positions. If you need column-accurate positioning, use the `cells` array
(when enabled) which includes a `width` field per cell.

### Process already exited

If the TUI process exits before you send an action, the driver returns
`E_PROCESS_EXIT`. This is not necessarily an error -- the application may have
finished its work. Use the `process_exited` wait condition when you expect the
process to exit:

```python
# Wait for the process to finish on its own
response = agent.send_action(
    {
        "type": "wait",
        "payload": {
            "condition": {"type": "process_exited", "payload": {}},
        },
    },
    timeout_ms=10000,
)
```

---

## Complete Example

This script drives `/bin/cat`: sends text, waits for the echo, verifies the
screen content, and terminates cleanly.

```python
#!/usr/bin/env python3
"""Minimal ptybox agent that drives /bin/cat."""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

# Write policy to a temporary file
POLICY = {
    "policy_version": 3,
    "sandbox": "none",
    "sandbox_unsafe_ack": True,
    "network": "disabled",
    "network_unsafe_ack": True,
    "fs": {"allowed_read": ["/tmp"], "allowed_write": ["/tmp"], "working_dir": "/tmp"},
    "fs_write_unsafe_ack": True,
    "exec": {"allowed_executables": ["/bin/cat"], "allow_shell": False},
    "env": {"allowlist": [], "set": {}, "inherit": False},
    "budgets": {
        "max_runtime_ms": 10000,
        "max_steps": 50,
        "max_output_bytes": 1048576,
        "max_snapshot_bytes": 2097152,
        "max_wait_ms": 5000,
    },
    "artifacts": {"enabled": False},
    "replay": {"strict": False},
}

policy_path = Path(tempfile.mktemp(suffix=".json"))
policy_path.write_text(json.dumps(POLICY))

try:
    # Start the driver
    proc = subprocess.Popen(
        [
            "ptybox", "driver", "--stdio", "--json",
            "--policy", str(policy_path),
            "--", "/bin/cat",
        ],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )
    assert proc.stdin is not None
    assert proc.stdout is not None

    seq = 0

    def send(action, timeout_ms=500):
        global seq
        seq += 1
        req = {
            "protocol_version": 2,
            "request_id": f"req-{seq}",
            "action": action,
            "timeout_ms": timeout_ms,
        }
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
        resp = json.loads(proc.stdout.readline())
        if resp["status"] == "error":
            print(f"Error: {resp['error']}", file=sys.stderr)
            sys.exit(1)
        return resp

    # 1. Send text and wait for echo
    send({"type": "text", "payload": {"text": "hello world\n"}})

    resp = send(
        {
            "type": "wait",
            "payload": {
                "condition": {
                    "type": "screen_contains",
                    "payload": {"text": "hello world"},
                }
            },
        },
        timeout_ms=2000,
    )

    # 2. Verify screen content
    lines = resp["observation"]["screen"]["lines"]
    screen = "\n".join(line.rstrip() for line in lines if line.strip())
    assert "hello world" in screen, f"Expected 'hello world' on screen, got:\n{screen}"
    print("Screen verified: 'hello world' is present.")

    # 3. Terminate
    send({"type": "terminate", "payload": {}})
    exit_code = proc.wait(timeout=5)
    print(f"Driver exited with code {exit_code}.")

finally:
    policy_path.unlink(missing_ok=True)
```

Save this as `agent_demo.py` and run it:

```bash
python3 agent_demo.py
```

Expected output:

```
Screen verified: 'hello world' is present.
Driver exited with code 0.
```

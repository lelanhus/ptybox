# AI Agent Integration

This guide covers integrating ptybox with AI agents (LLMs) for automated TUI testing and interaction.

## Overview

ptybox provides a stable NDJSON protocol that AI agents can use to:

- Drive terminal applications step by step
- Read screen content and state
- Wait for specific conditions
- Make decisions based on output

## Driver Mode

The `driver` command provides interactive control via NDJSON over stdin/stdout:

```bash
ptybox driver --stdio --json -- ./your-tui-app
```

The agent sends actions as JSON lines:

```json
{"protocol_version": 1, "action": {"type": "text", "payload": {"text": "hello"}}}
```

And receives observations:

```json
{
  "protocol_version": 1,
  "run_id": "abc123",
  "timestamp_ms": 150,
  "screen": {
    "rows": 24,
    "cols": 80,
    "cursor_row": 5,
    "cursor_col": 12,
    "lines": ["Welcome to the app", "Enter your name: _", ...]
  },
  "transcript_delta": "Enter your name: "
}
```

## Action Types

### Text Input

Send text to the application:

```json
{
  "protocol_version": 1,
  "action": {
    "type": "text",
    "payload": {"text": "user@example.com"}
  }
}
```

### Key Press

Send special keys:

```json
{
  "protocol_version": 1,
  "action": {
    "type": "key",
    "payload": {"key": "Enter"}
  }
}
```

Available keys: `Enter`, `Tab`, `Escape`, `Up`, `Down`, `Left`, `Right`, `Backspace`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`, `F1`-`F12`.

### Resize Terminal

Change terminal dimensions:

```json
{
  "protocol_version": 1,
  "action": {
    "type": "resize",
    "payload": {"rows": 30, "cols": 100}
  }
}
```

### Wait for Condition

Wait for text to appear on screen:

```json
{
  "protocol_version": 1,
  "action": {
    "type": "wait",
    "payload": {
      "condition": {
        "type": "screen_contains",
        "payload": {"text": "Success!"}
      },
      "timeout_ms": 5000
    }
  }
}
```

Wait condition types:
- `screen_contains` - Text appears anywhere on screen
- `screen_matches_regex` - Regex matches screen content
- `cursor_at` - Cursor at specific row/column
- `delay` - Wait fixed milliseconds
- `process_exited` - Process has terminated

### Terminate

End the session:

```json
{
  "protocol_version": 1,
  "action": {
    "type": "terminate",
    "payload": {}
  }
}
```

## Integration Patterns

### Python Subprocess Example

```python
import subprocess
import json

def run_tui_session(commands):
    proc = subprocess.Popen(
        ["ptybox", "driver", "--stdio", "--json", "--", "./app"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True
    )

    results = []
    for action in commands:
        request = {"protocol_version": 1, "action": action}
        proc.stdin.write(json.dumps(request) + "\n")
        proc.stdin.flush()

        response = proc.stdout.readline()
        observation = json.loads(response)
        results.append(observation)

        if action["type"] == "terminate":
            break

    proc.wait()
    return results
```

### LLM Tool Definition (OpenAI Format)

```json
{
  "name": "tui_action",
  "description": "Send an action to the terminal application",
  "parameters": {
    "type": "object",
    "properties": {
      "action_type": {
        "type": "string",
        "enum": ["text", "key", "wait", "terminate"],
        "description": "Type of action to perform"
      },
      "text": {
        "type": "string",
        "description": "Text to type (for 'text' action)"
      },
      "key": {
        "type": "string",
        "description": "Key to press (for 'key' action)"
      },
      "wait_for": {
        "type": "string",
        "description": "Text to wait for (for 'wait' action)"
      }
    },
    "required": ["action_type"]
  }
}
```

### Claude Tool Definition (MCP Format)

```json
{
  "name": "ptybox_action",
  "description": "Interact with a terminal application",
  "input_schema": {
    "type": "object",
    "properties": {
      "type": {
        "type": "string",
        "enum": ["text", "key", "resize", "wait", "terminate"]
      },
      "payload": {
        "type": "object"
      }
    },
    "required": ["type", "payload"]
  }
}
```

## Reading Screen State

The observation includes the full screen state:

```python
def analyze_screen(observation):
    screen = observation["screen"]
    lines = screen["lines"]
    cursor = (screen["cursor_row"], screen["cursor_col"])

    # Find specific content
    for i, line in enumerate(lines):
        if "Error" in line:
            return {"status": "error", "line": i, "content": line}

    # Check cursor position
    if cursor == (5, 0):
        return {"status": "at_prompt"}

    return {"status": "unknown"}
```

## Agent Loop Pattern

```python
class TUIAgent:
    def __init__(self, command):
        self.proc = subprocess.Popen(
            ["ptybox", "driver", "--stdio", "--json", "--"] + command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True
        )
        self.history = []

    def send(self, action):
        request = {"protocol_version": 1, "action": action}
        self.proc.stdin.write(json.dumps(request) + "\n")
        self.proc.stdin.flush()

        response = self.proc.stdout.readline()
        observation = json.loads(response)
        self.history.append({"action": action, "observation": observation})
        return observation

    def get_screen_text(self):
        """Get current screen as readable text."""
        if not self.history:
            return ""
        return "\n".join(self.history[-1]["observation"]["screen"]["lines"])

    def wait_for_text(self, text, timeout_ms=5000):
        """Wait for text to appear on screen."""
        return self.send({
            "type": "wait",
            "payload": {
                "condition": {"type": "screen_contains", "payload": {"text": text}},
                "timeout_ms": timeout_ms
            }
        })

    def type_text(self, text):
        return self.send({"type": "text", "payload": {"text": text}})

    def press_key(self, key):
        return self.send({"type": "key", "payload": {"key": key}})

    def terminate(self):
        self.send({"type": "terminate", "payload": {}})
        self.proc.wait()
```

## Best Practices

### 1. Always Wait for Ready State

Before interacting, wait for the application to be ready:

```python
agent = TUIAgent(["./app"])
agent.wait_for_text("Enter command:")  # Wait for prompt
agent.type_text("help")
agent.press_key("Enter")
```

### 2. Use Timeouts

Avoid hanging on wait conditions:

```python
try:
    agent.wait_for_text("Success", timeout_ms=10000)
except TimeoutError:
    print("Operation timed out, current screen:", agent.get_screen_text())
```

### 3. Handle Errors Gracefully

Check for error conditions on screen:

```python
obs = agent.type_text("invalid-command")
screen = "\n".join(obs["screen"]["lines"])

if "error" in screen.lower() or "invalid" in screen.lower():
    # Take corrective action
    agent.press_key("Escape")
```

### 4. Clean Termination

Always terminate the session:

```python
try:
    # ... interaction logic ...
finally:
    agent.terminate()
```

### 5. State Verification

Verify state after important actions:

```python
agent.type_text("save")
agent.press_key("Enter")

obs = agent.wait_for_text("Saved", timeout_ms=2000)
if "Saved" not in "\n".join(obs["screen"]["lines"]):
    raise Exception("Save failed")
```

## Debugging Agent Interactions

### Enable Verbose Logging

```bash
ptybox driver --stdio --json --verbose -- ./app 2>debug.log
```

### Save Session Transcript

Record all interactions for replay:

```python
with open("session.jsonl", "w") as f:
    for entry in agent.history:
        f.write(json.dumps(entry) + "\n")
```

### Visual Trace

Generate an HTML trace from recorded artifacts:

```bash
ptybox trace artifacts/ --output trace.html
```

## Protocol Reference

See [Protocol Reference](../reference/protocol.md) for complete protocol documentation.

Run `ptybox protocol-help --json` for machine-readable schema.

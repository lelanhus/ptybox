# API Documentation

The ptybox library provides a Rust API for programmatic control.

## Rustdoc

Full API documentation is available at:

**[API Reference (rustdoc)](/ptybox/api/ptybox/)**

## Quick Reference

### Running Commands

```rust
use ptybox::run::{run_exec, run_scenario};
use ptybox::model::{Policy, Scenario};

// Run a single command
let result = run_exec(
    "/bin/echo".to_string(),
    vec!["hello".to_string()],
    Some("/tmp".into()),
    policy,
)?;

// Run a scenario
let result = run_scenario(scenario)?;
```

### Session Control

```rust
use ptybox::session::{Session, SessionConfig};
use ptybox::model::{Action, ActionType};
use std::time::Duration;

// Spawn a session
let config = SessionConfig { /* ... */ };
let mut session = Session::spawn(config)?;

// Send input
let action = Action {
    action_type: ActionType::Text,
    payload: serde_json::json!({"text": "hello"}),
};
session.send(&action)?;

// Observe output
let observation = session.observe(Duration::from_secs(1))?;
println!("Screen: {:?}", observation.screen.lines);

// Terminate
session.terminate()?;
```

### Policy Building

```rust
use ptybox::model::PolicyBuilder;

let policy = PolicyBuilder::new()
    .sandbox_none()
    .network_disabled()
    .allowed_executable("/bin/echo")
    .allowed_read("/tmp")
    .build();
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| `ptybox` | Core library |
| `ptybox-cli` | CLI binary |
| `ptybox-fixtures` | Test helpers (internal) |

## Key Types

- `Policy` - Security policy configuration
- `Scenario` - Test scenario definition
- `Session` - PTY session handle
- `RunResult` - Execution result
- `Observation` - Terminal state snapshot
- `ScreenSnapshot` - Screen content with cursor

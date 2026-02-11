# API Reference

## Rustdoc

Build local API docs:

```bash
cargo doc --workspace --no-deps
```

Then open `target/doc/ptybox/index.html`.

## Primary Rust APIs

### Run a single command

```rust
use ptybox::model::Policy;
use ptybox::run::run_exec;

let policy = Policy::default();
let result = run_exec(
    "/bin/echo".to_string(),
    vec!["hello".to_string()],
    None,
    policy,
)?;
```

### Run a scenario

```rust
use ptybox::model::Scenario;
use ptybox::run::run_scenario;

let scenario: Scenario = serde_json::from_str(&std::fs::read_to_string("scenario.json")?)?;
let result = run_scenario(scenario)?;
```

### Run driver loop programmatically

```rust
use ptybox::driver::{run_driver, DriverConfig};
use ptybox::model::policy::PolicyBuilder;

let cfg = DriverConfig {
    command: "/bin/cat".to_string(),
    args: Vec::new(),
    cwd: None,
    policy: PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_read(vec!["/tmp".to_string()])
        .allowed_executables(vec!["/bin/cat".to_string()])
        .build(),
    artifacts: None,
};
run_driver(cfg)?;
```

## Common model types

- `Policy`, `PolicyBuilder`
- `Scenario`, `RunConfig`, `Step`, `Action`
- `Observation`, `ScreenSnapshot`, `Event`
- `RunResult`, `ErrorInfo`
- `DriverRequestV2`, `DriverResponseV2`

## Crates

| Crate | Purpose |
|---|---|
| `ptybox` | Core library |
| `ptybox-cli` | CLI frontend |
| `ptybox-fixtures` | Test fixture binaries |

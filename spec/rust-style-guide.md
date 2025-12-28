# Rust Style Guide for ptybox

This document defines the coding standards enforced across the ptybox workspace.
All rules are enforced via workspace-level lints in `Cargo.toml` and build-time
flags in `.cargo/config.toml`.

## Quick Reference

| Category | Rule | Enforcement |
|----------|------|-------------|
| Unsafe code | Forbidden in library, allowed in fixtures | `deny` (not `forbid`) |
| Panics | Use `Result`, not `unwrap()`/`expect()` | `deny` |
| Error handling | Propagate with `?`, typed errors via `thiserror` | Style |
| Documentation | Required for public items (currently `warn`) | `warn` |
| Warnings | Treated as errors | `-D warnings` in rustflags |

## Lint Configuration

### Workspace Lints (`Cargo.toml`)

```toml
[workspace.lints.rust]
unsafe_code = "deny"      # Allows override for fixtures crate
missing_docs = "warn"     # Upgrade to deny once docs complete

[workspace.lints.clippy]
# Core groups (all deny with priority -1)
all = { level = "deny", priority = -1 }
correctness = { level = "deny", priority = -1 }
suspicious = { level = "deny", priority = -1 }
complexity = { level = "deny", priority = -1 }
perf = { level = "deny", priority = -1 }
style = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
cargo = { level = "deny", priority = -1 }

# Security-critical (from restriction group)
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
exit = "deny"
dbg_macro = "deny"
unreachable = "deny"
```

### Allowed Exceptions

| Lint | Reason |
|------|--------|
| `module_name_repetitions` | Common in public APIs |
| `similar_names` | Too aggressive for domain naming |
| `too_many_lines` | Prefer cognitive_complexity |
| `struct_excessive_bools` | Policy structs have many flags |
| `single_match_else` | Match often clearer than if-let |
| `needless_pass_by_value` | Clearer APIs, Copy types cheap |
| `redundant_else` | Else block aids readability |
| `must_use_candidate` | Add manually when needed |

---

## Naming Conventions

### Case Rules

| Element | Case | Example |
|---------|------|---------|
| Modules | `snake_case` | `policy_validator` |
| Functions | `snake_case` | `validate_path` |
| Variables | `snake_case` | `user_input` |
| Types/Structs | `CamelCase` | `PolicyError` |
| Traits | `CamelCase` | `Validator` |
| Enums | `CamelCase` | `SandboxMode` |
| Enum Variants | `CamelCase` | `SandboxMode::None` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_BUFFER_SIZE` |
| Statics | `SCREAMING_SNAKE_CASE` | `PROTOCOL_VERSION` |

### Method Naming Patterns

| Prefix | Meaning | Ownership | Example |
|--------|---------|-----------|---------|
| `as_*` | Cheap reference conversion | Borrows | `as_str()` |
| `to_*` | Expensive conversion | Borrows, allocates | `to_string()` |
| `into_*` | Consuming conversion | Takes ownership | `into_bytes()` |
| `is_*` | Boolean predicate | Borrows | `is_empty()` |
| `has_*` | Possession check | Borrows | `has_permission()` |
| `try_*` | Fallible operation | Varies | `try_from()` |

### Getter Pattern

**DO:** Use bare noun (no `get_` prefix)
```rust
// Good
fn name(&self) -> &str { &self.name }
fn policy(&self) -> &Policy { &self.policy }

// Bad
fn get_name(&self) -> &str { &self.name }
```

---

## Error Handling

### Rules

1. **NEVER** use `.unwrap()` or `.expect()` in production code
2. **ALWAYS** use `Result<T, E>` for fallible operations
3. **USE** `?` operator for error propagation
4. **USE** `thiserror` for typed library errors
5. **INCLUDE** stable error codes (e.g., `E_POLICY_DENIED`)

### Error Type Pattern

```rust
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("[{code}] {message}")]
    PolicyDenied {
        code: String,
        message: String,
        context: Option<serde_json::Value>,
    },

    #[error("[{code}] {message}: {source}")]
    Io {
        code: String,
        message: String,
        #[source]
        source: std::io::Error,
    },
}
```

### Stable Error Codes

| Code | Meaning |
|------|---------|
| `E_POLICY_DENIED` | Policy validation failed |
| `E_SANDBOX_UNAVAILABLE` | Sandbox not available |
| `E_TIMEOUT` | Budget/timeout exceeded |
| `E_ASSERTION_FAILED` | Assertion did not pass |
| `E_PROCESS_FAILED` | Child process exited non-zero |
| `E_TERMINAL_PARSE` | Terminal parsing failed |
| `E_PROTOCOL` | Protocol error |
| `E_IO` | I/O operation failed |
| `E_REPLAY_MISMATCH` | Replay comparison failed |

### Tests Exception

Tests MAY use `.unwrap()` and `.expect()` for clarity:
```rust
// Test file header
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
```

---

## Function Signatures

### Parameter Guidelines

| Situation | Use | Why |
|-----------|-----|-----|
| Read-only string | `&str` | Avoid allocation |
| Accept String or &str | `impl AsRef<str>` | Flexibility |
| Store ownership | `impl Into<String>` | Let caller decide |
| Accept iterator | `impl Iterator<Item=T>` | Avoid forcing Vec |

### Return Guidelines

| Situation | Use | Why |
|-----------|-----|-----|
| Lazy iteration | `impl Iterator<Item=T>` | Avoid allocation |
| Conditional ownership | `Cow<'_, str>` | Efficiency |
| Fallible operation | `Result<T, E>` | Explicit errors |
| Optional value | `Option<T>` | Explicit absence |

### Example

```rust
// Good: accepts &str or String, returns Result
pub fn validate_path(path: impl AsRef<str>) -> Result<(), PolicyError> {
    let path = path.as_ref();
    // ...
}

// Good: returns iterator, not Vec
pub fn allowed_paths(&self) -> impl Iterator<Item = &str> {
    self.paths.iter().map(String::as_str)
}
```

---

## Integer Safety

### Enforced Rules

| Lint | Level | Purpose |
|------|-------|---------|
| `cast_possible_truncation` | `deny` | Prevent silent data loss |
| `cast_sign_loss` | `deny` | Prevent unsigned/signed issues |
| `cast_possible_wrap` | `deny` | Prevent integer overflow |
| `indexing_slicing` | `deny` | Prevent index panics |

### When Casts Are Safe

Add inline allowance with comment explaining why:

```rust
// Terminal coordinates are always small, safe to truncate
#[allow(clippy::cast_possible_truncation)]
let row = value as u16;

// Exit codes are typically 0-255, safe to cast
#[allow(clippy::cast_possible_wrap)]
let code = status.exit_code() as i32;
```

### Prefer Checked Arithmetic

```rust
// Good: explicit overflow handling
let size = base.checked_add(extra)
    .ok_or(Error::Overflow)?;

// Bad: implicit panic on overflow
let size = base + extra;
```

---

## Memory Safety

### Unsafe Code Policy

- `#![forbid(unsafe_code)]` in library crate
- Fixtures crate allows unsafe for terminal ioctl (with `#![allow(unsafe_code)]`)
- All unsafe code requires safety comment

### Indexing Policy

Use `.get()` instead of `[]` to prevent panics:

```rust
// Good: returns Option
let byte = buffer.get(index)?;

// Bad: may panic
let byte = buffer[index];
```

### Path Validation

All filesystem paths must be:
1. Canonicalized before use
2. Validated against allowlists
3. Rejected if they escape sandbox

---

## Import Organization

### Grouping Order

1. Standard library (`std::*`)
2. External crates (alphabetical)
3. Current crate (`crate::*`)

### Example

```rust
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::Policy;
use crate::runner::RunnerError;
```

---

## Documentation

### Requirements

- All public items MUST have `///` doc comments
- Include `# Examples` section with runnable code
- Document `# Errors` for fallible functions
- Document `# Panics` (should be rare)
- Use `[`BacktickLinks`]` for cross-references

### Example

```rust
/// Validates a path against the policy allowlist.
///
/// # Errors
///
/// Returns `PolicyError::PathDenied` if the path is not in the allowlist.
///
/// # Examples
///
/// ```
/// use ptybox::policy::validate_path;
///
/// let result = validate_path("/usr/bin/ls", &policy);
/// assert!(result.is_ok());
/// ```
pub fn validate_path(path: &str, policy: &Policy) -> Result<(), PolicyError> {
    // ...
}
```

---

## Testing

### Test Naming

Use pattern: `<what>_<condition>_<expected>`

```rust
#[test]
fn validate_path_with_allowed_path_succeeds() { }

#[test]
fn validate_path_with_denied_path_returns_error() { }
```

### Test File Header

All test files should have relaxed linting:

```rust
// Test module - relaxed lint rules
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(missing_docs)]
#![allow(clippy::panic)]
```

### Integration Tests

Prefer integration tests in `tests/` directory over unit tests
for testing public API behavior.

---

## CLI-Specific Rules

The CLI crate (`ptybox-cli`) has specific allowances:

```rust
#![allow(clippy::print_stdout)]  // CLI must print
#![allow(clippy::print_stderr)]  // CLI must print errors
#![allow(clippy::exit)]          // CLI uses exit codes
#![allow(clippy::unreachable)]   // Exhaustive matching
#![allow(clippy::fn_params_excessive_bools)]  // CLI flags
```

---

## Build Configuration

### `.cargo/config.toml`

```toml
[build]
rustflags = ["-D", "warnings"]

[alias]
lint = "clippy --workspace --all-targets --all-features -- -D warnings"
fix-all = "clippy --workspace --all-targets --all-features --fix --allow-dirty"
fmt-check = "fmt --all -- --check"
```

### CI Commands

```bash
# Format check
cargo fmt --all -- --check

# Lint check
cargo clippy --workspace --all-targets --all-features

# Full CI
cargo fmt --all -- --check && cargo clippy --workspace --all-targets --all-features && cargo test --workspace
```

---

## Performance Idioms

### Avoid Unnecessary Allocations

```rust
// Good: collect only when needed
for item in items.iter() { }

// Bad: unnecessary allocation
for item in items.iter().collect::<Vec<_>>() { }
```

### Prefer Iterators

```rust
// Good: lazy evaluation
fn items(&self) -> impl Iterator<Item = &Item> {
    self.items.iter()
}

// Bad: eager allocation
fn items(&self) -> Vec<&Item> {
    self.items.iter().collect()
}
```

### Use `Cow` for Conditional Ownership

```rust
use std::borrow::Cow;

fn normalize(s: &str) -> Cow<'_, str> {
    if needs_normalization(s) {
        Cow::Owned(do_normalization(s))
    } else {
        Cow::Borrowed(s)
    }
}
```

---

## Security Patterns

### Path Validation

```rust
fn validate_seatbelt_path(s: &str) -> Result<(), PolicyError> {
    // Reject characters that could escape S-expression literals
    if s.contains('"') || s.contains('(') || s.contains(')')
        || s.contains('\n') || s.contains('\r') || s.contains('\0') {
        return Err(PolicyError::UnsafePath);
    }
    Ok(())
}
```

### Secret Handling

Never log secrets. Use wrapper types:

```rust
pub struct Secret<T>(T);

impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
```

### Environment Variables

Don't use `std::env::set_var` (not thread-safe). Configure in `clippy.toml`:

```toml
disallowed-methods = [
    { path = "std::env::set_var", reason = "not thread-safe" },
    { path = "std::env::remove_var", reason = "not thread-safe" },
]
```

---

## Quick Checks

Before committing:

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets --all-features

# Test
cargo test --workspace

# All at once
cargo fmt --all && cargo clippy --workspace --all-targets --all-features && cargo test --workspace
```

# Contributing to ptybox

Thank you for your interest in contributing to ptybox! This document provides guidelines and instructions for contributing.

## About This Project

This project was developed entirely using AI coding assistants ([Claude Code](https://claude.ai/code) and [Codex CLI](https://github.com/openai/codex)). Contributions via AI-assisted development are welcome and encouraged.

## Code of Conduct

This project adheres to the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code.

## Getting Started

### Prerequisites

- Rust 1.75 or later (check with `rustc --version`)
- macOS or Linux (Windows is not supported)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/lelanhus/ptybox
cd ptybox

# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace --all-features

# Run lints (must pass with zero warnings)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Code Standards

### Zero Warnings Policy

All code must compile without warnings. CI enforces `-D warnings` for both rustc and clippy.

### No `unwrap()` or `expect()` in Library Code

Library code (`crates/ptybox/`) must propagate errors with context:

```rust
// Bad
let file = File::open(path).unwrap();

// Good
let file = File::open(path)
    .into_diagnostic()
    .wrap_err_with(|| format!("failed to open: {}", path.display()))?;
```

Test code may use `unwrap()` and `expect()` for clarity.

### No `unsafe` Code

The library forbids unsafe code: `#![forbid(unsafe_code)]`

### Error Handling

- Use `miette` for error reporting with context
- Return stable error codes (e.g., `E_POLICY_DENIED`)
- Include actionable remediation in error messages

### Formatting

Run `cargo fmt --all` before committing. The project uses default rustfmt settings.

## Making Changes

### Spec-First Discipline

Any change to public types, CLI protocol, or default behavior must update:

1. `spec/data-model.md` - Type definitions and protocols
2. `spec/feature-list.json` - Feature status tracking
3. `CHANGELOG.md` - Version history

### Commit Messages

Use conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
```
feat(runner): add retry support for flaky assertions
fix(policy): reject relative paths in fs allowlists
docs(readme): add installation instructions
```

### Pull Request Process

1. Fork the repository and create a feature branch
2. Make your changes with appropriate tests
3. Ensure all checks pass:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-features
   ```
4. Update documentation if needed
5. Submit a pull request with a clear description

### What to Include in a PR

- **Title**: Clear, concise summary using conventional commit style
- **Description**: What changed and why
- **Testing**: How the change was tested
- **Breaking changes**: Any compatibility implications

## Project Structure

```
ptybox/
├── crates/
│   ├── ptybox/           # Core library
│   │   ├── src/
│   │   │   ├── session/   # PTY management
│   │   │   ├── terminal/  # VT100 parsing
│   │   │   ├── policy/    # Security sandbox
│   │   │   ├── runner/    # Step execution
│   │   │   └── model/     # Domain types
│   │   └── tests/
│   ├── ptybox-cli/       # CLI binary
│   └── ptybox-fixtures/  # Test fixtures
├── spec/                   # Specifications
│   ├── data-model.md      # Types and protocols
│   ├── plan.md            # Architecture
│   └── feature-list.json  # Feature tracking
└── scripts/               # Build/test scripts
```

## Testing

### Running Tests

```bash
# All tests
cargo test --workspace --all-features

# Specific test
cargo test --workspace test_name

# With output
cargo test --workspace -- --nocapture
```

### Writing Tests

- Place unit tests in the same file as the code
- Place integration tests in `tests/` directories
- Use the fixture programs in `ptybox-fixtures` for CLI tests

### Test Fixtures

The `ptybox-fixtures` crate provides purpose-built TUI programs for testing:

- `ptybox-echo-keys` - Echoes keypresses
- `ptybox-show-size` - Displays terminal size
- `ptybox-delay-output` - Outputs after delay
- `ptybox-exit-code` - Exits with specified code
- `ptybox-unicode-test` - Prints Unicode/emoji

## Security

### Reporting Vulnerabilities

Please report security vulnerabilities privately. See [SECURITY.md](SECURITY.md) for details.

### Security-Sensitive Areas

Be especially careful when modifying:

- `crates/ptybox/src/policy/` - Policy validation and sandbox
- `crates/ptybox/src/session/` - Process spawning
- Any code handling file paths or environment variables

## Questions?

- Open a [GitHub issue](https://github.com/lelanhus/ptybox/issues) for bugs or features
- Check existing issues before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).

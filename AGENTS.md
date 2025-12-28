# Agent Guidelines (ptybox)

These instructions apply to the entire repository.

## Quality bar
- Treat this as a production-grade project: correct, secure, maintainable, and predictable.
- Prefer **simplicity over cleverness**; avoid over-engineering and needless abstraction.
- Be explicit; avoid “magic” behavior and silent fallbacks.

## Clean repo rule
- Always leave the project in a clean state: no stray debug output, no failing tests, no broken builds, no half-finished refactors.
- Fix formatting and lint issues as part of the change that introduced them (don’t defer).

## Warnings / lint / format
- **No warnings** in our code. Treat warnings as errors for CI and local checks.
- Run and keep passing (when available):
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`

## Coding style
- Favor declarative / functional patterns where they simplify reasoning (pure functions, immutable data flow, explicit transforms).
- Keep modules cohesive and small; avoid “god” modules.
- Prefer typed domain models over loosely-typed maps/strings.
- Error handling:
  - Fail fast and loud with actionable messages.
  - No silent `ok()`/ignored `Result` in production code.
  - Avoid `unwrap()`/`expect()` outside tests; propagate with context.

## Architecture preference
- Use a **vertical-slice, feature-based** structure: each feature owns its types, errors, tests, and documentation touchpoints.
- Keep shared “core” minimal and stable.

## Testing (TDD, meaningful, non-brittle)
- Use a TDD workflow: write/adjust a failing test first, then implement, then refactor.
- Only add tests that verify real behavior or invariants; avoid “fake” tests that merely mirror implementation.
- Tests should be isolated and deterministic:
  - Avoid time-based sleeps; prefer “wait until condition” patterns with bounded timeouts.
  - Avoid network and external dependencies unless explicitly required and hermetically controlled.
  - Prefer small fixtures and clear assertions with good diagnostics.

## Workflow / git hygiene
- Prefer feature branches with **atomic commits**; merge by **squash**.
- Don’t rewrite history unless explicitly requested.
- Don’t commit or create branches unless the user asks; if asked, follow the above rules.

## Spec-first discipline
- `spec/data-model.md` is the source of truth for public types and protocols.
- Any public behavior/type/protocol change must also update:
  - `spec/data-model.md`
  - `spec/feature-list.json`
  - `CHANGELOG.md`


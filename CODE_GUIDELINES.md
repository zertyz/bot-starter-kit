# Denied Constructs

The rules state here are enforced by `scripts/enforce_code_guidelines`. Keep these files in sync and use that script in CI/CD rules.
Scope: Unless a rule explicitly says otherwise, guidelines apply to production/project code and do not apply to unit tests, integration tests, or examples.

## Scope

Unless a rule explicitly says otherwise, these guidelines apply to production/project code and do not apply to unit tests, integration tests, or examples.

Unit tests are Rust modules guarded by `#[cfg(test)]`. Integration tests live under `tests/`. Examples live under `examples/`.

## 1. Error Context Hiding

Do not allow `anyhow::Context` and related `.context` & `.with_context` high-order-functions.

Reason: they drop the error context, promoting a partial error hiding anti-pattern.
Alternative: use `.map_err(|err| anyhow!("At operation X: {err}"))`, which correctly preserves the root cause `err`

## 2. Program-Crashing Shortcuts Outside Tests

Do not allow panic-based shortcuts in production Rust source under `src/`, including `.expect(...)`, `.unwrap(...)`, `panic!(...)`, `todo!(...)`, `unimplemented!(...)`, `unreachable!(...)`, `assert!(...)`, `assert_eq!(...)`, and `assert_ne!(...)`.

Reason: production code should return or log recoverable errors rather than aborting the process.
Alternative: use `Result`, `Option` handling, explicit error variants, or `.map_err(|err| anyhow!("At operation X: {err}"))` as appropriate.

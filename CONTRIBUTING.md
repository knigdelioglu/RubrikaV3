# Contributing — Rust + Tauri Projects

This guide is reusable for desktop applications built with Rust, Tauri, and a web frontend.

## 1. Working rules

- Work task-by-task and keep the patch understandable.
- Separate unrelated UI, domain, storage, and packaging changes when practical.
- Preserve architecture boundaries.
- Prefer root-cause fixes over symptom hiding.
- Keep public commands and DTOs typed.
- Use background jobs only when the operation benefits from progress, cancellation, or asynchronous execution.
- Do not introduce silent fallback or production mock behavior.

## 2. Code quality

- Add tests for behavior changes.
- Cover success and important failure paths when practical.
- Use structured application errors at the Tauri boundary.
- Avoid raw technical errors in the main UI.
- Use atomic file writes or database transactions where partial writes are unsafe.
- Keep one canonical writer for mutable application state.
- Avoid floating-point values where exact decimal behavior is required.
- Do not persist placeholder UI values as real data.

## 3. Documentation

Update documentation only when the corresponding contract changes:

- public command or DTO
- storage format or migration
- error contract
- job/event behavior
- build, packaging, signing, updater, or release process
- architecture boundary

Small internal refactors do not require broad documentation rewrites.

## 4. Validation matrix

Use the repository's actual scripts and package manager. The commands below are examples, not mandatory names.

### Documentation-only

- documentation checks, if configured

### Frontend-only

- frontend typecheck
- frontend lint
- targeted frontend tests

### Rust-only local change

- `cargo fmt --check`
- targeted Rust tests
- Clippy for the affected target or package

### Cross-layer Tauri change

- targeted Rust command/service tests
- targeted frontend API/component tests
- frontend typecheck
- relevant lint
- `cargo fmt --check`
- relevant Clippy

### High-risk, merge, or release change

- full frontend tests
- full Rust workspace tests
- workspace Clippy with required targets/features
- Tauri build or smoke check
- migration, packaging, signing, or updater checks as applicable

## 5. Test execution policy

- Run targeted tests during development.
- Run broader checks after the implementation stabilizes.
- Do not run every Rust test after every edit.
- Run the full suite at most once per task by default.
- A pure UI/CSS change does not require `cargo test`.
- A local Rust helper change does not require every frontend test.
- Full validation is required when the change affects shared domain logic, storage/migrations, startup, security, concurrency, packaging, or release behavior.
- Report any skipped or failing check with its impact.

Use this format when needed:

```text
Skipped or failed: <command>
Reason:
Impact on confidence:
```

## 6. Pull request checklist

- [ ] Scope matches the requested change.
- [ ] No unrelated files were modified.
- [ ] Frontend does not duplicate authoritative Rust domain logic.
- [ ] New or changed Tauri commands use typed input/output.
- [ ] Errors are structured and user-readable.
- [ ] State-changing operations use the canonical storage path.
- [ ] Behavior changes have proportional test coverage.
- [ ] Targeted checks pass.
- [ ] Full checks were run when the risk level required them.
- [ ] Skipped or unrelated failures are documented.
- [ ] Relevant contract documentation was updated.
- [ ] Manual verification steps are included when needed.

## 7. Before merge or release

Before merging a broad change or publishing a build, run the project's full validation pipeline. A typical pipeline may include:

```text
frontend typecheck
frontend lint
frontend full test suite
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
Tauri build or release smoke check
```

Adjust feature and target flags to the repository. Do not run expensive combinations that the project does not support.

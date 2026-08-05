# Rust + Tauri Engineering Standards

Target stack: Rust + Tauri + a web frontend such as React, Vue, Svelte, or Solid.

This file defines the default engineering rules for coding agents. Keep it short, task-focused, and reusable across projects.

## 1. Operating principles

- Keep the requested scope narrow.
- Prefer the smallest correct change over a broad rewrite.
- Do not modify unrelated modules merely because you noticed possible improvements.
- Do not claim success without running the checks appropriate to the changed area.
- Never hide a backend or data problem with a UI-only workaround.
- Do not introduce silent fallback, fake success, placeholder persistence, or production mock behavior.
- Stop and report when another process is actively changing the same files or when the working tree changes unexpectedly.

## 2. Read only what is relevant

Before editing:

1. Read this file.
2. Read `ARCHITECTURE.md` only when the task crosses architectural boundaries or the repository explicitly requires it.
3. Read only the documentation directly related to the requested change.
4. Inspect the target files and their nearest callers, types, and tests.

Do not scan every document or the entire repository for a small, well-scoped task.

Examples:

- CSS/layout task: page, shared components, styles, affected UI tests.
- Tauri command task: command, DTOs, service, frontend client, command tests.
- Storage task: store/repository, schema or serialization types, migration, persistence tests.
- Rust domain task: domain type, owning service, direct callers, unit/integration tests.

## 3. Architecture boundaries

### 3.1 Frontend

The frontend owns:

- presentation
- local selection and modal state
- unsaved form drafts
- loading, success, and error feedback
- rendering backend-provided state

The frontend must not independently own domain rules that must remain authoritative in Rust.

### 3.2 Tauri command layer

Commands should:

- have typed inputs and outputs
- perform validation or delegate it to a domain service
- map failures into structured application errors
- avoid hidden unrelated side effects
- return promptly

Use a background job only for work that is genuinely long-running or needs progress/cancellation. Small reads and writes should remain normal commands.

### 3.3 Rust domain and services

- Put business rules in Rust services or domain modules.
- Keep one clear owner for each mutable state area.
- Avoid god services that mix storage, UI concerns, jobs, and unrelated domain logic.
- Use fixed-precision or integer representations for values that must not accumulate floating-point error.
- Production paths should return typed errors instead of using `unwrap`, `expect`, `panic!`, `todo!`, or `unimplemented!` for recoverable conditions.

### 3.4 Storage

- Use atomic file replacement or database transactions where partial writes would corrupt state.
- Validate identifiers and paths before file operations.
- Avoid multiple independent writers for the same canonical state.
- Migrations and repair operations must be idempotent where practical.

## 4. UI and error behavior

Every user action should provide:

- a disabled reason when unavailable
- a loading state while running
- visible success feedback when completed
- a user-readable error when failed

Raw stack traces, Rust internals, database messages, and internal error codes belong in diagnostics, not the main UI.

## 5. Testing policy: progressive, not exhaustive by default

Validation must match the risk and scope of the change. Do not run every test suite after every edit.

### Level A — Documentation or comments only

Run:

- markdown/document checks if the repository has them

Do not run Rust or frontend test suites unless the documentation change affects generated code or executable examples.

### Level B — Frontend-only change

Run:

- typecheck for the affected frontend package
- lint for changed files or the frontend package
- targeted component/page tests

Run the full frontend test suite only when shared routing, global state, API wrappers, or widely reused components changed.

Do not run `cargo test` for a pure frontend change.

### Level C — Local Rust change

Run:

- `cargo fmt --check`
- targeted Rust tests, for example:
  - `cargo test module_name`
  - `cargo test test_name -- --exact`
  - `cargo test --test integration_file`
- targeted Clippy where possible, otherwise `cargo clippy -- -D warnings`

Run the full Rust suite only if the change affects shared domain logic, public traits, serialization, storage, migrations, concurrency, build features, or many callers.

### Level D — Frontend/backend boundary change

Run:

- targeted Rust tests for the command/service
- targeted frontend tests for the client and screen
- frontend typecheck
- lint for affected packages
- `cargo fmt --check`
- Clippy for the affected Rust targets

Run full suites only if shared command infrastructure, global DTOs, router/state, or common services changed.

### Level E — High-risk or release validation

Full validation is required for changes involving one or more of:

- storage schema or migrations
- authentication or authorization
- security-sensitive file/path handling
- concurrency or background jobs
- shared domain engine or calculation core
- application startup
- packaging, signing, updater, or release configuration
- broad refactors across multiple layers
- release, merge, or explicit user request

Typical full validation:

```text
frontend typecheck
frontend lint
frontend full tests
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
Tauri build or smoke check when relevant
```

Use the repository's actual package manager and scripts instead of assuming `npm`.

## 6. Efficient validation rules

- Start with the narrowest relevant test.
- Run broader checks only after targeted checks pass.
- Run a full suite at most once per task by default.
- After a small fix, rerun the failed or affected check; do not automatically rerun every previously passing suite.
- Do not run both a project script and an equivalent raw command unless they verify different behavior.
- Do not use `--all-features` or `--all-targets` unless the task or release policy requires them.
- If a known unrelated test fails, prove it is unrelated with the smallest practical check and report it; do not spend unlimited time repairing out-of-scope failures.
- Never modify tests merely to make an unrelated failure disappear.

## 7. Test expectations by change type

Add or update tests when the change affects behavior.

Prefer:

- unit tests for domain logic
- command contract tests for Tauri boundaries
- component tests for UI behavior
- integration tests for persistence or multi-module workflows
- regression tests for a reproduced bug

Do not add redundant tests that repeat existing coverage without increasing confidence.

## 8. Documentation updates

Update documentation only when a contract changes, such as:

- public Tauri command or DTO
- storage format or migration
- error code or user-visible failure contract
- job/event contract
- architecture boundary
- setup, build, packaging, or release procedure

Do not rewrite unrelated documentation during a narrow feature task.

## 9. Completion criteria

A task is complete when:

- requested behavior is implemented
- relevant tests exist and pass
- validation appropriate to the change level has been run
- skipped checks and their impact are reported
- no known unrelated edits were introduced
- manual verification steps are provided when automated checks cannot cover the behavior

Do not state that the entire project is green unless the entire project suite was actually run.

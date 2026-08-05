# Coding Agent Task Protocol — Rust + Tauri

Use this protocol for focused implementation tasks. It is intentionally lightweight so small tasks do not become repository-wide audits.

## 1. Start of task

Before editing, provide a short plan only when the task is not trivial:

```text
Files to inspect:
Files likely to modify:
Behavior or contract affected:
Targeted tests:
Main risk:
```

For a one-file typo, style adjustment, or obvious local fix, begin directly after inspecting the target file.

## 2. Investigation limits

- Start from the files named by the user.
- Expand only to direct callers, shared types, owning services, and relevant tests.
- Do not read every file under `docs/`.
- Do not perform a repository-wide search unless the symbol location or root cause is unknown.
- Do not redesign adjacent systems without explicit approval.
- If the task reveals a larger unrelated issue, report it separately instead of silently expanding scope.

## 3. Implementation rules

- Keep domain authority in Rust when the rule must be consistent across UI, commands, and background work.
- Use typed Tauri command inputs and outputs.
- Keep commands narrow; one command should not perform several unrelated workflows.
- Use jobs only for operations that are genuinely long-running, cancellable, or progress-reporting.
- Preserve existing public contracts unless the task requires a change.
- Add structured error handling for realistic failure paths.
- Do not use production mocks, fake success, or silent fallback.
- Do not create a git commit unless explicitly requested.

## 4. Progressive validation

Choose the smallest validation set that can establish confidence.

### Frontend-only

Run:

```text
frontend typecheck
frontend lint
related component/page tests
```

Do not run Rust tests unless the frontend change depends on modified Rust behavior.

### Rust-only local change

Run:

```text
cargo fmt --check
targeted Rust test(s)
targeted Clippy or cargo clippy -- -D warnings
```

Do not run the entire workspace suite for a local helper or isolated service change unless shared contracts changed.

### Tauri command or cross-layer change

Run:

```text
targeted command/service Rust tests
targeted frontend API/UI tests
frontend typecheck
relevant lint
cargo fmt --check
relevant Clippy
```

### Full suite

Run a full workspace/frontend suite only when:

- shared domain or storage behavior changed
- migrations, concurrency, startup, security, or packaging changed
- the refactor is broad
- release/merge validation is requested
- the user explicitly asks for all tests

A full suite should normally be run once, near task completion, not after every edit.

## 5. Failure handling

When a check fails:

1. Determine whether the failure is caused by the current change.
2. Fix and rerun the smallest affected check.
3. Broaden validation only after the targeted check passes.
4. If another process changes the same files, stop and report before continuing.
5. If a known unrelated failure exists, report the command, failure, and impact. Do not claim a clean full suite.

Do not repeatedly run the same expensive suite without a specific reason.

## 6. Completion report

Keep the report concise:

```text
What changed
Why
Files modified
Contracts/types affected
Tests run and results
Checks deliberately not run
Known limitations or unrelated failures
Manual verification steps
```

Use exact test counts only when they come from actual command output.

## 7. Scope examples

Good:

```text
Add a typed Tauri command that updates one settings record, connect it to the existing form, and add command plus component tests.
```

Too broad:

```text
Redesign settings, rewrite storage, add auto-update, refactor all commands, and modernize the UI.
```

## 8. Stop conditions

Stop and ask or report when:

- requirements conflict
- a destructive migration is required but not authorized
- the correct source of truth is unclear
- another process is actively modifying the same files
- the task would require a broad architectural change outside the request
- required credentials, hardware, signing identities, or external services are unavailable

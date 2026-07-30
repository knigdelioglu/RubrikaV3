# Codex Task Protocol

## Task Protocol
- Inspect the current architecture and the relevant docs before editing code.
- Keep scope narrow and do not introduce unrelated workflow logic.
- Use typed backend commands for state-changing actions.
- Treat long-running work as jobs, not synchronous UI work.

## Read-First Checklist
- `AGENTS.md`
- `ARCHITECTURE.md`
- `docs/API_CONTRACTS.md`
- `docs/WORKFLOW_STATES.md`
- `docs/ERROR_CODES.md`
- `docs/MODEL_GATEWAY.md`

## Implementation Checklist
- State the files to modify before editing.
- Prefer backend/domain fixes over UI-only patches.
- Keep workflow, model status, job status, and project state connected.
- Do not persist placeholder data as real rubric or question content.

## Validation Checklist
- Run the requested typecheck, lint, and test commands.
- Run the Rust formatting, clippy, and test commands.
- Report any skipped or failing command explicitly.
- Do not claim a test passed unless it was actually run.

## Completion Report Template
- Root cause
- Modified files
- Added command/job/type
- Test results
- Manual smoke result
- Deliberately not done
- Remaining risks
- Can the next milestone start?

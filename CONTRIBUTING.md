# Contributing

## Working Rules
- Work branch-by-branch or task-by-task.
- Keep changes aligned with the current milestone.
- Do not merge UI, workflow, and storage changes into one opaque patch if they can be split.

## Documentation Rules
- Update `docs/API_CONTRACTS.md` when a public command changes.
- Update `docs/WORKFLOW_STATES.md` when workflow states or transitions change.
- Update `docs/ERROR_CODES.md` when error codes are added or renamed.
- Update `docs/JOB_SYSTEM.md` when job kinds or job event rules change.
- Update `docs/MODEL_GATEWAY.md` when model transport or diagnostics change.

## Quality Rules
- Domain changes require tests.
- Tests must cover both success and failure paths when practical.
- Do not introduce silent fallback behavior without visible diagnostics.
- Keep teacher-facing labels free of raw technical codes.

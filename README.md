# Rubrika v3

Rubrika v3 is a controlled re-architecture of the Rubrika grading application, built with React, TypeScript, Tauri, Rust, and llama.cpp.

## Documentation

- [Engineering Standards](docs/ENGINEERING_STANDARDS.md) - The core rules and architectural guidelines.
- [API Contracts](docs/API_CONTRACTS.md) - Tauri command definitions.
- [Workflow States](docs/WORKFLOW_STATES.md) - The deterministic stages of a project.
- [Error Codes](docs/ERROR_CODES.md) - The unified error handling strategy.
- [Job System](docs/JOB_SYSTEM.md) - Background tasks and event emission.
- [Model Gateway](docs/MODEL_GATEWAY.md) - AI integration strategy.
- [Project Format](docs/PROJECT_FORMAT.md) - On-disk project structure.
- [Codex Task Protocol](docs/CODEX_TASK_PROTOCOL.md) - Rules for AI agents.

## Development

Install dependencies:
```bash
npm install
```

Start the application in development mode:
```bash
npm run tauri:dev
```

Tablet MVP (local network):

The desktop app can expose an opt-in read-only mobile API. Start it with a
LAN address and a temporary access token, for example:

```bash
VITE_HOST=0.0.0.0 RUBRIKA_LAN_API=1 RUBRIKA_LAN_API_HOST=0.0.0.0 RUBRIKA_LAN_API_TOKEN=change-me npm run tauri:dev
```

Then open the `.local` web address shown in MacBook → Ayarlar → Tableti bağla
on the tablet. The tablet fills the matching `.local:8787` API address
automatically; enter the same token once. Both devices must be on the same
Wi‑Fi network with local network discovery enabled.
The first MVP exposes health and project-list/project-read endpoints only;
evaluation and file-upload endpoints remain on the desktop Tauri boundary
until their mobile conflict and progress contracts are defined.

Development mode allows disposable local projects to continue without a
verified-backup/release-proof marker. Real project-integrity findings still
block writes. Set `RUBRIKA_ALLOW_UNVERIFIED_PROJECT_WRITES=0` before the
command to exercise the strict gate.

Run quality checks (typecheck, lint, formatting, tests):
```bash
npm run quality
```

For a faster development loop, use `npm run check:fast` for frontend typecheck
and Rust compilation checks. `npm run cargo:test` prefers `cargo-nextest` when
installed; see [Fast testing](docs/FAST_TESTING.md) for linker, test-runner,
and security-scanner guidance.

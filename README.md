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

Run quality checks (typecheck, lint, formatting, tests):
```bash
npm run quality
```

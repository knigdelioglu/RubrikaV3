# Codex Task Protocol

This protocol defines the standard operating procedure for AI agents working on the Rubrika v3 codebase.

## Workflow Rules

1. **Inspect Files First**: Always examine the current state of relevant files (`view_file`, `list_dir`) before making changes. Understand the existing architecture and standards.
2. **State Expected Modifications**: Clearly outline which files will be modified and why, directly corresponding to the requested task.
3. **Do Not Broaden Scope Silently**: Stick strictly to the defined goal. Do not refactor unrelated code, add "nice-to-have" features, or implement unrequested domain logic.
4. **Add Tests**: When adding new functionality (especially in domain services or command contracts), include corresponding unit or integration tests.
5. **Run Checks**: Run the appropriate quality gate scripts (e.g., `npm run quality`, `cargo check`, `cargo fmt`) to ensure your changes do not introduce regressions or formatting issues.
6. **Report Results**: Provide a clear summary of the completed work, any skipped items, and remaining missing checks or components.

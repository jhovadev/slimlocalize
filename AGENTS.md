# AGENTS.md

## Repository layout

- `crates/core`: domain and services, no framework dependencies.
- `crates/storage`: SQLite adapter and migrations.
- `crates/server`: optional read-only Axum server.
- `crates/cli`: `localize` executable.
- `crates/mcp`: read-only MCP tool registry.
- `crates/collaboration`: Loro prototype.
- `crates/security`: security primitives and contracts.
- `packages/platform`: TypeScript contracts and adapters.
- `packages/ui`: SolidJS design system and workspace UI.
- `apps/client`: Tauri desktop/mobile client.
- `apps/web`: web build.
- `fixtures/locales`: reusable import fixtures.

## Verification commands

```bash
just fmt-check
just lint
just typecheck
just test
just build
just docker-config
```

## Architecture limits

- Rust core must not depend on Tauri, Axum, SQLite or CLI.
- UI components must use design tokens from `packages/ui/src/styles`.
- Do not expose filesystem, shell, HTTP or dialog capabilities in Tauri.
- Do not store secrets in plaintext or Loro documents.
- Remote MCP and write tools remain disabled.

## Adding a feature

1. Implement domain logic in `crates/core`.
2. Add storage methods and migrations if needed.
3. Update CLI, Tauri and server adapters.
4. Add tests.
5. Update documentation and status.

## Testing rule

Never report a test as passed unless it has been executed. Mocks are not evidence of completed functionality.

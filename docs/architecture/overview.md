# Architecture overview

SlimLocalize is a local-first monorepo. The Rust domain core is the source of
truth for desktop and CLI. Adapters (Tauri, Axum, CLI, MCP) call the same
application service.

## Dependency direction

- `core`: no Tauri, Axum, SQLite, CLI or web dependencies.
- `storage`: implements the `Store` port with SQLite.
- `server`, `cli`, `mcp`, `client`: consume `core` and `storage`.
- TypeScript packages consume contracts through `packages/platform`.

## Persistence

SQLite is embedded and owned by the storage crate. Migrations are versioned and
applied automatically. WAL mode is enabled for local responsiveness.

## Collaboration

Loro is used only behind the `collaboration` crate. The F1 app writes through
Rust core and SQLite. Shared CRDT documents are not yet exposed by the UI or
server.

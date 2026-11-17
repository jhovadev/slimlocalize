# Contributing

## Prerequisites

```bash
mise trust
mise install
just doctor
just setup
```

## Development

```bash
just dev-desktop
just dev-web
just dev-server
just test
just ci
```

## Rules

- Keep Rust domain code independent from Tauri, Axum, SQLite and CLI.
- Add tests for every domain change.
- Do not introduce SaaS dependencies for core functionality.
- Do not commit local databases or `.env` files.
- Run `just ci` before requesting review.

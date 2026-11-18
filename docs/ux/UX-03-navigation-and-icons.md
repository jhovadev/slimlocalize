# SlimLocalize UX-03 — Navigation and icon reliability

- Inline, bundled SVG Lucide-style icons are used instead of generated CSS icon classes. This eliminates the missing-icon problem across Vite/Tauri builds.
- The topbar now presents a real **New project** action and a project breadcrumb without technical badges.
- Project creation appears in a dialog rather than an almost invisible square button in the sidebar.
- The sidebar retains only the project list and section navigation.
- No repository storage, Loro, or API schema is changed.

## Not yet implemented (avoid misleading users)

Rename project / translation key requires additional domain methods, storage transactions, Tauri commands and web-adapter parity. Never simulate this with create/delete because it can disrupt references and CRDT convergence.

## Test collaboration already available

`just sync-relay-e2e` verifies independent Rust clients exchanging Loro updates through the development relay. It is a separate test from two **GUI instances** live-syncing. The current Tauri app's collaboration panel supports manual bootstrap/export/import, not automatic authenticated live sync. `just sync-up` starts a development relay; it does not connect the GUI automatically.

## Validation on real checkout

`python3 apply.py --check --repo .` then `python3 apply.py --apply --repo .`; `bun run format && bun run typecheck && bun run --cwd packages/ui test && just ci && just dev-desktop`.

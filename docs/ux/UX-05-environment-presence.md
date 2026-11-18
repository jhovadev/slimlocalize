# UX-05 — Shared environments and presence (local development)

A shared project now carries the non-secret environment **names/kinds** and **public configuration variables** in its existing Loro document. A variable marked `local` is never added to a collaborative document. Edits to locally overridden variables on the receiving device are not overwritten by remote public values.

This does **not** implement collaborative secret management, authenticated identities, secure revocation, or native Tauri relay connections. A value that was previously shared cannot be made confidential simply by changing its visibility: other devices may already have retained it. Rotate sensitive credentials if accidentally shared.

Presence is transmitted via the official `%EPH` Loro Ephemeral Store adaptor over the already-connected WebSocket. It expires and is not persisted to SQLite or IndexedDB. Display names are self-asserted _guest labels_ in development; they are **not verified user identities**. The UI only shows a collaborator indicator when another active peer is actually observed. The focus hint refers to the selected key or environment, not a text caret with CRDT-stable position.

### How to try

1. `just sync-up` then `just dev-web`.
2. Open the URL printed by Vite in a normal browser profile.
3. Create a project and connect from Settings. Set a display name.
4. Copy the pairing code; join in a separate browser profile/private session at the same origin.
5. In A, add a public variable to an environment. Verify B receives it. In B, edit it. Verify A updates.
6. In A, add a `local` variable. Verify it is absent in B.
7. Select a key or environment on A. Verify B displays a collaborator avatar and the selected location. Leave/close A and verify it disappears after expiry (~35 s).
8. Disconnect B, modify a public variable, reconnect and verify convergence. Close/reopen both browser profiles and confirm persistent data survives.

Run `bun run typecheck`, `bun run test`, `bun run --cwd apps/web build`, `just ci`, and E2E browser tests before committing. **Do not expose the development relay publicly.**

### Technical boundaries

- Updates are Loro map operations under `environments` and `env_variables`, scoped by project document ID. Existing translation schema remains unchanged.
- Both clients need the same project ID, supplied by the existing pairing code.
- The sync relay runs in development mode and currently grants room access without production authentication.
- Collaboration with Tauri, managed secrets, environment/variable deletions, and permission enforcement require separate changes to Rust services and persistence.

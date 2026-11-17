# Collaboration design

F2 separates four concerns:

1. CRDT document (`collaboration` crate).
2. Local persistence (`storage` crate).
3. Sync transport (optional relay or local files).
4. Authorization (development mode only in F2A).

Authority rule: when collaboration is enabled, the CRDT document is the authority
for shared translation values. SQLite projections remain queryable derivatives.

Current implementation:

- Durable per-project Loro documents.
- Append-only update log.
- Snapshot and pending-update persistence.
- File-based offline exchange.
- Optional Loro WebSocket relay.
- SQL projection rebuild.

Not yet implemented:

- User authentication.
- Remote authorization.
- Invitations.
- Production relay hardening.
  EOF

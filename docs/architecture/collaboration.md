# Collaboration design

F2 separates four concerns:

1. CRDT document (`collaboration` crate).
2. Local persistence (`storage` crate).
3. Sync transport (future).
4. Authorization (future).

Authority rule: when collaboration is enabled, the CRDT document is the authority
for shared translation text. SQLite projections remain queryable derivatives.

Current prototype demonstrates two-replica convergence, serialization,
restoration and idempotent imports. It is not a complete sync system.

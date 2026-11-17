# CRDT persistence

Collaborative documents are persisted in SQLite:

- `collab_peers`: durable peer identity.
- `collab_documents`: document metadata and schema version.
- `collab_snapshots`: full Loro snapshot, pending sidecar and version vector.
- `collab_updates`: append-only update log.
- `collab_checkpoints`: latest version vector.

On open, SlimLocalize restores the snapshot, imports pending updates, then imports
the update log. Repeated imports are idempotent.

SQL translation tables are projections rebuilt from the CRDT document.
EOF

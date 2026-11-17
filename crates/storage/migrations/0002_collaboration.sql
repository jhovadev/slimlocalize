-- SlimLocalize collaboration schema, version 2.
-- Persists Loro documents, snapshots, incremental updates and peer identity.
-- This migration is additive and does not modify F1 tables.

CREATE TABLE collab_peers (
    peer_id TEXT PRIMARY KEY NOT NULL,
    device_name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE collab_documents (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    document_key TEXT NOT NULL,
    schema_version INTEGER NOT NULL DEFAULT 1,
    peer_id TEXT NOT NULL REFERENCES collab_peers(peer_id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (project_id, document_key)
);

CREATE INDEX idx_collab_documents_project ON collab_documents(project_id);

CREATE TABLE collab_snapshots (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES collab_documents(id) ON DELETE CASCADE,
    snapshot BLOB NOT NULL,
    pending BLOB,
    version_vector BLOB NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE collab_updates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id TEXT NOT NULL REFERENCES collab_documents(id) ON DELETE CASCADE,
    update_blob BLOB NOT NULL,
    origin_peer_id TEXT,
    created_at TEXT NOT NULL,
    applied INTEGER NOT NULL DEFAULT 1 CHECK (applied IN (0, 1))
);

CREATE INDEX idx_collab_updates_document ON collab_updates(document_id, id);

CREATE TABLE collab_checkpoints (
    document_id TEXT PRIMARY KEY NOT NULL REFERENCES collab_documents(id) ON DELETE CASCADE,
    version_vector BLOB NOT NULL,
    updated_at TEXT NOT NULL
);

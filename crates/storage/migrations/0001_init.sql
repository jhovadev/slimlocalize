-- SlimLocalize local schema, version 1.
-- SQLite is the embedded source of truth for the desktop app and CLI.
-- All timestamps are RFC 3339 UTC strings. All identifiers are UUID v4 text.

CREATE TABLE workspaces (
    id          TEXT PRIMARY KEY NOT NULL,
    slug        TEXT NOT NULL UNIQUE CHECK (length(slug) BETWEEN 1 AND 64),
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    description TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE workspace_members (
    id           TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    subject      TEXT NOT NULL,
    role         TEXT NOT NULL CHECK (role IN ('owner', 'maintainer', 'translator', 'viewer')),
    created_at   TEXT NOT NULL,
    UNIQUE (workspace_id, subject)
);

CREATE INDEX idx_workspace_members_workspace ON workspace_members(workspace_id);

CREATE TABLE projects (
    id             TEXT PRIMARY KEY NOT NULL,
    workspace_id   TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    slug           TEXT NOT NULL CHECK (length(slug) BETWEEN 1 AND 64),
    name           TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    description    TEXT,
    default_locale TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    UNIQUE (workspace_id, slug)
);

CREATE INDEX idx_projects_workspace ON projects(workspace_id);

CREATE TABLE locales (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    code       TEXT NOT NULL CHECK (length(code) BETWEEN 2 AND 35),
    name       TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL,
    UNIQUE (project_id, code)
);

CREATE INDEX idx_locales_project ON locales(project_id);
CREATE UNIQUE INDEX idx_locales_one_default
    ON locales(project_id) WHERE is_default = 1;

CREATE TABLE namespaces (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    created_at TEXT NOT NULL,
    UNIQUE (project_id, name)
);

CREATE INDEX idx_namespaces_project ON namespaces(project_id);

CREATE TABLE translation_keys (
    id           TEXT PRIMARY KEY NOT NULL,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    namespace_id TEXT NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    key          TEXT NOT NULL CHECK (length(key) BETWEEN 1 AND 256),
    description  TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    UNIQUE (namespace_id, key)
);

CREATE INDEX idx_translation_keys_project ON translation_keys(project_id);
CREATE INDEX idx_translation_keys_key ON translation_keys(key);

CREATE TABLE translation_values (
    id         TEXT PRIMARY KEY NOT NULL,
    key_id     TEXT NOT NULL REFERENCES translation_keys(id) ON DELETE CASCADE,
    locale_id  TEXT NOT NULL REFERENCES locales(id) ON DELETE CASCADE,
    value      TEXT NOT NULL,
    status     TEXT NOT NULL DEFAULT 'draft'
               CHECK (status IN ('draft', 'translated', 'reviewed')),
    updated_at TEXT NOT NULL,
    UNIQUE (key_id, locale_id)
);

CREATE INDEX idx_translation_values_locale ON translation_values(locale_id);

CREATE TABLE environments (
    id         TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    kind       TEXT NOT NULL CHECK (kind IN ('development', 'staging', 'production', 'custom')),
    created_at TEXT NOT NULL,
    UNIQUE (project_id, name)
);

CREATE INDEX idx_environments_project ON environments(project_id);

-- F1 never stores secrets. is_secret is pinned to 0 by a CHECK constraint so a
-- future migration can deliberately widen it after an encrypted vault exists.
CREATE TABLE config_variables (
    id             TEXT PRIMARY KEY NOT NULL,
    environment_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    key            TEXT NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
    value          TEXT NOT NULL,
    visibility     TEXT NOT NULL CHECK (visibility IN ('public', 'local')),
    is_secret      INTEGER NOT NULL DEFAULT 0 CHECK (is_secret = 0),
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    UNIQUE (environment_id, key)
);

CREATE INDEX idx_config_variables_environment ON config_variables(environment_id);

CREATE TABLE audit_events (
    id           TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id   TEXT REFERENCES projects(id) ON DELETE CASCADE,
    actor        TEXT NOT NULL,
    action       TEXT NOT NULL,
    entity_type  TEXT NOT NULL,
    entity_id    TEXT,
    payload      TEXT NOT NULL,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_audit_events_workspace_time ON audit_events(workspace_id, created_at);
CREATE INDEX idx_audit_events_project_time ON audit_events(project_id, created_at);

CREATE TABLE sync_documents (
    id           TEXT PRIMARY KEY NOT NULL,
    project_id   TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    document_key TEXT NOT NULL,
    version      INTEGER NOT NULL DEFAULT 0,
    snapshot     BLOB NOT NULL,
    pending      BLOB,
    updated_at   TEXT NOT NULL,
    UNIQUE (project_id, document_key)
);

CREATE INDEX idx_sync_documents_project ON sync_documents(project_id);

CREATE TABLE sync_checkpoints (
    id            TEXT PRIMARY KEY NOT NULL,
    document_id   TEXT NOT NULL REFERENCES sync_documents(id) ON DELETE CASCADE,
    peer_id       TEXT NOT NULL,
    state_vector  BLOB NOT NULL,
    updated_at    TEXT NOT NULL,
    UNIQUE (document_id, peer_id)
);

CREATE INDEX idx_sync_checkpoints_document ON sync_checkpoints(document_id);

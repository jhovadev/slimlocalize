use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use rusqlite::types::Type;
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use serde::Serialize;
use slimlocalize_core::ports::{ConfigStore, ProjectStore, Store, SyncStore, WorkspaceStore};
use slimlocalize_core::{
    AuditEvent, ConfigVariable, ConfigVisibility, DomainError, Environment, EnvironmentKind,
    ImportEntry, ImportSummary, KeySummary, Locale, Namespace, Project, Result,
    SaveTranslationInput, SetConfigVariableInput, SyncCheckpoint, SyncDocument, TranslationEntry,
    TranslationStatus, Workspace,
};
use uuid::Uuid;

use crate::migrations::{self, map_db};

/// SQLite-backed store.
///
/// The connection is wrapped in a mutex because rusqlite is synchronous. The
/// HTTP adapter dispatches blocking work through `spawn_blocking`; Tauri does
/// the same so the UI thread is never blocked by disk I/O.
pub struct SqliteStore {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStore")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub database_path: String,
    pub sqlite_version: String,
    pub schema_version: i64,
    pub journal_mode: String,
    pub file_size_bytes: u64,
    pub workspaces: i64,
    pub projects: i64,
    pub locales: i64,
    pub translation_keys: i64,
    pub translation_values: i64,
    pub environments: i64,
    pub config_variables: i64,
    pub audit_events: i64,
    pub sync_documents: i64,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if path.exists() && path.is_dir() {
            return Err(DomainError::validation(
                "database path",
                format!("{} is a directory", path.display()),
            ));
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|source| DomainError::Io {
                context: format!("cannot create database directory {}", parent.display()),
                source,
            })?;
        }
        let mut connection = Connection::open(&path).map_err(map_db("open database"))?;
        migrations::configure(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            path,
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut connection =
            Connection::open_in_memory().map_err(map_db("open memory database"))?;
        migrations::configure(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            path: PathBuf::from(":memory:"),
            connection: Mutex::new(connection),
        })
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn sqlite_version(&self) -> String {
        rusqlite::version().to_owned()
    }

    pub fn journal_mode(&self) -> Result<String> {
        let connection = self.lock()?;
        connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(map_db("read journal mode"))
    }

    pub fn stats(&self) -> Result<StorageStats> {
        let connection = self.lock()?;
        let schema_version: i64 = connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(map_db("read schema version"))?;
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .map_err(map_db("read journal mode"))?;
        let count = |table: &str| -> Result<i64> {
            // Table names are static literals controlled by this adapter.
            let sql = format!("SELECT COUNT(*) FROM {table}");
            connection
                .query_row(&sql, [], |row| row.get(0))
                .map_err(map_db("count rows"))
        };
        let file_size_bytes = std::fs::metadata(&self.path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        Ok(StorageStats {
            database_path: self.path.display().to_string(),
            sqlite_version: self.sqlite_version(),
            schema_version,
            journal_mode,
            file_size_bytes,
            workspaces: count("workspaces")?,
            projects: count("projects")?,
            locales: count("locales")?,
            translation_keys: count("translation_keys")?,
            translation_values: count("translation_values")?,
            environments: count("environments")?,
            config_variables: count("config_variables")?,
            audit_events: count("audit_events")?,
            sync_documents: count("sync_documents")?,
        })
    }

    /// Safe local backup using `VACUUM INTO`. Refuses to overwrite an existing
    /// file so an accidental command cannot destroy a previous backup.
    pub fn backup_to(&self, target: impl AsRef<Path>) -> Result<PathBuf> {
        let target = target.as_ref();
        if target.exists() {
            return Err(DomainError::conflict(
                "backup",
                "path",
                format!("refusing to overwrite {}", target.display()),
            ));
        }
        let parent = target.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|source| DomainError::Io {
            context: format!("cannot create backup directory {}", parent.display()),
            source,
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|source| DomainError::Io {
            context: format!("cannot resolve backup directory {}", parent.display()),
            source,
        })?;
        let file_name = target
            .file_name()
            .ok_or_else(|| DomainError::validation("backup path", "must include a file name"))?;
        let destination = canonical_parent.join(file_name);
        if destination == self.path {
            return Err(DomainError::validation(
                "backup path",
                "must differ from the live database path",
            ));
        }

        let connection = self.lock()?;
        let _ = connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        connection
            .execute(
                "VACUUM INTO ?1",
                params![destination.to_string_lossy().to_string()],
            )
            .map_err(map_db("create database backup"))?;
        Ok(destination)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| DomainError::internal("database mutex is poisoned"))
    }
}

impl WorkspaceStore for SqliteStore {
    fn insert_workspace(&self, workspace: &Workspace) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO workspaces (id, slug, name, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    workspace.id.to_string(),
                    workspace.slug,
                    workspace.name,
                    workspace.description,
                    workspace.created_at.to_rfc3339(),
                    workspace.updated_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("insert workspace"))?;
        Ok(())
    }

    fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, slug, name, description, created_at, updated_at
                 FROM workspaces ORDER BY name COLLATE NOCASE, slug",
            )
            .map_err(map_db("prepare workspace list"))?;
        let rows = statement
            .query_map([], row_to_workspace)
            .map_err(map_db("list workspaces"))?;
        collect_rows(rows, "read workspace")
    }

    fn find_workspace_by_id(&self, id: Uuid) -> Result<Option<Workspace>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, slug, name, description, created_at, updated_at
                 FROM workspaces WHERE id = ?1",
                params![id.to_string()],
                row_to_workspace,
            )
            .optional()
            .map_err(map_db("find workspace by id"))
    }

    fn find_workspace_by_slug(&self, slug: &str) -> Result<Option<Workspace>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, slug, name, description, created_at, updated_at
                 FROM workspaces WHERE slug = ?1",
                params![slug],
                row_to_workspace,
            )
            .optional()
            .map_err(map_db("find workspace by slug"))
    }

    fn insert_audit_event(&self, event: &AuditEvent) -> Result<()> {
        let payload =
            serde_json::to_string(&event.payload).map_err(|source| DomainError::Json {
                context: "audit payload".to_owned(),
                source,
            })?;
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO audit_events
                 (id, workspace_id, project_id, actor, action, entity_type, entity_id, payload, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    event.id.to_string(),
                    event.workspace_id.map(|id| id.to_string()),
                    event.project_id.map(|id| id.to_string()),
                    event.actor,
                    event.action,
                    event.entity_type,
                    event.entity_id.map(|id| id.to_string()),
                    payload,
                    event.created_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("insert audit event"))?;
        Ok(())
    }

    fn list_audit_events(&self, workspace_id: Uuid, limit: usize) -> Result<Vec<AuditEvent>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, workspace_id, project_id, actor, action, entity_type, entity_id,
                        payload, created_at
                 FROM audit_events WHERE workspace_id = ?1
                 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(map_db("prepare audit list"))?;
        let rows = statement
            .query_map(
                params![workspace_id.to_string(), limit as i64],
                row_to_audit,
            )
            .map_err(map_db("list audit events"))?;
        collect_rows(rows, "read audit event")
    }
}

impl ProjectStore for SqliteStore {
    fn insert_project_with_defaults(
        &self,
        project: &Project,
        default_locale: Option<&Locale>,
        environments: &[Environment],
    ) -> Result<()> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(map_db("begin project transaction"))?;
        let default_code = project
            .default_locale
            .clone()
            .or_else(|| default_locale.map(|locale| locale.code.clone()));
        tx.execute(
            "INSERT INTO projects
             (id, workspace_id, slug, name, description, default_locale, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                project.id.to_string(),
                project.workspace_id.to_string(),
                project.slug,
                project.name,
                project.description,
                default_code,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
            ],
        )
        .map_err(map_db("insert project"))?;

        let namespace = Namespace::new(
            Uuid::new_v4(),
            project.id,
            "common".to_owned(),
            project.created_at,
        );
        insert_namespace_tx(&tx, &namespace)?;

        if let Some(locale) = default_locale {
            insert_locale_tx(&tx, locale)?;
        }
        for environment in environments {
            insert_environment_tx(&tx, environment)?;
        }
        tx.commit().map_err(map_db("commit project transaction"))?;
        Ok(())
    }

    fn list_projects(&self, workspace_id: Uuid) -> Result<Vec<Project>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, workspace_id, slug, name, description, default_locale,
                        created_at, updated_at
                 FROM projects WHERE workspace_id = ?1
                 ORDER BY name COLLATE NOCASE, slug",
            )
            .map_err(map_db("prepare project list"))?;
        let rows = statement
            .query_map(params![workspace_id.to_string()], row_to_project)
            .map_err(map_db("list projects"))?;
        collect_rows(rows, "read project")
    }

    fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, workspace_id, slug, name, description, default_locale,
                        created_at, updated_at
                 FROM projects WHERE id = ?1",
                params![id.to_string()],
                row_to_project,
            )
            .optional()
            .map_err(map_db("find project by id"))
    }

    fn find_project_by_slug(&self, workspace_id: Uuid, slug: &str) -> Result<Option<Project>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, workspace_id, slug, name, description, default_locale,
                        created_at, updated_at
                 FROM projects WHERE workspace_id = ?1 AND slug = ?2",
                params![workspace_id.to_string(), slug],
                row_to_project,
            )
            .optional()
            .map_err(map_db("find project by slug"))
    }

    fn insert_locale(&self, locale: &Locale) -> Result<()> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(map_db("begin locale transaction"))?;
        insert_locale_tx(&tx, locale)?;
        tx.commit().map_err(map_db("commit locale transaction"))?;
        Ok(())
    }

    fn list_locales(&self, project_id: Uuid) -> Result<Vec<Locale>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, code, name, is_default, created_at
                 FROM locales WHERE project_id = ?1 ORDER BY code",
            )
            .map_err(map_db("prepare locale list"))?;
        let rows = statement
            .query_map(params![project_id.to_string()], row_to_locale)
            .map_err(map_db("list locales"))?;
        collect_rows(rows, "read locale")
    }

    fn find_locale_by_code(&self, project_id: Uuid, code: &str) -> Result<Option<Locale>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, code, name, is_default, created_at
                 FROM locales WHERE project_id = ?1 AND code = ?2",
                params![project_id.to_string(), code],
                row_to_locale,
            )
            .optional()
            .map_err(map_db("find locale by code"))
    }

    fn set_default_locale(&self, project_id: Uuid, locale_id: Uuid) -> Result<()> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(map_db("begin default locale transaction"))?;
        tx.execute(
            "UPDATE locales SET is_default = 0 WHERE project_id = ?1",
            params![project_id.to_string()],
        )
        .map_err(map_db("clear default locale"))?;
        let changed = tx
            .execute(
                "UPDATE locales SET is_default = 1 WHERE id = ?1 AND project_id = ?2",
                params![locale_id.to_string(), project_id.to_string()],
            )
            .map_err(map_db("set default locale"))?;
        if changed == 0 {
            return Err(DomainError::not_found("locale", locale_id.to_string()));
        }
        let code: String = tx
            .query_row(
                "SELECT code FROM locales WHERE id = ?1",
                params![locale_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db("read default locale code"))?;
        tx.execute(
            "UPDATE projects SET default_locale = ?1, updated_at = ?2 WHERE id = ?3",
            params![code, Utc::now().to_rfc3339(), project_id.to_string()],
        )
        .map_err(map_db("update project default locale"))?;
        tx.commit()
            .map_err(map_db("commit default locale transaction"))?;
        Ok(())
    }

    fn insert_namespace(&self, namespace: &Namespace) -> Result<()> {
        let connection = self.lock()?;
        insert_namespace_conn(&connection, namespace)
    }

    fn list_namespaces(&self, project_id: Uuid) -> Result<Vec<Namespace>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, name, created_at FROM namespaces
                 WHERE project_id = ?1 ORDER BY name",
            )
            .map_err(map_db("prepare namespace list"))?;
        let rows = statement
            .query_map(params![project_id.to_string()], row_to_namespace)
            .map_err(map_db("list namespaces"))?;
        collect_rows(rows, "read namespace")
    }

    fn find_namespace_by_name(&self, project_id: Uuid, name: &str) -> Result<Option<Namespace>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, name, created_at FROM namespaces
                 WHERE project_id = ?1 AND name = ?2",
                params![project_id.to_string(), name],
                row_to_namespace,
            )
            .optional()
            .map_err(map_db("find namespace"))
    }

    fn list_keys(&self, project_id: Uuid) -> Result<Vec<KeySummary>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT k.id, n.name, k.key, k.description
                 FROM translation_keys k
                 JOIN namespaces n ON n.id = k.namespace_id
                 WHERE k.project_id = ?1
                 ORDER BY n.name, k.key",
            )
            .map_err(map_db("prepare key list"))?;
        let rows = statement
            .query_map(params![project_id.to_string()], |row| {
                Ok(KeySummary {
                    id: get_uuid(row, 0)?,
                    namespace: row.get(1)?,
                    key: row.get(2)?,
                    description: row.get(3)?,
                })
            })
            .map_err(map_db("list keys"))?;
        collect_rows(rows, "read key")
    }

    fn count_keys(&self, project_id: Uuid) -> Result<usize> {
        let connection = self.lock()?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM translation_keys WHERE project_id = ?1",
                params![project_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db("count keys"))?;
        Ok(count as usize)
    }

    fn save_translation(&self, input: &SaveTranslationInput) -> Result<TranslationEntry> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(map_db("begin translation transaction"))?;
        let locale_id: Uuid = tx
            .query_row(
                "SELECT id FROM locales WHERE project_id = ?1 AND code = ?2",
                params![input.project_id.to_string(), input.locale_code],
                |row| get_uuid(row, 0),
            )
            .optional()
            .map_err(map_db("resolve translation locale"))?
            .ok_or_else(|| DomainError::not_found("locale", input.locale_code.clone()))?;

        let namespace_id = match tx
            .query_row(
                "SELECT id FROM namespaces WHERE project_id = ?1 AND name = ?2",
                params![input.project_id.to_string(), input.namespace],
                |row| get_uuid(row, 0),
            )
            .optional()
            .map_err(map_db("resolve namespace"))?
        {
            Some(id) => id,
            None => {
                let namespace = Namespace::new(
                    Uuid::new_v4(),
                    input.project_id,
                    input.namespace.clone(),
                    Utc::now(),
                );
                insert_namespace_tx(&tx, &namespace)?;
                namespace.id
            }
        };

        let existing = tx
            .query_row(
                "SELECT id, description FROM translation_keys
                 WHERE namespace_id = ?1 AND key = ?2",
                params![namespace_id.to_string(), input.key],
                |row| Ok((get_uuid(row, 0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(map_db("resolve translation key"))?;

        let (key_id, description) = if let Some((id, current_description)) = existing {
            let description = if input.description.is_some() {
                let next = input.description.clone();
                tx.execute(
                    "UPDATE translation_keys SET description = ?1, updated_at = ?2 WHERE id = ?3",
                    params![next, Utc::now().to_rfc3339(), id.to_string()],
                )
                .map_err(map_db("update key metadata"))?;
                next
            } else {
                current_description
            };
            (id, description)
        } else {
            let id = Uuid::new_v4();
            let at = Utc::now();
            tx.execute(
                "INSERT INTO translation_keys
                 (id, project_id, namespace_id, key, description, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id.to_string(),
                    input.project_id.to_string(),
                    namespace_id.to_string(),
                    input.key,
                    input.description,
                    at.to_rfc3339(),
                    at.to_rfc3339(),
                ],
            )
            .map_err(map_db("insert translation key"))?;
            (id, input.description.clone())
        };

        let at = Utc::now();
        tx.execute(
            "INSERT INTO translation_values (id, key_id, locale_id, value, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key_id, locale_id) DO UPDATE SET
                value = excluded.value,
                status = excluded.status,
                updated_at = excluded.updated_at",
            params![
                Uuid::new_v4().to_string(),
                key_id.to_string(),
                locale_id.to_string(),
                input.value,
                input.status.as_str(),
                at.to_rfc3339(),
            ],
        )
        .map_err(map_db("upsert translation value"))?;
        tx.execute(
            "UPDATE translation_keys SET updated_at = ?1 WHERE id = ?2",
            params![at.to_rfc3339(), key_id.to_string()],
        )
        .map_err(map_db("touch translation key"))?;

        let entry = tx
            .query_row(
                "SELECT k.id, k.key, n.name, k.description, l.code, v.value, v.status, v.updated_at
                 FROM translation_keys k
                 JOIN namespaces n ON n.id = k.namespace_id
                 JOIN locales l ON l.id = ?2
                 LEFT JOIN translation_values v ON v.key_id = k.id AND v.locale_id = l.id
                 WHERE k.id = ?1",
                params![key_id.to_string(), locale_id.to_string()],
                row_to_translation_entry,
            )
            .map_err(map_db("read saved translation"))?;
        let _ = description;
        tx.commit()
            .map_err(map_db("commit translation transaction"))?;
        Ok(entry)
    }

    fn list_translations(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<TranslationEntry>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT k.id, k.key, n.name, k.description, l.code, v.value, v.status, v.updated_at
                 FROM translation_keys k
                 JOIN namespaces n ON n.id = k.namespace_id
                 JOIN locales l ON l.project_id = k.project_id AND l.code = ?2
                 LEFT JOIN translation_values v ON v.key_id = k.id AND v.locale_id = l.id
                 WHERE k.project_id = ?1 AND (?3 IS NULL OR n.name = ?3)
                 ORDER BY n.name, k.key",
            )
            .map_err(map_db("prepare translation list"))?;
        let rows = statement
            .query_map(
                params![project_id.to_string(), locale_code, namespace],
                row_to_translation_entry,
            )
            .map_err(map_db("list translations"))?;
        collect_rows(rows, "read translation")
    }

    fn apply_import(
        &self,
        project_id: Uuid,
        namespace: &str,
        entries: &[ImportEntry],
        at: DateTime<Utc>,
    ) -> Result<ImportSummary> {
        let mut connection = self.lock()?;
        let tx = connection
            .transaction()
            .map_err(map_db("begin import transaction"))?;
        let namespace_id = match tx
            .query_row(
                "SELECT id FROM namespaces WHERE project_id = ?1 AND name = ?2",
                params![project_id.to_string(), namespace],
                |row| get_uuid(row, 0),
            )
            .optional()
            .map_err(map_db("resolve import namespace"))?
        {
            Some(id) => id,
            None => {
                let created = Namespace::new(Uuid::new_v4(), project_id, namespace.to_owned(), at);
                insert_namespace_tx(&tx, &created)?;
                created.id
            }
        };

        let mut locale_ids: HashMap<String, Uuid> = HashMap::new();
        let mut locales_created = 0usize;
        let mut seen_locales = HashSet::new();
        for entry in entries {
            if seen_locales.insert(entry.locale_code.clone()) {
                let existing = tx
                    .query_row(
                        "SELECT id FROM locales WHERE project_id = ?1 AND code = ?2",
                        params![project_id.to_string(), entry.locale_code],
                        |row| get_uuid(row, 0),
                    )
                    .optional()
                    .map_err(map_db("resolve import locale"))?;
                let id = if let Some(id) = existing {
                    id
                } else {
                    let locale = Locale::new(
                        Uuid::new_v4(),
                        project_id,
                        entry.locale_code.clone(),
                        entry.locale_code.clone(),
                        false,
                        at,
                    );
                    insert_locale_tx(&tx, &locale)?;
                    locales_created += 1;
                    locale.id
                };
                locale_ids.insert(entry.locale_code.clone(), id);
            }
        }

        let mut keys_created = 0usize;
        let mut values_written = 0usize;
        for entry in entries {
            let key_id = tx
                .query_row(
                    "SELECT id FROM translation_keys WHERE namespace_id = ?1 AND key = ?2",
                    params![namespace_id.to_string(), entry.key],
                    |row| get_uuid(row, 0),
                )
                .optional()
                .map_err(map_db("resolve import key"))?;
            let key_id = if let Some(id) = key_id {
                id
            } else {
                let id = Uuid::new_v4();
                tx.execute(
                    "INSERT INTO translation_keys
                     (id, project_id, namespace_id, key, description, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
                    params![
                        id.to_string(),
                        project_id.to_string(),
                        namespace_id.to_string(),
                        entry.key,
                        at.to_rfc3339(),
                        at.to_rfc3339(),
                    ],
                )
                .map_err(map_db("insert imported key"))?;
                keys_created += 1;
                id
            };
            let locale_id = locale_ids
                .get(&entry.locale_code)
                .copied()
                .ok_or_else(|| DomainError::internal("import locale was not resolved"))?;
            tx.execute(
                "INSERT INTO translation_values (id, key_id, locale_id, value, status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'translated', ?5)
                 ON CONFLICT(key_id, locale_id) DO UPDATE SET
                    value = excluded.value,
                    status = excluded.status,
                    updated_at = excluded.updated_at",
                params![
                    Uuid::new_v4().to_string(),
                    key_id.to_string(),
                    locale_id.to_string(),
                    entry.value,
                    at.to_rfc3339(),
                ],
            )
            .map_err(map_db("upsert imported value"))?;
            values_written += 1;
        }

        tx.commit().map_err(map_db("commit import transaction"))?;
        Ok(ImportSummary {
            keys_created,
            values_written,
            locales_created,
        })
    }

    fn find_translation(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: &str,
        key: &str,
    ) -> Result<Option<(String, TranslationStatus)>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT v.value, v.status
                 FROM translation_values v
                 JOIN translation_keys k ON k.id = v.key_id
                 JOIN namespaces n ON n.id = k.namespace_id
                 JOIN locales l ON l.id = v.locale_id
                 WHERE k.project_id = ?1 AND l.code = ?2 AND n.name = ?3 AND k.key = ?4",
                params![project_id.to_string(), locale_code, namespace, key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        parse_status(&row.get::<_, String>(1)?, 1)?,
                    ))
                },
            )
            .optional()
            .map_err(map_db("find translation"))
    }
}

impl ConfigStore for SqliteStore {
    fn insert_environment(&self, environment: &Environment) -> Result<()> {
        let connection = self.lock()?;
        insert_environment_conn(&connection, environment)
    }

    fn list_environments(&self, project_id: Uuid) -> Result<Vec<Environment>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, name, kind, created_at FROM environments
                 WHERE project_id = ?1 ORDER BY name",
            )
            .map_err(map_db("prepare environment list"))?;
        let rows = statement
            .query_map(params![project_id.to_string()], row_to_environment)
            .map_err(map_db("list environments"))?;
        collect_rows(rows, "read environment")
    }

    fn find_environment_by_id(&self, id: Uuid) -> Result<Option<Environment>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, name, kind, created_at FROM environments WHERE id = ?1",
                params![id.to_string()],
                row_to_environment,
            )
            .optional()
            .map_err(map_db("find environment by id"))
    }

    fn find_environment_by_name(
        &self,
        project_id: Uuid,
        name: &str,
    ) -> Result<Option<Environment>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, name, kind, created_at FROM environments
                 WHERE project_id = ?1 AND name = ?2",
                params![project_id.to_string(), name],
                row_to_environment,
            )
            .optional()
            .map_err(map_db("find environment by name"))
    }

    fn upsert_config_variable(
        &self,
        input: &SetConfigVariableInput,
        at: DateTime<Utc>,
    ) -> Result<ConfigVariable> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO config_variables
                 (id, environment_id, key, value, visibility, is_secret, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)
                 ON CONFLICT(environment_id, key) DO UPDATE SET
                    value = excluded.value,
                    visibility = excluded.visibility,
                    updated_at = excluded.updated_at",
                params![
                    Uuid::new_v4().to_string(),
                    input.environment_id.to_string(),
                    input.key,
                    input.value,
                    input.visibility.as_str(),
                    at.to_rfc3339(),
                    at.to_rfc3339(),
                ],
            )
            .map_err(map_db("upsert config variable"))?;
        connection
            .query_row(
                "SELECT id, environment_id, key, value, visibility, created_at, updated_at
                 FROM config_variables WHERE environment_id = ?1 AND key = ?2",
                params![input.environment_id.to_string(), input.key],
                row_to_config_variable,
            )
            .map_err(map_db("read config variable"))
    }

    fn list_config_variables(&self, environment_id: Uuid) -> Result<Vec<ConfigVariable>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, environment_id, key, value, visibility, created_at, updated_at
                 FROM config_variables WHERE environment_id = ?1 ORDER BY key",
            )
            .map_err(map_db("prepare config list"))?;
        let rows = statement
            .query_map(params![environment_id.to_string()], row_to_config_variable)
            .map_err(map_db("list config variables"))?;
        collect_rows(rows, "read config variable")
    }
}

impl SyncStore for SqliteStore {
    fn upsert_sync_document(&self, document: &SyncDocument) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO sync_documents
                 (id, project_id, document_key, version, snapshot, pending, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(project_id, document_key) DO UPDATE SET
                    version = excluded.version,
                    snapshot = excluded.snapshot,
                    pending = excluded.pending,
                    updated_at = excluded.updated_at",
                params![
                    document.id.to_string(),
                    document.project_id.to_string(),
                    document.document_key,
                    document.version as i64,
                    document.snapshot,
                    document.pending,
                    document.updated_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("upsert sync document"))?;
        Ok(())
    }

    fn find_sync_document(
        &self,
        project_id: Uuid,
        document_key: &str,
    ) -> Result<Option<SyncDocument>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT id, project_id, document_key, version, snapshot, pending, updated_at
                 FROM sync_documents WHERE project_id = ?1 AND document_key = ?2",
                params![project_id.to_string(), document_key],
                row_to_sync_document,
            )
            .optional()
            .map_err(map_db("find sync document"))
    }

    fn list_sync_documents(&self, project_id: Uuid) -> Result<Vec<SyncDocument>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, project_id, document_key, version, snapshot, pending, updated_at
                 FROM sync_documents WHERE project_id = ?1 ORDER BY document_key",
            )
            .map_err(map_db("prepare sync document list"))?;
        let rows = statement
            .query_map(params![project_id.to_string()], row_to_sync_document)
            .map_err(map_db("list sync documents"))?;
        collect_rows(rows, "read sync document")
    }

    fn upsert_sync_checkpoint(&self, checkpoint: &SyncCheckpoint) -> Result<()> {
        let connection = self.lock()?;
        connection
            .execute(
                "INSERT INTO sync_checkpoints
                 (id, document_id, peer_id, state_vector, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(document_id, peer_id) DO UPDATE SET
                    state_vector = excluded.state_vector,
                    updated_at = excluded.updated_at",
                params![
                    checkpoint.id.to_string(),
                    checkpoint.document_id.to_string(),
                    checkpoint.peer_id,
                    checkpoint.state_vector,
                    checkpoint.updated_at.to_rfc3339(),
                ],
            )
            .map_err(map_db("upsert sync checkpoint"))?;
        Ok(())
    }

    fn list_sync_checkpoints(&self, document_id: Uuid) -> Result<Vec<SyncCheckpoint>> {
        let connection = self.lock()?;
        let mut statement = connection
            .prepare(
                "SELECT id, document_id, peer_id, state_vector, updated_at
                 FROM sync_checkpoints WHERE document_id = ?1 ORDER BY peer_id",
            )
            .map_err(map_db("prepare checkpoint list"))?;
        let rows = statement
            .query_map(params![document_id.to_string()], row_to_sync_checkpoint)
            .map_err(map_db("list sync checkpoints"))?;
        collect_rows(rows, "read sync checkpoint")
    }
}

impl Store for SqliteStore {
    fn health_check(&self) -> Result<()> {
        let connection = self.lock()?;
        let _: i64 = connection
            .query_row("SELECT 1", [], |row| row.get(0))
            .map_err(map_db("database health check"))?;
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .map_err(map_db("read foreign key mode"))?;
        if foreign_keys != 1 {
            return Err(DomainError::storage("foreign keys are disabled"));
        }
        let quick: String = connection
            .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
            .map_err(map_db("quick check"))?;
        if quick != "ok" {
            return Err(DomainError::storage(format!("SQLite quick_check: {quick}")));
        }
        Ok(())
    }

    fn schema_version(&self) -> Result<i64> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(map_db("read schema version"))
    }
}

// -------------------------------------------------------------------------
// Transaction helpers
// -------------------------------------------------------------------------

fn insert_namespace_conn(connection: &Connection, namespace: &Namespace) -> Result<()> {
    connection
        .execute(
            "INSERT INTO namespaces (id, project_id, name, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                namespace.id.to_string(),
                namespace.project_id.to_string(),
                namespace.name,
                namespace.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db("insert namespace"))?;
    Ok(())
}

fn insert_namespace_tx(tx: &Transaction<'_>, namespace: &Namespace) -> Result<()> {
    insert_namespace_conn(tx, namespace)
}

fn insert_environment_conn(connection: &Connection, environment: &Environment) -> Result<()> {
    connection
        .execute(
            "INSERT INTO environments (id, project_id, name, kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                environment.id.to_string(),
                environment.project_id.to_string(),
                environment.name,
                environment.kind.as_str(),
                environment.created_at.to_rfc3339(),
            ],
        )
        .map_err(map_db("insert environment"))?;
    Ok(())
}

fn insert_environment_tx(tx: &Transaction<'_>, environment: &Environment) -> Result<()> {
    insert_environment_conn(tx, environment)
}

fn insert_locale_tx(tx: &Transaction<'_>, locale: &Locale) -> Result<()> {
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM locales WHERE project_id = ?1",
            params![locale.project_id.to_string()],
            |row| row.get(0),
        )
        .map_err(map_db("count locales"))?;
    let is_default = locale.is_default || count == 0;
    if is_default {
        tx.execute(
            "UPDATE locales SET is_default = 0 WHERE project_id = ?1",
            params![locale.project_id.to_string()],
        )
        .map_err(map_db("clear default locale"))?;
    }
    tx.execute(
        "INSERT INTO locales (id, project_id, code, name, is_default, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            locale.id.to_string(),
            locale.project_id.to_string(),
            locale.code,
            locale.name,
            if is_default { 1_i64 } else { 0_i64 },
            locale.created_at.to_rfc3339(),
        ],
    )
    .map_err(map_db("insert locale"))?;
    if is_default {
        tx.execute(
            "UPDATE projects SET default_locale = ?1, updated_at = ?2 WHERE id = ?3",
            params![
                locale.code,
                Utc::now().to_rfc3339(),
                locale.project_id.to_string()
            ],
        )
        .map_err(map_db("update project default locale"))?;
    }
    Ok(())
}

// -------------------------------------------------------------------------
// Row mapping
// -------------------------------------------------------------------------

fn collect_rows<T, F>(rows: rusqlite::MappedRows<'_, F>, context: &'static str) -> Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(map_db(context))?);
    }
    Ok(values)
}

fn get_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let value: String = row.get(index)?;
    Uuid::parse_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
    })
}

fn get_datetime(row: &Row<'_>, index: usize) -> rusqlite::Result<DateTime<Utc>> {
    let value: String = row.get(index)?;
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
}

fn get_opt_datetime(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let value: Option<String> = row.get(index)?;
    let Some(value) = value else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(&value)
        .map(|value| Some(value.with_timezone(&Utc)))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
        })
}

fn parse_status(value: &str, index: usize) -> rusqlite::Result<TranslationStatus> {
    match value {
        "draft" => Ok(TranslationStatus::Draft),
        "translated" => Ok(TranslationStatus::Translated),
        "reviewed" => Ok(TranslationStatus::Reviewed),
        other => Err(invalid_value(
            index,
            format!("invalid translation status {other}"),
        )),
    }
}

fn parse_environment_kind(value: &str, index: usize) -> rusqlite::Result<EnvironmentKind> {
    match value {
        "development" => Ok(EnvironmentKind::Development),
        "staging" => Ok(EnvironmentKind::Staging),
        "production" => Ok(EnvironmentKind::Production),
        "custom" => Ok(EnvironmentKind::Custom),
        other => Err(invalid_value(
            index,
            format!("invalid environment kind {other}"),
        )),
    }
}

fn parse_visibility(value: &str, index: usize) -> rusqlite::Result<ConfigVisibility> {
    match value {
        "public" => Ok(ConfigVisibility::Public),
        "local" => Ok(ConfigVisibility::Local),
        other => Err(invalid_value(index, format!("invalid visibility {other}"))),
    }
}

fn invalid_value(index: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn row_to_workspace(row: &Row<'_>) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: get_uuid(row, 0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        created_at: get_datetime(row, 4)?,
        updated_at: get_datetime(row, 5)?,
    })
}

fn row_to_project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: get_uuid(row, 0)?,
        workspace_id: get_uuid(row, 1)?,
        slug: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        default_locale: row.get(5)?,
        created_at: get_datetime(row, 6)?,
        updated_at: get_datetime(row, 7)?,
    })
}

fn row_to_locale(row: &Row<'_>) -> rusqlite::Result<Locale> {
    Ok(Locale {
        id: get_uuid(row, 0)?,
        project_id: get_uuid(row, 1)?,
        code: row.get(2)?,
        name: row.get(3)?,
        is_default: row.get::<_, i64>(4)? == 1,
        created_at: get_datetime(row, 5)?,
    })
}

fn row_to_namespace(row: &Row<'_>) -> rusqlite::Result<Namespace> {
    Ok(Namespace {
        id: get_uuid(row, 0)?,
        project_id: get_uuid(row, 1)?,
        name: row.get(2)?,
        created_at: get_datetime(row, 3)?,
    })
}

fn row_to_environment(row: &Row<'_>) -> rusqlite::Result<Environment> {
    let kind: String = row.get(3)?;
    Ok(Environment {
        id: get_uuid(row, 0)?,
        project_id: get_uuid(row, 1)?,
        name: row.get(2)?,
        kind: parse_environment_kind(&kind, 3)?,
        created_at: get_datetime(row, 4)?,
    })
}

fn row_to_config_variable(row: &Row<'_>) -> rusqlite::Result<ConfigVariable> {
    let visibility: String = row.get(4)?;
    Ok(ConfigVariable {
        id: get_uuid(row, 0)?,
        environment_id: get_uuid(row, 1)?,
        key: row.get(2)?,
        value: row.get(3)?,
        visibility: parse_visibility(&visibility, 4)?,
        created_at: get_datetime(row, 5)?,
        updated_at: get_datetime(row, 6)?,
    })
}

fn row_to_translation_entry(row: &Row<'_>) -> rusqlite::Result<TranslationEntry> {
    let status: Option<String> = row.get(6)?;
    let status = match status {
        Some(status) => Some(parse_status(&status, 6)?),
        None => None,
    };
    Ok(TranslationEntry {
        key_id: get_uuid(row, 0)?,
        key: row.get(1)?,
        namespace: row.get(2)?,
        description: row.get(3)?,
        locale_code: row.get(4)?,
        value: row.get(5)?,
        status,
        updated_at: get_opt_datetime(row, 7)?,
    })
}

fn row_to_audit(row: &Row<'_>) -> rusqlite::Result<AuditEvent> {
    let payload: String = row.get(7)?;
    let payload: serde_json::Value = serde_json::from_str(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(error))
    })?;
    Ok(AuditEvent {
        id: get_uuid(row, 0)?,
        workspace_id: row
            .get::<_, Option<String>>(1)?
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(1, Type::Text, Box::new(error))
            })?,
        project_id: row
            .get::<_, Option<String>>(2)?
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(2, Type::Text, Box::new(error))
            })?,
        actor: row.get(3)?,
        action: row.get(4)?,
        entity_type: row.get(5)?,
        entity_id: row
            .get::<_, Option<String>>(6)?
            .map(|value| Uuid::parse_str(&value))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(6, Type::Text, Box::new(error))
            })?,
        payload,
        created_at: get_datetime(row, 8)?,
    })
}

fn row_to_sync_document(row: &Row<'_>) -> rusqlite::Result<SyncDocument> {
    Ok(SyncDocument {
        id: get_uuid(row, 0)?,
        project_id: get_uuid(row, 1)?,
        document_key: row.get(2)?,
        version: row.get::<_, i64>(3)? as u64,
        snapshot: row.get(4)?,
        pending: row.get(5)?,
        updated_at: get_datetime(row, 6)?,
    })
}

fn row_to_sync_checkpoint(row: &Row<'_>) -> rusqlite::Result<SyncCheckpoint> {
    Ok(SyncCheckpoint {
        id: get_uuid(row, 0)?,
        document_id: get_uuid(row, 1)?,
        peer_id: row.get(2)?,
        state_vector: row.get(3)?,
        updated_at: get_datetime(row, 4)?,
    })
}

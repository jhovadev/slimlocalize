use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::Result;
use crate::model::{
    AuditEvent, ConfigVariable, Environment, ImportEntry, ImportSummary, KeySummary, Locale,
    Namespace, Project, SaveTranslationInput, SetConfigVariableInput, SyncCheckpoint, SyncDocument,
    TranslationEntry, TranslationStatus, Workspace,
};

pub trait WorkspaceStore: Send + Sync {
    fn insert_workspace(&self, workspace: &Workspace) -> Result<()>;
    fn list_workspaces(&self) -> Result<Vec<Workspace>>;
    fn find_workspace_by_id(&self, id: Uuid) -> Result<Option<Workspace>>;
    fn find_workspace_by_slug(&self, slug: &str) -> Result<Option<Workspace>>;
    fn insert_audit_event(&self, event: &AuditEvent) -> Result<()>;
    fn list_audit_events(&self, workspace_id: Uuid, limit: usize) -> Result<Vec<AuditEvent>>;
}

pub trait ProjectStore: Send + Sync {
    /// Insert a project, its default namespace, optional first locale and the
    /// provided environments in a single transaction.
    fn insert_project_with_defaults(
        &self,
        project: &Project,
        default_locale: Option<&Locale>,
        environments: &[Environment],
    ) -> Result<()>;

    fn list_projects(&self, workspace_id: Uuid) -> Result<Vec<Project>>;
    fn find_project_by_id(&self, id: Uuid) -> Result<Option<Project>>;
    fn find_project_by_slug(&self, workspace_id: Uuid, slug: &str) -> Result<Option<Project>>;

    fn insert_locale(&self, locale: &Locale) -> Result<()>;
    fn list_locales(&self, project_id: Uuid) -> Result<Vec<Locale>>;
    fn find_locale_by_code(&self, project_id: Uuid, code: &str) -> Result<Option<Locale>>;
    fn set_default_locale(&self, project_id: Uuid, locale_id: Uuid) -> Result<()>;

    fn insert_namespace(&self, namespace: &Namespace) -> Result<()>;
    fn list_namespaces(&self, project_id: Uuid) -> Result<Vec<Namespace>>;
    fn find_namespace_by_name(&self, project_id: Uuid, name: &str) -> Result<Option<Namespace>>;

    fn list_keys(&self, project_id: Uuid) -> Result<Vec<KeySummary>>;
    fn count_keys(&self, project_id: Uuid) -> Result<usize>;

    /// Create the namespace/key if needed and upsert the locale value in one
    /// transaction. Returns the projected entry.
    fn save_translation(&self, input: &SaveTranslationInput) -> Result<TranslationEntry>;

    fn list_translations(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<TranslationEntry>>;

    fn apply_import(
        &self,
        project_id: Uuid,
        namespace: &str,
        entries: &[ImportEntry],
        at: DateTime<Utc>,
    ) -> Result<ImportSummary>;

    /// Convenience helper used by check/export to fetch the status of a value.
    fn find_translation(
        &self,
        project_id: Uuid,
        locale_code: &str,
        namespace: &str,
        key: &str,
    ) -> Result<Option<(String, TranslationStatus)>>;
}

pub trait ConfigStore: Send + Sync {
    fn insert_environment(&self, environment: &Environment) -> Result<()>;
    fn list_environments(&self, project_id: Uuid) -> Result<Vec<Environment>>;
    fn find_environment_by_id(&self, id: Uuid) -> Result<Option<Environment>>;
    fn find_environment_by_name(&self, project_id: Uuid, name: &str)
    -> Result<Option<Environment>>;
    fn upsert_config_variable(
        &self,
        input: &SetConfigVariableInput,
        at: DateTime<Utc>,
    ) -> Result<ConfigVariable>;
    fn list_config_variables(&self, environment_id: Uuid) -> Result<Vec<ConfigVariable>>;
}

pub trait SyncStore: Send + Sync {
    fn upsert_sync_document(&self, document: &SyncDocument) -> Result<()>;
    fn find_sync_document(
        &self,
        project_id: Uuid,
        document_key: &str,
    ) -> Result<Option<SyncDocument>>;
    fn list_sync_documents(&self, project_id: Uuid) -> Result<Vec<SyncDocument>>;
    fn upsert_sync_checkpoint(&self, checkpoint: &SyncCheckpoint) -> Result<()>;
    fn list_sync_checkpoints(&self, document_id: Uuid) -> Result<Vec<SyncCheckpoint>>;
}

/// Aggregate port consumed by the application service. Implemented by the
/// SQLite adapter; future Turso/libSQL adapters can provide the same contract.
pub trait Store: WorkspaceStore + ProjectStore + ConfigStore + SyncStore + Send + Sync {
    fn health_check(&self) -> Result<()>;
    fn schema_version(&self) -> Result<i64>;
}

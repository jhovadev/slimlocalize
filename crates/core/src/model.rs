use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier type used across every entity.
pub type EntityId = Uuid;

fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: EntityId,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceInput {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Owner,
    Maintainer,
    Translator,
    Viewer,
}

impl MemberRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Maintainer => "maintainer",
            Self::Translator => "translator",
            Self::Viewer => "viewer",
        }
    }
}

/// Membership metadata prepared for future authentication. F1 does not enforce
/// authorization; the local user owns every workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMember {
    pub id: EntityId,
    pub workspace_id: EntityId,
    pub subject: String,
    pub role: MemberRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: EntityId,
    pub workspace_id: EntityId,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub default_locale: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    /// Optional first locale, for example `en-US`. When provided it becomes the
    /// default locale of the project.
    pub default_locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Locale {
    pub id: EntityId,
    pub project_id: EntityId,
    pub code: String,
    pub name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLocaleInput {
    pub code: String,
    pub name: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub id: EntityId,
    pub project_id: EntityId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNamespaceInput {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationKey {
    pub id: EntityId,
    pub project_id: EntityId,
    pub namespace_id: EntityId,
    pub key: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Key metadata without a locale projection, used by check/export and MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySummary {
    pub id: EntityId,
    pub namespace: String,
    pub key: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationValue {
    pub id: EntityId,
    pub key_id: EntityId,
    pub locale_id: EntityId,
    pub value: String,
    pub status: TranslationStatus,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    #[default]
    Draft,
    Translated,
    Reviewed,
}

impl TranslationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Translated => "translated",
            Self::Reviewed => "reviewed",
        }
    }

    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "draft" => Ok(Self::Draft),
            "translated" => Ok(Self::Translated),
            "reviewed" => Ok(Self::Reviewed),
            other => Err(crate::DomainError::validation(
                "status",
                format!("expected draft, translated or reviewed, got {other}"),
            )),
        }
    }
}

/// A translation row projected for a specific locale. Missing values are
/// represented with `value = None` instead of being dropped, so the UI and the
/// checker can render gaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationEntry {
    pub key_id: EntityId,
    pub key: String,
    pub namespace: String,
    pub description: Option<String>,
    pub locale_code: String,
    pub value: Option<String>,
    pub status: Option<TranslationStatus>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTranslationInput {
    pub project_id: EntityId,
    pub namespace: String,
    pub key: String,
    pub locale_code: String,
    pub value: String,
    #[serde(default)]
    pub status: TranslationStatus,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentKind {
    Development,
    Staging,
    Production,
    Custom,
}

impl EnvironmentKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
            Self::Custom => "custom",
        }
    }

    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "development" => Ok(Self::Development),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            "custom" => Ok(Self::Custom),
            other => Err(crate::DomainError::validation(
                "kind",
                format!("expected development, staging, production or custom, got {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: EntityId,
    pub project_id: EntityId,
    pub name: String,
    pub kind: EnvironmentKind,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentInput {
    pub name: String,
    pub kind: EnvironmentKind,
}

/// Visibility of a configuration variable.
///
/// F1 stores public and local (non-shared) values. `Secret` is intentionally
/// rejected until the encrypted vault exists; see `SECURITY.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigVisibility {
    Public,
    Local,
}

impl ConfigVisibility {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Local => "local",
        }
    }

    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "public" => Ok(Self::Public),
            "local" => Ok(Self::Local),
            "secret" => Err(crate::DomainError::Unsupported {
                feature: "secret configuration variables (encrypted vault is not implemented)"
                    .into(),
            }),
            other => Err(crate::DomainError::validation(
                "visibility",
                format!("expected public or local, got {other}"),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigVariable {
    pub id: EntityId,
    pub environment_id: EntityId,
    pub key: String,
    pub value: String,
    pub visibility: ConfigVisibility,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConfigVariableInput {
    pub environment_id: EntityId,
    pub key: String,
    pub value: String,
    pub visibility: ConfigVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: EntityId,
    pub workspace_id: Option<EntityId>,
    pub project_id: Option<EntityId>,
    pub actor: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<EntityId>,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDocument {
    pub id: EntityId,
    pub project_id: EntityId,
    pub document_key: String,
    pub version: u64,
    pub snapshot: Vec<u8>,
    pub pending: Option<Vec<u8>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncCheckpoint {
    pub id: EntityId,
    pub document_id: EntityId,
    pub peer_id: String,
    pub state_vector: Vec<u8>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntry {
    pub locale_code: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub keys_created: usize,
    pub values_written: usize,
    pub locales_created: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOptions {
    pub path: PathBuf,
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

fn default_namespace() -> String {
    "common".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub path: PathBuf,
    pub namespace: String,
    pub files_processed: usize,
    pub locales_created: usize,
    pub keys_created: usize,
    pub values_written: usize,
    pub skipped_files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    #[default]
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub output_dir: PathBuf,
    #[serde(default)]
    pub format: ExportFormat,
    /// When true, dot-separated keys are expanded into nested JSON objects.
    #[serde(default)]
    pub nested: bool,
    /// Optional namespace filter. Without a filter, colliding keys from
    /// different namespaces are rejected instead of silently overwritten.
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFile {
    pub locale_code: String,
    pub relative_path: String,
    pub key_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub project_id: EntityId,
    pub output_dir: PathBuf,
    pub files: Vec<ExportFile>,
    pub total_keys: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingTranslation {
    pub locale_code: String,
    pub namespace: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceholderMismatch {
    pub locale_code: String,
    pub source_locale: String,
    pub namespace: String,
    pub key: String,
    pub expected: Vec<String>,
    pub actual: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    pub project_id: EntityId,
    pub default_locale: Option<String>,
    pub locales: Vec<String>,
    pub total_keys: usize,
    pub checked_values: usize,
    pub missing: Vec<MissingTranslation>,
    pub placeholder_mismatches: Vec<PlaceholderMismatch>,
}

impl CheckReport {
    #[must_use]
    pub fn has_issues(&self) -> bool {
        !self.missing.is_empty() || !self.placeholder_mismatches.is_empty()
    }
}

/// Snapshot of a project used by read-only adapters (server and MCP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub project: Project,
    pub locales: Vec<Locale>,
    pub namespaces: Vec<Namespace>,
    pub environments: Vec<Environment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub platform: String,
    pub storage: String,
    pub database_path: String,
    pub local_first: bool,
}

impl Workspace {
    #[must_use]
    pub fn new(
        id: EntityId,
        slug: String,
        name: String,
        description: Option<String>,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            slug,
            name,
            description,
            created_at: at,
            updated_at: at,
        }
    }
}

impl Project {
    #[must_use]
    pub fn new(
        id: EntityId,
        workspace_id: EntityId,
        slug: String,
        name: String,
        description: Option<String>,
        default_locale: Option<String>,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            workspace_id,
            slug,
            name,
            description,
            default_locale,
            created_at: at,
            updated_at: at,
        }
    }
}

impl Locale {
    #[must_use]
    pub fn new(
        id: EntityId,
        project_id: EntityId,
        code: String,
        name: String,
        is_default: bool,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            project_id,
            code,
            name,
            is_default,
            created_at: at,
        }
    }
}

impl Namespace {
    #[must_use]
    pub fn new(id: EntityId, project_id: EntityId, name: String, at: DateTime<Utc>) -> Self {
        Self {
            id,
            project_id,
            name,
            created_at: at,
        }
    }
}

impl Environment {
    #[must_use]
    pub fn new(
        id: EntityId,
        project_id: EntityId,
        name: String,
        kind: EnvironmentKind,
        at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            project_id,
            name,
            kind,
            created_at: at,
        }
    }
}

impl AuditEvent {
    #[must_use]
    pub fn new(
        workspace_id: Option<EntityId>,
        project_id: Option<EntityId>,
        action: impl Into<String>,
        entity_type: impl Into<String>,
        entity_id: Option<EntityId>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            project_id,
            actor: "local-user".to_owned(),
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id,
            payload,
            created_at: now(),
        }
    }
}

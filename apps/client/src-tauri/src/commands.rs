use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use slimlocalize_core::{
    AddLocaleInput, AppInfo, CheckReport, ConfigVariable, CreateEnvironmentInput,
    CreateNamespaceInput, CreateProjectInput, CreateWorkspaceInput, DomainError, Environment,
    ExportOptions, ExportReport, ImportOptions, ImportReport, Locale, LocalizeService, Namespace,
    Project, SaveTranslationInput, SetConfigVariableInput, TranslationEntry, Workspace,
};
use slimlocalize_collaboration::{CollaborationService, CollabStatus, SyncEnvelope};
use slimlocalize_storage::SqliteStore;
use tauri::State;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal_error".to_owned(),
            message: message.into(),
        }
    }
}

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}

async fn run<T, F>(state: &AppState, action: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&LocalizeService<SqliteStore>) -> slimlocalize_core::Result<T> + Send + 'static,
{
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || action(&service))
        .await
        .map_err(|error| ApiError::internal(format!("blocking task failed: {error}")))?
        .map_err(ApiError::from)
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    state.service.app_info(std::env::consts::OS, &state.db_path)
}

#[tauri::command]
pub async fn list_workspaces(state: State<'_, AppState>) -> Result<Vec<Workspace>, ApiError> {
    run(&state, |service| service.list_workspaces()).await
}

#[tauri::command]
pub async fn create_workspace(
    state: State<'_, AppState>,
    input: CreateWorkspaceInput,
) -> Result<Workspace, ApiError> {
    run(&state, move |service| service.create_workspace(input)).await
}

#[tauri::command]
pub async fn list_projects(
    state: State<'_, AppState>,
    workspace_id: Uuid,
) -> Result<Vec<Project>, ApiError> {
    run(&state, move |service| service.list_projects(workspace_id)).await
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    workspace_id: Uuid,
    input: CreateProjectInput,
) -> Result<Project, ApiError> {
    run(&state, move |service| {
        service.create_project(workspace_id, input)
    })
    .await
}

#[tauri::command]
pub async fn list_locales(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<Vec<Locale>, ApiError> {
    run(&state, move |service| service.list_locales(project_id)).await
}

#[tauri::command]
pub async fn add_locale(
    state: State<'_, AppState>,
    project_id: Uuid,
    input: AddLocaleInput,
) -> Result<Locale, ApiError> {
    run(&state, move |service| service.add_locale(project_id, input)).await
}

#[tauri::command]
pub async fn set_default_locale(
    state: State<'_, AppState>,
    project_id: Uuid,
    code: String,
) -> Result<Locale, ApiError> {
    run(&state, move |service| {
        service.set_default_locale(project_id, &code)
    })
    .await
}

#[tauri::command]
pub async fn list_namespaces(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<Vec<Namespace>, ApiError> {
    run(&state, move |service| service.list_namespaces(project_id)).await
}

#[tauri::command]
pub async fn create_namespace(
    state: State<'_, AppState>,
    project_id: Uuid,
    name: String,
) -> Result<Namespace, ApiError> {
    run(&state, move |service| {
        service.create_namespace(project_id, CreateNamespaceInput { name })
    })
    .await
}

#[tauri::command]
pub async fn list_translations(
    state: State<'_, AppState>,
    project_id: Uuid,
    locale_code: String,
    namespace: Option<String>,
) -> Result<Vec<TranslationEntry>, ApiError> {
    run(&state, move |service| {
        service.list_translations(project_id, &locale_code, namespace.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn save_translation(
    state: State<'_, AppState>,
    input: SaveTranslationInput,
) -> Result<TranslationEntry, ApiError> {
    run(&state, move |service| service.save_translation(input)).await
}

#[tauri::command]
pub async fn list_environments(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<Vec<Environment>, ApiError> {
    run(&state, move |service| service.list_environments(project_id)).await
}

#[tauri::command]
pub async fn create_environment(
    state: State<'_, AppState>,
    project_id: Uuid,
    input: CreateEnvironmentInput,
) -> Result<Environment, ApiError> {
    run(&state, move |service| {
        service.create_environment(project_id, input)
    })
    .await
}

#[tauri::command]
pub async fn list_config_variables(
    state: State<'_, AppState>,
    environment_id: Uuid,
) -> Result<Vec<ConfigVariable>, ApiError> {
    run(&state, move |service| {
        service.list_config_variables(environment_id)
    })
    .await
}

#[tauri::command]
pub async fn set_config_variable(
    state: State<'_, AppState>,
    input: SetConfigVariableInput,
) -> Result<ConfigVariable, ApiError> {
    run(&state, move |service| service.set_config_variable(input)).await
}

#[tauri::command]
pub async fn check_project(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<CheckReport, ApiError> {
    run(&state, move |service| service.check_project(project_id)).await
}

#[tauri::command]
pub async fn export_project(
    state: State<'_, AppState>,
    project_id: Uuid,
    nested: bool,
    namespace: Option<String>,
) -> Result<ExportReport, ApiError> {
    let exports_dir = state.exports_dir.clone();
    run(&state, move |service| {
        let project = service.get_project(project_id)?;
        let output_dir = exports_dir.join(&project.slug);
        service.export_translations(
            project_id,
            ExportOptions {
                output_dir,
                format: Default::default(),
                nested,
                namespace,
            },
        )
    })
    .await
}

#[tauri::command]
pub async fn import_project(
    state: State<'_, AppState>,
    project_id: Uuid,
    path: String,
    namespace: String,
) -> Result<ImportReport, ApiError> {
    run(&state, move |service| {
        service.import_translations(
            project_id,
            ImportOptions {
                path: PathBuf::from(path),
                namespace,
            },
        )
    })
    .await
}


#[tauri::command]
pub async fn collab_status(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<CollabStatus, ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        collab.status(project_id)
    })
    .await
}

#[tauri::command]
pub async fn collab_bootstrap(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<CollabStatus, ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        collab.bootstrap(project_id)
    })
    .await
}

#[tauri::command]
pub async fn collab_export(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<String, ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        let envelope = collab.export_update(project_id)?;
        Ok(serde_json::to_string_pretty(&envelope)?)
    })
    .await
}

#[tauri::command]
pub async fn collab_import(
    state: State<'_, AppState>,
    project_id: Uuid,
    payload: String,
) -> Result<CollabStatus, ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        let envelope: SyncEnvelope = serde_json::from_str(&payload)?;
        collab.import_update(project_id, &envelope)?;
        collab.status(project_id)
    })
    .await
}

#[tauri::command]
pub async fn collab_rebuild_projection(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<(), ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        collab.rebuild_projection(project_id)
    })
    .await
}

#[tauri::command]
pub async fn collab_validate(
    state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<CheckReport, ApiError> {
    run(&state, move |service| {
        let collab = CollaborationService::new(service.store_arc(), "tauri-peer");
        collab.validate(project_id)
    })
    .await
}

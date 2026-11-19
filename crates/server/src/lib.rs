//! Optional HTTP adapter.
//!
//! F1 exposes **read-only** endpoints and healthchecks. The server has no
//! authentication yet, so it binds loopback by default and refuses public binds
//! unless the operator explicitly opts in. Write endpoints, sharing and remote
//! collaboration are intentionally absent.

mod remote_mcp;
mod remote_mcp_sync;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use slimlocalize_core::ports::Store;
use slimlocalize_core::{
    CheckReport, DomainError, Environment, Locale, LocalizeService, Namespace, Project,
    TranslationEntry, Workspace,
};
use slimlocalize_storage::SqliteStore;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub service: Arc<LocalizeService<SqliteStore>>,
    pub(crate) mcp_token: Option<Arc<str>>,
    pub(crate) mcp_allowed_origin: Option<Arc<str>>,
    pub(crate) sync_db_path: Option<Arc<str>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

pub fn router(service: Arc<LocalizeService<SqliteStore>>) -> Router {
    let mcp_token = std::env::var("SLIM_MCP_TOKEN")
        .ok()
        .filter(|token| token.len() >= 32)
        .map(Arc::<str>::from);
    let mcp_allowed_origin = std::env::var("SLIM_MCP_ALLOWED_ORIGIN")
        .ok()
        .filter(|origin| !origin.is_empty())
        .map(Arc::<str>::from);
    let sync_db_path = std::env::var("SLIM_SYNC_DB_READONLY")
        .ok()
        .filter(|path| !path.is_empty())
        .map(Arc::<str>::from);
    let state = AppState {
        service,
        mcp_token,
        mcp_allowed_origin,
        sync_db_path,
    };
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .route("/mcp", axum::routing::post(remote_mcp::handler))
        .route("/api/v1/workspaces", get(list_workspaces))
        .route(
            "/api/v1/workspaces/{workspace}/projects",
            get(list_projects),
        )
        .route(
            "/api/v1/workspaces/{workspace}/projects/{project}/locales",
            get(list_locales),
        )
        .route(
            "/api/v1/workspaces/{workspace}/projects/{project}/namespaces",
            get(list_namespaces),
        )
        .route(
            "/api/v1/workspaces/{workspace}/projects/{project}/translations",
            get(list_translations),
        )
        .route(
            "/api/v1/workspaces/{workspace}/projects/{project}/environments",
            get(list_environments),
        )
        .route(
            "/api/v1/workspaces/{workspace}/projects/{project}/check",
            get(check_project),
        )
        .with_state(state)
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(15),
        ))
        .layer(TraceLayer::new_for_http())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    mode: &'static str,
    local_first: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "slimlocalize-server",
        version: env!("CARGO_PKG_VERSION"),
        mode: "read-only",
        local_first: true,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadyResponse {
    status: &'static str,
    database: &'static str,
    schema_version: i64,
    mode: &'static str,
}

async fn ready(State(state): State<AppState>) -> Result<Json<ReadyResponse>, ApiError> {
    let service = Arc::clone(&state.service);
    let schema_version = tokio::task::spawn_blocking(move || service.store().schema_version())
        .await
        .map_err(|error| ApiError(DomainError::internal(format!("join error: {error}"))))??;
    Ok(Json(ReadyResponse {
        status: "ready",
        database: "ok",
        schema_version,
        mode: "read-only",
    }))
}

async fn list_workspaces(State(state): State<AppState>) -> Result<Json<Vec<Workspace>>, ApiError> {
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || service.list_workspaces())
            .await
            .map_err(join_error)??,
    ))
}

#[derive(Debug, Deserialize)]
struct ProjectPath {
    workspace: String,
}

async fn list_projects(
    State(state): State<AppState>,
    Path(path): Path<ProjectPath>,
) -> Result<Json<Vec<Project>>, ApiError> {
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || {
            let workspace = service.resolve_workspace(&path.workspace)?;
            service.list_projects(workspace.id)
        })
        .await
        .map_err(join_error)??,
    ))
}

#[derive(Debug, Deserialize)]
struct ProjectTarget {
    workspace: String,
    project: String,
}

async fn list_locales(
    State(state): State<AppState>,
    Path(path): Path<ProjectTarget>,
) -> Result<Json<Vec<Locale>>, ApiError> {
    let project = resolve_project(&state, &path).await?;
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || service.list_locales(project))
            .await
            .map_err(join_error)??,
    ))
}

async fn list_namespaces(
    State(state): State<AppState>,
    Path(path): Path<ProjectTarget>,
) -> Result<Json<Vec<Namespace>>, ApiError> {
    let project = resolve_project(&state, &path).await?;
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || service.list_namespaces(project))
            .await
            .map_err(join_error)??,
    ))
}

async fn list_environments(
    State(state): State<AppState>,
    Path(path): Path<ProjectTarget>,
) -> Result<Json<Vec<Environment>>, ApiError> {
    let project = resolve_project(&state, &path).await?;
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || service.list_environments(project))
            .await
            .map_err(join_error)??,
    ))
}

#[derive(Debug, Deserialize)]
struct TranslationQuery {
    locale: String,
    namespace: Option<String>,
}

async fn list_translations(
    State(state): State<AppState>,
    Path(path): Path<ProjectTarget>,
    Query(query): Query<TranslationQuery>,
) -> Result<Json<Vec<TranslationEntry>>, ApiError> {
    let project = resolve_project(&state, &path).await?;
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || {
            service.list_translations(project, &query.locale, query.namespace.as_deref())
        })
        .await
        .map_err(join_error)??,
    ))
}

async fn check_project(
    State(state): State<AppState>,
    Path(path): Path<ProjectTarget>,
) -> Result<Json<CheckReport>, ApiError> {
    let project = resolve_project(&state, &path).await?;
    let service = Arc::clone(&state.service);
    Ok(Json(
        tokio::task::spawn_blocking(move || service.check_project(project))
            .await
            .map_err(join_error)??,
    ))
}

async fn resolve_project(state: &AppState, path: &ProjectTarget) -> Result<uuid::Uuid, ApiError> {
    let service = Arc::clone(&state.service);
    let workspace = path.workspace.clone();
    let project = path.project.clone();
    Ok(tokio::task::spawn_blocking(move || {
        let workspace = service.resolve_workspace(&workspace)?;
        Ok::<_, DomainError>(service.resolve_project(workspace.id, &project)?.id)
    })
    .await
    .map_err(join_error)??)
}

fn join_error(error: tokio::task::JoinError) -> ApiError {
    ApiError(DomainError::internal(format!("join error: {error}")))
}

#[derive(Debug)]
struct ApiError(DomainError);

impl From<DomainError> for ApiError {
    fn from(error: DomainError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            DomainError::NotFound { .. } => StatusCode::NOT_FOUND,
            DomainError::Validation { .. } => StatusCode::BAD_REQUEST,
            DomainError::Conflict { .. } => StatusCode::CONFLICT,
            DomainError::Unsupported { .. } => StatusCode::NOT_IMPLEMENTED,
            DomainError::PathTraversal { .. } => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = json!({
            "error": {
                "code": self.0.code(),
                "message": self.0.to_string(),
            }
        });
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use slimlocalize_core::{
        AddLocaleInput, CreateProjectInput, CreateWorkspaceInput, SaveTranslationInput,
        TranslationStatus,
    };
    use tower::ServiceExt;

    fn test_app() -> (tempfile::TempDir, Router) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("server.sqlite3")).unwrap();
        let service = Arc::new(LocalizeService::new(store));
        let workspace = service
            .create_workspace(CreateWorkspaceInput {
                name: "Server".to_owned(),
                slug: None,
                description: None,
            })
            .unwrap();
        let project = service
            .create_project(
                workspace.id,
                CreateProjectInput {
                    name: "App".to_owned(),
                    slug: None,
                    description: None,
                    default_locale: Some("en-US".to_owned()),
                },
            )
            .unwrap();
        service
            .add_locale(
                project.id,
                AddLocaleInput {
                    code: "es-PE".to_owned(),
                    name: None,
                    is_default: false,
                },
            )
            .unwrap();
        service
            .save_translation(SaveTranslationInput {
                project_id: project.id,
                namespace: "common".to_owned(),
                key: "welcome".to_owned(),
                locale_code: "en-US".to_owned(),
                value: "Welcome {name}".to_owned(),
                status: TranslationStatus::Translated,
                description: None,
            })
            .unwrap();
        (dir, router(service))
    }

    async fn json_response(app: Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn health_and_ready_respond() {
        let (_dir, app) = test_app();
        let (status, body) = json_response(app.clone(), "/healthz").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["mode"], "read-only");
        let (status, body) = json_response(app, "/readyz").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["schemaVersion"], 2);
    }

    #[tokio::test]
    async fn read_only_project_routes_work() {
        let (_dir, app) = test_app();
        let (status, body) = json_response(app.clone(), "/api/v1/workspaces").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);
        let (status, body) = json_response(
            app.clone(),
            "/api/v1/workspaces/server/projects/app/locales",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 2);
        let (status, body) = json_response(
            app.clone(),
            "/api/v1/workspaces/server/projects/app/translations?locale=es-PE&namespace=common",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert!(body[0]["value"].is_null());
        let (status, body) =
            json_response(app, "/api/v1/workspaces/server/projects/app/check").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["missing"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn missing_workspace_is_404() {
        let (_dir, app) = test_app();
        let (status, body) =
            json_response(app, "/api/v1/workspaces/nope/projects/also-nope/locales").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }

    #[tokio::test]
    async fn write_methods_are_not_exposed() {
        let (_dir, app) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workspaces")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}

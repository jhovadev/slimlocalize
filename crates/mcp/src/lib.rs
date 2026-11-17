//! Local MCP tool surface for SlimLocalize.
//!
//! The registry is **read-only by construction**. `propose_translation` and any
//! other mutation are documented as future contracts but are not callable. Remote
//! MCP is not implemented and must remain disabled until authentication,
//! authorization and transport security exist.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use slimlocalize_core::ports::Store;
use slimlocalize_core::{CheckReport, DomainError, LocalizeService};
use thiserror::Error;

pub const PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
    pub enabled: bool,
}

/// Tools advertised through `tools/list`. All of them are read-only.
#[must_use]
pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        read_only_tool(
            "list_workspaces",
            "List local workspaces.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        ),
        read_only_tool(
            "list_projects",
            "List projects in a workspace. `workspace` accepts a slug or UUID.",
            json!({
                "type": "object",
                "properties": { "workspace": { "type": "string" } },
                "required": ["workspace"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "list_locales",
            "List locales configured for a project.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" }
                },
                "required": ["workspace", "project"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "get_translation",
            "Get one translation entry including its status. Missing values are returned as null.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" },
                    "locale": { "type": "string" },
                    "namespace": { "type": "string" },
                    "key": { "type": "string" }
                },
                "required": ["workspace", "project", "locale", "namespace", "key"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "search_translations",
            "Case-insensitive search over keys and values. Read-only.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" },
                    "query": { "type": "string" },
                    "locale": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 200 }
                },
                "required": ["workspace", "project", "query"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "find_missing_translations",
            "List keys missing from non-default locales.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" }
                },
                "required": ["workspace", "project"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "validate_project",
            "Run missing-key and placeholder validation.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" }
                },
                "required": ["workspace", "project"],
                "additionalProperties": false
            }),
        ),
        read_only_tool(
            "list_environments",
            "List environment metadata. Configuration values are intentionally not exposed.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": { "type": "string" },
                    "project": { "type": "string" }
                },
                "required": ["workspace", "project"],
                "additionalProperties": false
            }),
        ),
    ]
}

/// Contracts for future write-capable tools. Not returned by `tools/list` and
/// not callable until authentication/authorization and human approval exist.
#[must_use]
pub fn future_tool_contracts() -> Vec<ToolDefinition> {
    vec![ToolDefinition {
        name: "propose_translation".to_owned(),
        description: "Future: propose a translation as a reviewable draft. Requires authentication, ACL and audit approval.".to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "workspace": { "type": "string" },
                "project": { "type": "string" },
                "locale": { "type": "string" },
                "namespace": { "type": "string" },
                "key": { "type": "string" },
                "value": { "type": "string" }
            },
            "required": ["workspace", "project", "locale", "namespace", "key", "value"],
            "additionalProperties": false
        }),
        read_only: false,
        enabled: false,
    }]
}

fn read_only_tool(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: name.to_owned(),
        description: description.to_owned(),
        input_schema,
        read_only: true,
        enabled: true,
    }
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("domain error: {0}")]
    Domain(#[from] DomainError),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default)]
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn error(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Read-only tool dispatcher over the shared application service.
#[derive(Debug)]
pub struct McpToolRegistry<S: Store> {
    service: Arc<LocalizeService<S>>,
}

impl<S: Store> McpToolRegistry<S> {
    #[must_use]
    pub fn new(service: Arc<LocalizeService<S>>) -> Self {
        Self { service }
    }

    pub fn call(&self, name: &str, arguments: &Value) -> Result<Value, McpError> {
        match name {
            "list_workspaces" => Ok(serde_json::to_value(self.service.list_workspaces()?)?),
            "list_projects" => {
                let workspace = required_str(arguments, "workspace")?;
                let workspace = self.service.resolve_workspace(workspace)?;
                Ok(serde_json::to_value(
                    self.service.list_projects(workspace.id)?,
                )?)
            }
            "list_locales" => {
                let project = self.resolve_project(arguments)?;
                Ok(serde_json::to_value(self.service.list_locales(project)?)?)
            }
            "get_translation" => {
                let project = self.resolve_project(arguments)?;
                let locale = required_str(arguments, "locale")?;
                let namespace = required_str(arguments, "namespace")?;
                let key = required_str(arguments, "key")?;
                Ok(serde_json::to_value(
                    self.service
                        .get_translation(project, locale, namespace, key)?,
                )?)
            }
            "search_translations" => {
                let project = self.resolve_project(arguments)?;
                let query = required_str(arguments, "query")?;
                let locale = optional_str(arguments, "locale");
                let limit = arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .map_or(50, |value| value as usize);
                Ok(serde_json::to_value(
                    self.service
                        .search_translations(project, query, locale, limit)?,
                )?)
            }
            "find_missing_translations" => {
                let project = self.resolve_project(arguments)?;
                let report: CheckReport = self.service.check_project(project)?;
                Ok(serde_json::to_value(report.missing)?)
            }
            "validate_project" => {
                let project = self.resolve_project(arguments)?;
                Ok(serde_json::to_value(self.service.check_project(project)?)?)
            }
            "list_environments" => {
                let project = self.resolve_project(arguments)?;
                Ok(serde_json::to_value(
                    self.service.list_environments(project)?,
                )?)
            }
            other => Err(McpError::UnknownTool(other.to_owned())),
        }
    }

    fn resolve_project(&self, arguments: &Value) -> Result<uuid::Uuid, McpError> {
        let workspace = required_str(arguments, "workspace")?;
        let project = required_str(arguments, "project")?;
        let workspace = self.service.resolve_workspace(workspace)?;
        Ok(self.service.resolve_project(workspace.id, project)?.id)
    }
}

/// Minimal local JSON-RPC server for stdio integration.
#[derive(Debug)]
pub struct McpServer<S: Store> {
    registry: McpToolRegistry<S>,
}

impl<S: Store> McpServer<S> {
    #[must_use]
    pub fn new(service: Arc<LocalizeService<S>>) -> Self {
        Self {
            registry: McpToolRegistry::new(service),
        }
    }

    /// Handle one JSON-RPC message. Notifications (no id) return `None`.
    pub fn handle(&self, request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => id.map(|id| {
                JsonRpcResponse::success(
                    id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": { "tools": { "listChanged": false } },
                        "serverInfo": {
                            "name": "slimlocalize-local",
                            "version": env!("CARGO_PKG_VERSION")
                        },
                        "instructions": "Local, read-only SlimLocalize tools. Writes, secrets and remote access are disabled."
                    }),
                )
            }),
            "notifications/initialized" => None,
            "ping" => id.map(|id| JsonRpcResponse::success(id, json!({}))),
            "tools/list" => id.map(|id| {
                JsonRpcResponse::success(
                    id,
                    json!({ "tools": tool_definitions() }),
                )
            }),
            "tools/call" => {
                let id = id?;
                let name = request
                    .params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let arguments = request
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match self.registry.call(name, &arguments) {
                    Ok(value) => Some(JsonRpcResponse::success(
                        id,
                        json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&value).unwrap_or_else(|_| "null".to_owned())
                            }],
                            "isError": false
                        }),
                    )),
                    Err(McpError::UnknownTool(name)) => Some(JsonRpcResponse::error(
                        id,
                        -32602,
                        format!("unknown or disabled tool: {name}"),
                    )),
                    Err(error) => Some(JsonRpcResponse::success(
                        id,
                        json!({
                            "content": [{
                                "type": "text",
                                "text": error.to_string()
                            }],
                            "isError": true
                        }),
                    )),
                }
            }
            _ => id.map(|id| {
                JsonRpcResponse::error(id, -32601, format!("method not found: {}", request.method))
            }),
        }
    }

    pub fn handle_line(&self, line: &str) -> Option<String> {
        let request: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(request) => request,
            Err(error) => {
                return Some(
                    serde_json::to_string(&JsonRpcResponse::error(
                        Value::Null,
                        -32700,
                        format!("parse error: {error}"),
                    ))
                    .unwrap_or_else(|_| "{}".to_owned()),
                );
            }
        };
        self.handle(&request)
            .and_then(|response| serde_json::to_string(&response).ok())
    }
}

fn required_str<'a>(arguments: &'a Value, field: &str) -> Result<&'a str, McpError> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| McpError::InvalidArguments(format!("missing string field `{field}`")))
}

fn optional_str<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use slimlocalize_core::{
        AddLocaleInput, CreateProjectInput, CreateWorkspaceInput, SaveTranslationInput,
    };
    use slimlocalize_storage::SqliteStore;
    use std::sync::Arc;

    fn fixture() -> (tempfile::TempDir, McpServer<SqliteStore>) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open(dir.path().join("mcp.sqlite3")).unwrap();
        let service = Arc::new(LocalizeService::new(store));
        let workspace = service
            .create_workspace(CreateWorkspaceInput {
                name: "MCP".to_owned(),
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
                value: "Welcome, {name}!".to_owned(),
                status: slimlocalize_core::TranslationStatus::Translated,
                description: None,
            })
            .unwrap();
        (dir, McpServer::new(service))
    }

    fn call(server: &McpServer<SqliteStore>, name: &str, args: Value) -> Value {
        let response = server
            .handle(&JsonRpcRequest {
                jsonrpc: "2.0".to_owned(),
                id: Some(json!(1)),
                method: "tools/call".to_owned(),
                params: json!({ "name": name, "arguments": args }),
            })
            .unwrap();
        assert!(response.error.is_none());
        let result = response.result.unwrap();
        if result["isError"] == json!(true) {
            panic!("tool error: {}", result["content"][0]["text"]);
        }
        let text = result["content"][0]["text"].as_str().unwrap();
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn lists_only_read_only_tools() {
        let tools = tool_definitions();
        assert!(tools.iter().all(|tool| tool.read_only && tool.enabled));
        assert!(tools.iter().any(|tool| tool.name == "validate_project"));
        assert!(tools.iter().all(|tool| tool.name != "propose_translation"));
        assert!(!future_tool_contracts()[0].enabled);
    }

    #[test]
    fn dispatches_read_only_queries() {
        let (_dir, server) = fixture();
        let workspaces = call(&server, "list_workspaces", json!({}));
        assert_eq!(workspaces.as_array().unwrap().len(), 1);
        let projects = call(&server, "list_projects", json!({ "workspace": "mcp" }));
        assert_eq!(projects.as_array().unwrap().len(), 1);
        let locales = call(
            &server,
            "list_locales",
            json!({ "workspace": "mcp", "project": "app" }),
        );
        assert_eq!(locales.as_array().unwrap().len(), 2);
        let report = call(
            &server,
            "validate_project",
            json!({ "workspace": "mcp", "project": "app" }),
        );
        assert_eq!(report["missing"].as_array().unwrap().len(), 1);
        let search = call(
            &server,
            "search_translations",
            json!({ "workspace": "mcp", "project": "app", "query": "welcome" }),
        );
        assert!(!search.as_array().unwrap().is_empty());
    }

    #[test]
    fn unknown_write_tool_is_rejected() {
        let (_dir, server) = fixture();
        let response = server
            .handle(&JsonRpcRequest {
                jsonrpc: "2.0".to_owned(),
                id: Some(json!(2)),
                method: "tools/call".to_owned(),
                params: json!({
                    "name": "propose_translation",
                    "arguments": {
                        "workspace": "mcp",
                        "project": "app",
                        "locale": "es-PE",
                        "namespace": "common",
                        "key": "welcome",
                        "value": "Hola"
                    }
                }),
            })
            .unwrap();
        assert_eq!(response.error.unwrap().code, -32602);
    }

    #[test]
    fn notification_initialized_has_no_response() {
        let (_dir, server) = fixture();
        let response = server.handle(&JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: None,
            method: "notifications/initialized".to_owned(),
            params: json!({}),
        });
        assert!(response.is_none());
    }
}

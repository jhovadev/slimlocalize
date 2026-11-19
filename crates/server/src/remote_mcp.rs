//! Stateless Streamable HTTP MCP adapter for the existing read-only registry.
//! Security boundary: a dedicated Bearer token AND origin validation; the
//! endpoint is off when SLIM_MCP_TOKEN is absent. Never expose the raw REST API.
use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use slimlocalize_mcp::{JsonRpcRequest, JsonRpcResponse, McpServer};
use std::sync::Arc;

use crate::{AppState, remote_mcp_sync};

fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0));
    }
    diff == 0
}

fn bearer_matches(headers: &HeaderMap, expected: &str) -> bool {
    let Some(header) = headers.get(header::AUTHORIZATION) else {
        return false;
    };
    let Ok(value) = header.to_str() else {
        return false;
    };
    let Some(bearer) = value.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_equal(bearer.as_bytes(), expected.as_bytes())
}

fn valid_origin(headers: &HeaderMap, expected: Option<&str>) -> bool {
    match headers.get(header::ORIGIN) {
        None => true, // Non-browser MCP clients typically omit Origin.
        Some(origin) => expected.is_some_and(|allowed| origin.to_str().ok() == Some(allowed)),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Response {
    Json(JsonRpcResponse::error(id, code, message)).into_response()
}

pub async fn handler(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(token) = state.mcp_token.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !bearer_matches(&headers, token) {
        return (
            StatusCode::UNAUTHORIZED,
            [(
                header::WWW_AUTHENTICATE,
                "Bearer realm=\"SlimLocalize MCP\"",
            )],
        )
            .into_response();
    }
    if !valid_origin(&headers, state.mcp_allowed_origin.as_deref()) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"))
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return rpc_error(Value::Null, -32700, "Invalid JSON-RPC request"),
    };
    if request.jsonrpc != "2.0" {
        return rpc_error(
            request.id.unwrap_or(Value::Null),
            -32600,
            "Expected JSON-RPC 2.0",
        );
    }
    if request.method == "notifications/initialized" {
        return StatusCode::ACCEPTED.into_response();
    }
    if request.method == "initialize" {
        let Some(id) = request.id else {
            return StatusCode::ACCEPTED.into_response();
        };
        let requested_version = request
            .params
            .get("protocolVersion")
            .and_then(Value::as_str);
        if requested_version != Some("2025-11-25") {
            return rpc_error(id, -32602, "Supported MCP protocolVersion: 2025-11-25");
        }
        return Json(JsonRpcResponse::success(id, json!({
            "protocolVersion": "2025-11-25",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "slimlocalize-readonly-http", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Read-only API SQLite tools and shared Loro snapshot tools (if configured). Shared environment values and write tools are excluded."
        }))).into_response();
    }
    if request.method == "tools/list" {
        let Some(id) = request.id else {
            return StatusCode::ACCEPTED.into_response();
        };
        let mut tools = slimlocalize_mcp::tool_definitions();
        if state.sync_db_path.is_some() {
            tools.extend(remote_mcp_sync::tool_definitions());
        }
        return Json(JsonRpcResponse::success(id, json!({"tools": tools}))).into_response();
    }
    if request.method == "tools/call" {
        let name = request
            .params
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if remote_mcp_sync::is_shared_tool(name) {
            let Some(id) = request.id else {
                return StatusCode::ACCEPTED.into_response();
            };
            let Some(path) = state.sync_db_path.clone() else {
                return rpc_error(id, -32602, "Shared relay database is not configured");
            };
            let name = name.to_owned();
            let args = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            return match tokio::task::spawn_blocking(move ||
                remote_mcp_sync::call(std::path::Path::new(path.as_ref()), &name, &args)
            ).await {
                Ok(Ok(value)) => Json(JsonRpcResponse::success(id, json!({
                    "content": [{"type":"text", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}],
                    "isError": false
                }))).into_response(),
                Ok(Err(message)) => Json(JsonRpcResponse::success(id, json!({
                    "content": [{"type":"text", "text":message}], "isError":true
                }))).into_response(),
                Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            };
        }
    }
    if !matches!(request.method.as_str(), "tools/call" | "ping") {
        return rpc_error(
            request.id.unwrap_or(Value::Null),
            -32601,
            "Unsupported method",
        );
    }
    // The storage adapter uses synchronous SQLite methods; keep it off Tokio worker threads.
    let service = Arc::clone(&state.service);
    match tokio::task::spawn_blocking(move || McpServer::new(service).handle(&request)).await {
        Ok(Some(response)) => Json(response).into_response(),
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    #[test]
    fn requires_exact_bearer_and_origin() {
        let mut headers = HeaderMap::new();
        let token = "0123456789abcdef0123456789abcdef";
        assert!(!bearer_matches(&headers, token));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer 0123456789abcdef0123456789abcdef"),
        );
        assert!(bearer_matches(&headers, token));
        assert!(!bearer_matches(
            &headers,
            "another0123456789abcdef0123456789ab"
        ));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!valid_origin(&headers, None));
        assert!(!valid_origin(&headers, Some("https://app.example")));
        assert!(valid_origin(&headers, Some("https://evil.example")));
    }
}

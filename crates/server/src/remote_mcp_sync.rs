//! Read-only MCP projection of the *persisted* Loro collaboration documents.
//! This is deliberately not an authority for writes, permissions or invitations.
use loro::{LoroDoc, ToJson};
use rusqlite::{Connection, OpenFlags, params};
use serde_json::{Value, json};
use slimlocalize_mcp::ToolDefinition;
use std::path::Path;

const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;
const MAX_RESULTS: usize = 200;

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_shared_projects".into(),
            description: "List project IDs that have persisted in the team Loro relay. Project metadata is browser-local.".into(),
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            read_only: true, enabled: true,
        },
        ToolDefinition {
            name: "list_shared_translations".into(),
            description: "Read a team's persisted translations, optionally filtered by locale and namespace. Max 200 entries.".into(),
            input_schema: json!({"type":"object","properties":{
                "projectId":{"type":"string"},
                "locale":{"type":"string"},
                "namespace":{"type":"string"},
                "limit":{"type":"integer","minimum":1,"maximum":200}
            },"required":["projectId"],"additionalProperties":false}),
            read_only: true, enabled: true,
        },
        ToolDefinition {
            name: "list_shared_environments".into(),
            description: "Read shared environment names and kinds only; never returns configuration values.".into(),
            input_schema: json!({"type":"object","properties":{
                "projectId":{"type":"string"}
            },"required":["projectId"],"additionalProperties":false}),
            read_only: true, enabled: true,
        },
    ]
}

pub fn is_shared_tool(name: &str) -> bool {
    matches!(
        name,
        "list_shared_projects" | "list_shared_translations" | "list_shared_environments"
    )
}

fn connection(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("Cannot read relay database: {error}"))
}

fn snapshot(conn: &Connection, project_id: &str) -> Result<Value, String> {
    let parsed_id = uuid::Uuid::parse_str(project_id)
        .map_err(|_| "projectId must be a valid UUID".to_owned())?;
    let room = format!("project:{parsed_id}");
    let blob: Vec<u8> = conn
        .query_row(
            "SELECT data FROM documents WHERE workspace='' AND room=?1 AND crdt='loro'",
            params![room],
            |row| row.get(0),
        )
        .map_err(|error| format!("Project has no persisted Loro snapshot: {error}"))?;
    if blob.len() > MAX_SNAPSHOT_BYTES {
        return Err("Collaboration snapshot exceeds 8 MiB MCP read limit".into());
    }
    let doc = LoroDoc::new();
    doc.import(&blob)
        .map_err(|_| "Invalid Loro snapshot".to_owned())?;
    Ok(doc.get_deep_value().to_json_value())
}

pub fn call(path: &Path, name: &str, args: &Value) -> Result<Value, String> {
    let conn = connection(path)?;
    match name {
        "list_shared_projects" => {
            let mut stmt = conn.prepare(
                "SELECT room, updated_at FROM documents WHERE workspace='' AND crdt='loro' AND room LIKE 'project:%' ORDER BY updated_at DESC LIMIT 200"
            ).map_err(|error| error.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    let room: String = row.get(0)?;
                    let updated_at: i64 = row.get(1)?;
                    Ok((room, updated_at))
                })
                .map_err(|error| error.to_string())?;
            let mut projects = Vec::new();
            for result in rows {
                let (room, updated_at) = result.map_err(|error| error.to_string())?;
                if let Some(id) = room.strip_prefix("project:")
                    && uuid::Uuid::parse_str(id).is_ok()
                {
                    projects.push(json!({"projectId":id,"updatedAtMs":updated_at}));
                }
            }
            Ok(Value::Array(projects))
        }
        "list_shared_translations" => {
            let project_id = args
                .get("projectId")
                .and_then(Value::as_str)
                .ok_or_else(|| "Missing projectId".to_owned())?;
            let document = snapshot(&conn, project_id)?;
            let locale = args.get("locale").and_then(Value::as_str);
            let namespace = args.get("namespace").and_then(Value::as_str);
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map_or(MAX_RESULTS, |num| (num as usize).clamp(1, MAX_RESULTS));
            let mut entries = Vec::new();
            if let Some(map) = document.get("translations").and_then(Value::as_object) {
                for (encoded, value) in map {
                    let mut fields = encoded.split('\u{001f}');
                    let (Some(ns), Some(key), Some(code), None) =
                        (fields.next(), fields.next(), fields.next(), fields.next())
                    else {
                        continue;
                    };
                    if value.as_str().is_none()
                        || locale.is_some_and(|want| want != code)
                        || namespace.is_some_and(|want| want != ns)
                    {
                        continue;
                    }
                    entries.push(json!({"namespace":ns,"key":key,"locale":code,"value":value}));
                    if entries.len() >= limit {
                        break;
                    }
                }
            }
            Ok(Value::Array(entries))
        }
        "list_shared_environments" => {
            let project_id = args
                .get("projectId")
                .and_then(Value::as_str)
                .ok_or_else(|| "Missing projectId".to_owned())?;
            let document = snapshot(&conn, project_id)?;
            // Explicitly omit env_variables even if the Loro document contains them.
            let environments = document
                .get("environments")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            Ok(json!({"environments": environments}))
        }
        _ => Err("Unknown or disabled shared tool".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loro::ExportMode;
    #[test]
    fn read_only_snapshot_projection_omits_configuration_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync.sqlite");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE documents(workspace TEXT, crdt TEXT, room TEXT, data BLOB, updated_at INTEGER)").unwrap();
        let doc = LoroDoc::new();
        doc.get_map("translations")
            .insert("common\u{001f}hello\u{001f}en-US", "Hello")
            .unwrap();
        doc.get_map("environments")
            .insert("Production", "production")
            .unwrap();
        doc.get_map("env_variables")
            .insert("Production\u{001f}PUBLIC_URL", "https://example.test")
            .unwrap();
        doc.commit();
        let data = doc.export(ExportMode::all_updates()).unwrap();
        let project_id = uuid::Uuid::new_v4();
        conn.execute(
            "INSERT INTO documents(workspace,crdt,room,data,updated_at) VALUES ('','loro',?1,?2,0)",
            params![format!("project:{project_id}"), data],
        )
        .unwrap();
        drop(conn);
        let result = call(
            &path,
            "list_shared_translations",
            &json!({"projectId":project_id.to_string()}),
        )
        .unwrap();
        assert_eq!(result[0]["value"], "Hello");
        let result = call(
            &path,
            "list_shared_environments",
            &json!({"projectId":project_id.to_string()}),
        )
        .unwrap();
        assert_eq!(result["environments"]["Production"], "production");
        assert!(!result.to_string().contains("PUBLIC_URL"));
        assert!(!result.to_string().contains("example.test"));
    }
}

use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use loro_websocket_server::protocol::CrdtType;
use loro_websocket_server::{
    serve_incoming_with_config, LoadDocArgs, LoadedDoc, SaveDocArgs, ServerConfig,
};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "slimlocalize-sync-relay", version, about)]
struct Args {
    #[arg(long, env = "SLIM_SYNC_BIND", default_value = "0.0.0.0:8787")]
    bind: String,

    #[arg(long, env = "SLIM_SYNC_HEALTH_BIND", default_value = "0.0.0.0:8788")]
    health_bind: String,

    #[arg(long, env = "SLIM_SYNC_DB", default_value = "sync.sqlite")]
    db: PathBuf,

    #[arg(long, env = "SLIM_SYNC_MODE", default_value = "development")]
    mode: String,

    #[arg(long, env = "SLIM_SYNC_TOKEN")]
    token: Option<String>,

    #[arg(long, env = "SLIM_SYNC_SAVE_INTERVAL_MS", default_value_t = 1000)]
    save_interval_ms: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = Args::parse();
    init_tracing();
    if args.mode == "production" && args.token.as_deref().is_none_or(|key| key.len() < 32) {
        return Err("production mode requires a SLIM_SYNC_TOKEN of at least 32 characters".into());
    }

    init_db(&args.db).await?;
    let db_for_load = args.db.clone();
    let db_for_save = args.db.clone();
    type LoadFut = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<LoadedDoc<()>, String>> + Send>,
    >;
    type SaveFut = std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>;

    let on_load = Arc::new(move |args: LoadDocArgs| {
        let db_path = db_for_load.clone();
        let fut = async move {
            let snapshot = tokio::task::spawn_blocking(move || {
                load_snapshot(&db_path, &args.workspace, &args.room, args.crdt)
            })
            .await
            .map_err(|e| format!("task join error: {e}"))?;
            let snapshot = snapshot?;
            Ok(LoadedDoc { snapshot, ctx: None })
        };
        let fut: LoadFut = Box::pin(fut);
        fut
    });

    let on_save = Arc::new(move |args: SaveDocArgs<()>| {
        let db_path = db_for_save.clone();
        let fut = async move {
            tokio::task::spawn_blocking(move || {
                save_snapshot(&db_path, &args.workspace, &args.room, args.crdt, &args.data)
            })
            .await
            .map_err(|e| format!("task join error: {e}"))?
        };
        let fut: SaveFut = Box::pin(fut);
        fut
    });

    let token = args.token.clone();
    let mode = args.mode.clone();
    type AuthFut = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<loro_websocket_server::protocol::Permission>, String>> + Send>>;
    let authenticate = Arc::new(move |_workspace: String, _crdt: CrdtType, auth: Vec<u8>| {
        let token = token.clone();
        let mode = mode.clone();
        let fut = async move {
            if mode == "production" {
                let expected = match token {
                    Some(value) => value,
                    None => return Ok(None),
                };
                if constant_time_equal(&auth, expected.as_bytes()) {
                    Ok(Some(loro_websocket_server::protocol::Permission::Write))
                } else {
                    Ok(None)
                }
            } else {
                // Development mode is explicit and local-only. Writes are allowed
                // without a shared secret so the optional relay can be tested.
                Ok(Some(loro_websocket_server::protocol::Permission::Write))
            }
        };
        let fut: AuthFut = Box::pin(fut);
        fut
    });

    // Do not place shared secrets in WebSocket URLs. The authenticate callback
    // denies each join (including ephemeral rooms) unless its auth bytes match.
    // A connection alone never authorizes reading a document.
    let cfg = ServerConfig {
        on_load_document: Some(on_load),
        on_save_document: Some(on_save),
        save_interval_ms: Some(args.save_interval_ms),
        default_permission: loro_websocket_server::protocol::Permission::Read,
        authenticate: Some(authenticate),
        handshake_auth: None,
    };

    let listener = TcpListener::bind(&args.bind).await?;
    info!(bind = %args.bind, db = %args.db.display(), mode = %args.mode, "starting sync relay");

    let health_listener = TcpListener::bind(&args.health_bind).await?;
    tokio::spawn(async move {
        loop {
            if let Ok((mut stream, _)) = health_listener.accept().await {
                use tokio::io::AsyncWriteExt;
                let _ = stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                    .await;
            }
        }
    });

    serve_incoming_with_config(listener, cfg).await
}

async fn init_db(path: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
    let path = path.clone();
    tokio::task::spawn_blocking(move || -> Result<(), Box<dyn Error + Send + Sync>> {
        let conn = rusqlite::Connection::open(path)?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS documents (
                workspace TEXT NOT NULL,
                crdt TEXT NOT NULL,
                room TEXT NOT NULL,
                data BLOB NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (workspace, crdt, room)
            );
            "#,
        )?;
        Ok(())
    })
    .await
    .map_err(|e| format!("task join error: {e}"))??;
    Ok(())
}

fn load_snapshot(
    path: &PathBuf,
    workspace: &str,
    room: &str,
    crdt: CrdtType,
) -> Result<Option<Vec<u8>>, String> {
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM documents WHERE workspace=?1 AND crdt=?2 AND room=?3")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query(rusqlite::params![workspace, crdt_to_str(crdt), room])
        .map_err(|e| e.to_string())?;
    if let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let data: Vec<u8> = row.get(0).map_err(|e| e.to_string())?;
        Ok(Some(data))
    } else {
        Ok(None)
    }
}

fn save_snapshot(
    path: &PathBuf,
    workspace: &str,
    room: &str,
    crdt: CrdtType,
    data: &[u8],
) -> Result<(), String> {
    let conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    let now_ms = chrono_like_now_ms();
    conn.execute(
        "INSERT INTO documents (workspace, crdt, room, data, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(workspace, crdt, room) DO UPDATE SET data=excluded.data, updated_at=excluded.updated_at",
        rusqlite::params![workspace, crdt_to_str(crdt), room, data, now_ms],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn crdt_to_str(crdt: CrdtType) -> &'static str {
    match crdt {
        CrdtType::Loro => "loro",
        CrdtType::LoroEphemeralStore => "loro_ephemeral",
        CrdtType::LoroEphemeralStorePersisted => "loro_ephemeral_persisted",
        CrdtType::Yjs => "yjs",
        CrdtType::YjsAwareness => "yjs_awareness",
        CrdtType::Elo => "elo",
    }
}

fn chrono_like_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    dur.as_millis() as i64
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(false)
        .init();
}

/// Avoid byte-by-byte early exits when comparing a shared team key.
fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    let mut difference = a.len() ^ b.len();
    for index in 0..a.len().max(b.len()) {
        let left = a.get(index).copied().unwrap_or(0);
        let right = b.get(index).copied().unwrap_or(0);
        difference |= usize::from(left ^ right);
    }
    difference == 0
}

#[cfg(test)]
mod security_tests {
    use super::constant_time_equal;
    #[test]
    fn rejects_short_and_incorrect_keys() {
        assert!(constant_time_equal(b"correct-team-key", b"correct-team-key"));
        assert!(!constant_time_equal(b"correct-team-key", b"other-team-key"));
        assert!(!constant_time_equal(b"correct-team-key", b"correct-team-key-extra"));
    }
}

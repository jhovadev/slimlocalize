use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use slimlocalize_core::LocalizeService;
use slimlocalize_server::router;
use slimlocalize_storage::{SqliteStore, default_database_path};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "slimlocalize-server", version, about)]
struct Args {
    /// Bind address. Loopback by default; public binds require explicit opt-in.
    #[arg(long, env = "SLIMLOCALIZE_BIND", default_value = "127.0.0.1:8787")]
    bind: SocketAddr,

    /// SQLite database path.
    #[arg(long, env = "SLIMLOCALIZE_DB")]
    db: Option<PathBuf>,

    /// Log format: plain or json.
    #[arg(long, env = "SLIMLOCALIZE_LOG_FORMAT", default_value = "plain")]
    log_format: String,

    /// Allow a non-loopback bind. The API is read-only but unauthenticated.
    #[arg(
        long,
        env = "SLIMLOCALIZE_ALLOW_PUBLIC_READONLY",
        default_value_t = false
    )]
    allow_public_readonly: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    init_tracing(&args.log_format);

    if !is_loopback(args.bind.ip()) && !args.allow_public_readonly {
        return Err(format!(
            "refusing to bind {}: the F1 server has no authentication. \
             Use --allow-public-readonly only on a trusted network, or keep loopback.",
            args.bind
        )
        .into());
    }

    let db_path = args.db.unwrap_or_else(default_database_path);
    let store = SqliteStore::open(&db_path)?;
    let service = Arc::new(LocalizeService::new(store));
    if std::env::var("SLIM_MCP_TOKEN").is_ok_and(|token| token.len() < 32) {
        return Err("SLIM_MCP_TOKEN must contain at least 32 characters".into());
    }
    let app = router(Arc::clone(&service));

    let listener = TcpListener::bind(args.bind).await?;
    tracing::info!(
        bind = %args.bind,
        database = %db_path.display(),
        mode = "read-only",
        "slimlocalize server listening"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("server stopped");
    Ok(())
}

fn is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}

fn init_tracing(format: &str) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("slimlocalize=info,tower_http=info,axum=info"));
    if format == "json" {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .with_current_span(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .compact()
            .init();
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

use std::{env, net::SocketAddr};

use ftnl_web_server::{app, AppState};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ftnl_web_server=info,tower_http=info")),
        )
        .init();
    let address: SocketAddr = env::var("FTNL_WEB_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()
        .expect("FTNL_WEB_BIND must be a socket address");
    let api_origin =
        env::var("FTNL_API_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned());
    let listener = TcpListener::bind(address)
        .await
        .expect("failed to bind FTNL_WEB_BIND");
    info!(%address, %api_origin, "File Tunnel portal listening");
    axum::serve(listener, app(AppState::new(api_origin)))
        .with_graceful_shutdown(shutdown())
        .await
        .expect("server failed");
}

async fn shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}

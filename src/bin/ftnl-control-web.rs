#![forbid(unsafe_code)]

use ftnl_web_server::control::{
    control_router, ControlState, DirectReadBackend, HttpBackend, NatsBackend,
    SharedAuthControlAuthenticator, TcpBackend, TunnelBackend,
};
use sea_orm::Database;
use shared_auth_lib::{AuthorityConfig, Guard, GuardConfig};
use std::{env, net::SocketAddr, sync::Arc, time::Duration};

fn required(name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let value =
        env::var(name).map_err(|_| format!("missing required environment variable {name}"))?;
    if value.trim().is_empty() {
        return Err(format!("environment variable {name} must not be empty").into());
    }
    Ok(value)
}

fn forbids_remote_cleartext(value: &str) -> bool {
    value.starts_with("http://")
        && !value.starts_with("http://127.0.0.1")
        && !value.starts_with("http://localhost")
        && !value.starts_with("http://[::1]")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ftnl_control_web=info,hyper=warn".into()),
        )
        .init();

    let shared_auth_base = required("SHARED_AUTH_BASE_URL")?;
    if forbids_remote_cleartext(&shared_auth_base) {
        return Err("Shared Auth requires HTTPS outside explicit loopback development".into());
    }
    let guard = Guard::new(GuardConfig {
        authority: AuthorityConfig {
            shared_auth_base,
            issuer: required("SHARED_AUTH_ISSUER")?,
            audience: required("SHARED_AUTH_AUDIENCE")?,
            arm_timeout: Duration::from_millis(1200),
            ..AuthorityConfig::default()
        },
        jwks_url: env::var("SHARED_AUTH_JWKS_URL").ok(),
        race_deadline: Duration::from_millis(1500),
        ..GuardConfig::default()
    });

    let mode = env::var("FTNL_CONTROL_MODE").unwrap_or_else(|_| "http".to_owned());
    let backend: Arc<dyn TunnelBackend> = match mode.as_str() {
        "direct_read" => {
            let database = Database::connect(required("FTNL_READONLY_DATABASE_URL")?).await?;
            Arc::new(DirectReadBackend::new_read_only(database))
        }
        "http" => Arc::new(
            HttpBackend::new(&required("FTNL_API_BASE_URL")?)
                .map_err(|_| "invalid HTTP backend configuration")?,
        ),
        "tcp" => {
            let address = required("FTNL_API_TCP_ADDR")?;
            let socket: SocketAddr = address.parse()?;
            let trusted_mesh = env::var("FTNL_TRUSTED_MESH_TCP").as_deref() == Ok("true");
            if !socket.ip().is_loopback() && !trusted_mesh {
                return Err("non-loopback TCP requires FTNL_TRUSTED_MESH_TCP=true and reviewed mTLS mesh policy".into());
            }
            Arc::new(
                TcpBackend::connect(&address)
                    .await
                    .map_err(|_| "TCP backend unavailable")?,
            )
        }
        "nats" => {
            let client = async_nats::connect(required("FTNL_NATS_URL")?).await?;
            let subject = env::var("FTNL_NATS_READ_SUBJECT")
                .unwrap_or_else(|_| "ftnl.tunnel.read.v1".to_owned());
            Arc::new(NatsBackend::new(client, subject))
        }
        _ => return Err("FTNL_CONTROL_MODE must be direct_read, http, tcp, or nats".into()),
    };

    let state = ControlState::new(
        Arc::new(SharedAuthControlAuthenticator::new(guard)),
        backend,
    );
    let address: SocketAddr = env::var("FTNL_CONTROL_BIND")
        .unwrap_or_else(|_| "127.0.0.1:3100".to_owned())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, backend_mode = %mode, "File Tunnel control web ready");
    axum::serve(listener, control_router(state))
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        })
        .await?;
    Ok(())
}

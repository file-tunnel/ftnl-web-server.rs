//! Process configuration and lifecycle for the File Tunnel portal.

use std::{env, net::SocketAddr};

use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{app, observability, AppState};

const DEFAULT_BIND: &str = "127.0.0.1:3000";
const DEFAULT_API_ORIGIN: &str = "http://127.0.0.1:8080";

#[derive(Clone, Debug, Eq, PartialEq)]
struct RuntimeConfig {
    address: SocketAddr,
    api_origin: String,
}

impl RuntimeConfig {
    fn from_values(
        bind: Option<&str>,
        api_origin: Option<&str>,
    ) -> Result<Self, std::net::AddrParseError> {
        Ok(Self {
            address: bind.unwrap_or(DEFAULT_BIND).parse()?,
            api_origin: api_origin.unwrap_or(DEFAULT_API_ORIGIN).to_owned(),
        })
    }

    fn from_env() -> Result<Self, std::net::AddrParseError> {
        let bind = env::var("FTNL_WEB_BIND").ok();
        let api_origin = env::var("FTNL_API_ORIGIN").ok();
        Self::from_values(bind.as_deref(), api_origin.as_deref())
    }
}

/// Run the File Tunnel portal until its shutdown signal completes.
///
/// # Errors
///
/// Returns an error when the configured bind address is malformed, the socket
/// cannot bind, or the Axum server exits unexpectedly.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ftnl_web_server=info,tower_http=info")),
        )
        .init();

    let telemetry = observability::logger();
    observability::event(&telemetry, "web.service.starting");
    let config = RuntimeConfig::from_env()?;
    let listener = TcpListener::bind(config.address).await?;
    info!(address = %config.address, "File Tunnel portal listening");
    observability::event(&telemetry, "web.service.listening");
    let result = axum::serve(listener, app(AppState::new(config.api_origin)))
        .with_graceful_shutdown(shutdown())
        .await;
    observability::event(&telemetry, "web.service.stopped");
    let _ = telemetry.close();
    result?;
    Ok(())
}

async fn shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_defaults_preserve_the_existing_local_contract() {
        assert_eq!(
            RuntimeConfig::from_values(None, None).expect("valid defaults"),
            RuntimeConfig {
                address: DEFAULT_BIND.parse().expect("valid default bind"),
                api_origin: DEFAULT_API_ORIGIN.to_owned(),
            }
        );
    }

    #[test]
    fn explicit_runtime_values_are_kept_separate() {
        assert_eq!(
            RuntimeConfig::from_values(Some("0.0.0.0:4310"), Some("https://api.example.test"))
                .expect("valid explicit values"),
            RuntimeConfig {
                address: "0.0.0.0:4310".parse().expect("valid test bind"),
                api_origin: "https://api.example.test".to_owned(),
            }
        );
    }

    #[test]
    fn malformed_bind_addresses_fail_before_listener_creation() {
        assert!(RuntimeConfig::from_values(Some("not-a-socket"), None).is_err());
    }

    #[test]
    fn executable_remains_a_thin_tokio_adapter() {
        let main = include_str!("main.rs");
        assert!(main.lines().count() <= 6);
        for lifecycle_symbol in [
            "TcpListener",
            "FTNL_WEB_BIND",
            "FTNL_API_ORIGIN",
            "tracing_subscriber",
            "axum::serve",
        ] {
            assert!(!main.contains(lifecycle_symbol));
        }
    }
}

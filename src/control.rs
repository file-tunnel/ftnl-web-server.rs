//! Account-authenticated File Tunnel status web surfaces.
//!
//! This is separate from the anonymous upload portal runtime. It never handles
//! pairing secrets, capabilities, filenames, or file bytes.

use async_trait::async_trait;
use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use ftnl_interfaces::TunnelStatus;
use futures_util::{SinkExt, StreamExt};
use maud::{html, Markup, DOCTYPE};
use ores_lib_core::{redact_value, valid_correlation_id};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use shared_auth_lib::{AuthOutcome, Guard};
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_util::codec::{Framed, LinesCodec};
use uuid::Uuid;

const MAX_AUTHORIZATION_BYTES: usize = 16 * 1024;
const MAX_FRAME_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const READ_PROJECTION_SQL: &str = "SELECT status, expires_at::text AS expires_at \
     FROM account_tunnel_summaries \
     WHERE tunnel_id = CAST($1 AS uuid) AND shared_user_id = $2 \
     LIMIT 1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    DirectRead,
    Http,
    Tcp,
    Nats,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TunnelSummary {
    pub tunnel_id: Uuid,
    pub status: TunnelStatus,
    pub expires_at: String,
    pub transport: TransportMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Actor {
    subject: String,
}

impl Actor {
    #[must_use]
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
        }
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthDecision {
    Authenticated(Actor),
    Anonymous,
    Unauthenticated,
    Degraded,
}

#[async_trait]
pub trait ControlAuthenticator: Send + Sync {
    async fn authorize(&self, headers: &HeaderMap) -> AuthDecision;
}

pub struct SharedAuthControlAuthenticator {
    guard: Guard,
}

impl SharedAuthControlAuthenticator {
    #[must_use]
    pub fn new(guard: Guard) -> Self {
        Self { guard }
    }
}

#[async_trait]
impl ControlAuthenticator for SharedAuthControlAuthenticator {
    async fn authorize(&self, headers: &HeaderMap) -> AuthDecision {
        match self.guard.check(headers).await {
            AuthOutcome::Authenticated { identity, .. } => {
                AuthDecision::Authenticated(Actor::new(identity.shared_user_id))
            }
            AuthOutcome::Anonymous => AuthDecision::Anonymous,
            AuthOutcome::Unauthenticated => AuthDecision::Unauthenticated,
            AuthOutcome::Degraded { .. } => AuthDecision::Degraded,
        }
    }
}

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("invalid backend request")]
    InvalidRequest,
    #[error("backend rejected request")]
    Rejected,
    #[error("backend unavailable")]
    Unavailable,
    #[error("tunnel not found")]
    NotFound,
}

#[async_trait]
pub trait TunnelBackend: Send + Sync {
    async fn lookup(
        &self,
        tunnel_id: Uuid,
        actor: &Actor,
        authorization: &str,
    ) -> Result<TunnelSummary, BackendError>;
}

pub struct DirectReadBackend {
    database: DatabaseConnection,
}

impl DirectReadBackend {
    /// The supplied database role must be SELECT-only at the Postgres layer.
    #[must_use]
    pub fn new_read_only(database: DatabaseConnection) -> Self {
        Self { database }
    }
}

#[async_trait]
impl TunnelBackend for DirectReadBackend {
    async fn lookup(
        &self,
        tunnel_id: Uuid,
        actor: &Actor,
        _authorization: &str,
    ) -> Result<TunnelSummary, BackendError> {
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            READ_PROJECTION_SQL,
            vec![tunnel_id.to_string().into(), actor.subject().into()],
        );
        let row = self
            .database
            .query_one(statement)
            .await
            .map_err(|_| BackendError::Unavailable)?
            .ok_or(BackendError::NotFound)?;
        let status: String = row
            .try_get("", "status")
            .map_err(|_| BackendError::Unavailable)?;
        let expires_at: String = row
            .try_get("", "expires_at")
            .map_err(|_| BackendError::Unavailable)?;
        Ok(TunnelSummary {
            tunnel_id,
            status: parse_status(&status)?,
            expires_at,
            transport: TransportMode::DirectRead,
        })
    }
}

fn parse_status(value: &str) -> Result<TunnelStatus, BackendError> {
    match value {
        "waiting" => Ok(TunnelStatus::Waiting),
        "connected" => Ok(TunnelStatus::Connected),
        "transferring" => Ok(TunnelStatus::Transferring),
        "complete" => Ok(TunnelStatus::Complete),
        "cancelled" => Ok(TunnelStatus::Cancelled),
        "expired" => Ok(TunnelStatus::Expired),
        _ => Err(BackendError::Unavailable),
    }
}

pub struct HttpBackend {
    client: reqwest::Client,
    base: reqwest::Url,
}

impl HttpBackend {
    pub fn new(base: &str) -> Result<Self, BackendError> {
        let base = reqwest::Url::parse(base).map_err(|_| BackendError::InvalidRequest)?;
        if base.scheme() != "https"
            && !(base.scheme() == "http"
                && matches!(base.host_str(), Some("127.0.0.1" | "localhost" | "::1")))
        {
            return Err(BackendError::InvalidRequest);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| BackendError::Unavailable)?;
        Ok(Self { client, base })
    }
}

#[async_trait]
impl TunnelBackend for HttpBackend {
    async fn lookup(
        &self,
        tunnel_id: Uuid,
        _actor: &Actor,
        authorization: &str,
    ) -> Result<TunnelSummary, BackendError> {
        validate_authorization(authorization)?;
        let endpoint = self
            .base
            .join(&format!("/v1/tunnels/{tunnel_id}"))
            .map_err(|_| BackendError::InvalidRequest)?;
        let response = self
            .client
            .get(endpoint)
            .header(AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(|_| BackendError::Unavailable)?;
        match response.status() {
            reqwest::StatusCode::UNAUTHORIZED => return Err(BackendError::Rejected),
            reqwest::StatusCode::NOT_FOUND => return Err(BackendError::NotFound),
            _ => {}
        }
        let response = response
            .error_for_status()
            .map_err(|_| BackendError::Unavailable)?;
        let body = response
            .bytes()
            .await
            .map_err(|_| BackendError::Unavailable)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(BackendError::Unavailable);
        }
        serde_json::from_slice(&body).map_err(|_| BackendError::Unavailable)
    }
}

#[derive(Serialize)]
struct ReadEnvelope<'a> {
    authorization: &'a str,
    request_id: &'a str,
    tunnel_id: Uuid,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum WireReply {
    Ok {
        request_id: String,
        response: TunnelSummary,
    },
    Error {
        request_id: Option<String>,
        error: String,
    },
}

pub struct TcpBackend {
    framed: Mutex<Framed<TcpStream, LinesCodec>>,
}

impl TcpBackend {
    pub async fn connect(address: &str) -> Result<Self, BackendError> {
        let stream = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(address))
            .await
            .map_err(|_| BackendError::Unavailable)?
            .map_err(|_| BackendError::Unavailable)?;
        Ok(Self {
            framed: Mutex::new(Framed::new(
                stream,
                LinesCodec::new_with_max_length(MAX_FRAME_BYTES),
            )),
        })
    }
}

#[async_trait]
impl TunnelBackend for TcpBackend {
    async fn lookup(
        &self,
        tunnel_id: Uuid,
        _actor: &Actor,
        authorization: &str,
    ) -> Result<TunnelSummary, BackendError> {
        validate_authorization(authorization)?;
        let request_id = Uuid::new_v4().simple().to_string();
        validate_request_id(&request_id)?;
        let payload = serde_json::to_string(&ReadEnvelope {
            authorization,
            request_id: &request_id,
            tunnel_id,
        })
        .map_err(|_| BackendError::InvalidRequest)?;
        let mut framed = self.framed.lock().await;
        tokio::time::timeout(Duration::from_secs(3), framed.send(payload))
            .await
            .map_err(|_| BackendError::Unavailable)?
            .map_err(|_| BackendError::Unavailable)?;
        let frame = tokio::time::timeout(Duration::from_secs(3), framed.next())
            .await
            .map_err(|_| BackendError::Unavailable)?
            .ok_or(BackendError::Unavailable)?
            .map_err(|_| BackendError::Unavailable)?;
        decode_reply(&frame, &request_id)
    }
}

pub struct NatsBackend {
    client: async_nats::Client,
    subject: String,
}

impl NatsBackend {
    #[must_use]
    pub fn new(client: async_nats::Client, subject: impl Into<String>) -> Self {
        Self {
            client,
            subject: subject.into(),
        }
    }
}

#[async_trait]
impl TunnelBackend for NatsBackend {
    async fn lookup(
        &self,
        tunnel_id: Uuid,
        _actor: &Actor,
        authorization: &str,
    ) -> Result<TunnelSummary, BackendError> {
        validate_authorization(authorization)?;
        let request_id = Uuid::new_v4().simple().to_string();
        validate_request_id(&request_id)?;
        let payload = serde_json::to_vec(&ReadEnvelope {
            authorization,
            request_id: &request_id,
            tunnel_id,
        })
        .map_err(|_| BackendError::InvalidRequest)?;
        let message = tokio::time::timeout(
            Duration::from_secs(3),
            self.client.request(self.subject.clone(), payload.into()),
        )
        .await
        .map_err(|_| BackendError::Unavailable)?
        .map_err(|_| BackendError::Unavailable)?;
        if message.payload.len() > MAX_RESPONSE_BYTES {
            return Err(BackendError::Unavailable);
        }
        let frame = std::str::from_utf8(&message.payload).map_err(|_| BackendError::Unavailable)?;
        decode_reply(frame, &request_id)
    }
}

fn decode_reply(frame: &str, request_id: &str) -> Result<TunnelSummary, BackendError> {
    match serde_json::from_str(frame).map_err(|_| BackendError::Unavailable)? {
        WireReply::Ok {
            request_id: reply_id,
            response,
        } if reply_id == request_id => Ok(response),
        WireReply::Error {
            request_id: reply_id,
            error,
        } => {
            if reply_id.as_deref() != Some(request_id) {
                return Err(BackendError::Unavailable);
            }
            match error.as_str() {
                "unauthorized" => Err(BackendError::Rejected),
                "not_found" => Err(BackendError::NotFound),
                "invalid_request" => Err(BackendError::InvalidRequest),
                _ => Err(BackendError::Unavailable),
            }
        }
        _ => Err(BackendError::Unavailable),
    }
}

fn validate_authorization(value: &str) -> Result<(), BackendError> {
    if value.len() > MAX_AUTHORIZATION_BYTES || !value.starts_with("Bearer ") {
        return Err(BackendError::Rejected);
    }
    Ok(())
}

fn validate_request_id(value: &str) -> Result<(), BackendError> {
    valid_correlation_id(value)
        .then_some(())
        .ok_or(BackendError::InvalidRequest)
}

#[derive(Clone)]
pub struct ControlState {
    auth: Arc<dyn ControlAuthenticator>,
    backend: Arc<dyn TunnelBackend>,
}

impl ControlState {
    #[must_use]
    pub fn new(auth: Arc<dyn ControlAuthenticator>, backend: Arc<dyn TunnelBackend>) -> Self {
        Self { auth, backend }
    }
}

pub fn control_router(state: ControlState) -> Router {
    Router::new()
        .route("/healthz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/readyz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/control/tunnels/{tunnel_id}", get(full_tunnel))
        .route("/control/pagelets/tunnels/{tunnel_id}", get(tunnel_pagelet))
        .route(
            "/control/islands/tunnel-status/{tunnel_id}",
            get(tunnel_island),
        )
        .with_state(state)
}

async fn full_tunnel(
    State(state): State<ControlState>,
    Path(tunnel_id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    match load_tunnel(&state, &headers, tunnel_id).await {
        Ok(summary) => secure_html(render_full(&summary)),
        Err(error) => error.into_response(),
    }
}

async fn tunnel_pagelet(
    State(state): State<ControlState>,
    Path(tunnel_id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    match load_tunnel(&state, &headers, tunnel_id).await {
        Ok(summary) => secure_html(render_summary(&summary)),
        Err(error) => error.into_response(),
    }
}

async fn tunnel_island(
    State(state): State<ControlState>,
    Path(tunnel_id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    match load_tunnel(&state, &headers, tunnel_id).await {
        Ok(summary) => secure_html(render_island(&summary)),
        Err(error) => error.into_response(),
    }
}

async fn load_tunnel(
    state: &ControlState,
    headers: &HeaderMap,
    tunnel_id: Uuid,
) -> Result<TunnelSummary, ControlError> {
    let actor = match state.auth.authorize(headers).await {
        AuthDecision::Authenticated(actor) => actor,
        AuthDecision::Anonymous | AuthDecision::Unauthenticated => {
            return Err(ControlError::Unauthorized);
        }
        AuthDecision::Degraded => return Err(ControlError::AuthUnavailable),
    };
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ControlError::Unauthorized)?;
    state
        .backend
        .lookup(tunnel_id, &actor, authorization)
        .await
        .map_err(ControlError::Backend)
}

#[derive(Debug, Error)]
enum ControlError {
    #[error("authentication required")]
    Unauthorized,
    #[error("authentication unavailable")]
    AuthUnavailable,
    #[error("backend failed")]
    Backend(BackendError),
}

impl IntoResponse for ControlError {
    fn into_response(self) -> Response {
        let (status, title, message) = match self {
            Self::Unauthorized | Self::Backend(BackendError::Rejected) => (
                StatusCode::UNAUTHORIZED,
                "Sign in required",
                "A valid File Tunnel session is required.",
            ),
            Self::Backend(BackendError::InvalidRequest) => (
                StatusCode::BAD_REQUEST,
                "Invalid request",
                "The tunnel request is invalid.",
            ),
            Self::Backend(BackendError::NotFound) => (
                StatusCode::NOT_FOUND,
                "Tunnel not found",
                "No authorized tunnel summary matched this request.",
            ),
            Self::AuthUnavailable | Self::Backend(BackendError::Unavailable) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Temporarily unavailable",
                "The authority required to decide this request is unavailable.",
            ),
        };
        let body = html! {
            (DOCTYPE)
            html lang="en" { head { meta charset="utf-8"; title { (title) } }
                body { main { h1 { (title) } p { (message) } } } }
        };
        let mut response = (status, Html(body.into_string())).into_response();
        no_store(response.headers_mut());
        response
    }
}

fn secure_html(markup: Markup) -> Response {
    let mut response = Html(markup.into_string()).into_response();
    no_store(response.headers_mut());
    response
}

fn no_store(headers: &mut HeaderMap) {
    headers.insert(
        "cache-control",
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert("vary", HeaderValue::from_static("authorization"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
}

fn render_full(summary: &TunnelSummary) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head { meta charset="utf-8"; meta name="viewport" content="width=device-width, initial-scale=1";
                title { "File Tunnel status" } }
            body { main id="main-content" { (render_summary(summary)) } }
        }
    }
}

fn render_summary(summary: &TunnelSummary) -> Markup {
    html! {
        section id="tunnel-summary" data-transport=(format!("{:?}", summary.transport).to_lowercase()) {
            h1 { "File Tunnel" }
            dl {
                dt { "Tunnel" } dd { (summary.tunnel_id) }
                dt { "Status" } dd { (format!("{:?}", summary.status).to_lowercase()) }
                dt { "Expires" } dd { (summary.expires_at) }
            }
        }
    }
}

fn render_island(summary: &TunnelSummary) -> Markup {
    html! {
        aside data-island="tunnel-status" aria-live="polite" {
            span class="tunnel-status" { (format!("{:?}", summary.status).to_lowercase()) }
        }
    }
}

#[must_use]
pub fn redacted_diagnostic<'a>(key: &str, value: &'a str) -> &'a str {
    redact_value(key, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    const TUNNEL_ID: &str = "018f47d2-2d9f-7a41-a2aa-1aef7d847001";

    struct FakeAuth;
    #[async_trait]
    impl ControlAuthenticator for FakeAuth {
        async fn authorize(&self, headers: &HeaderMap) -> AuthDecision {
            if headers.get(AUTHORIZATION).is_some() {
                AuthDecision::Authenticated(Actor::new("actor-1"))
            } else {
                AuthDecision::Anonymous
            }
        }
    }

    struct FakeBackend {
        mode: TransportMode,
    }
    #[async_trait]
    impl TunnelBackend for FakeBackend {
        async fn lookup(
            &self,
            tunnel_id: Uuid,
            actor: &Actor,
            authorization: &str,
        ) -> Result<TunnelSummary, BackendError> {
            assert_eq!(actor.subject(), "actor-1");
            assert_eq!(authorization, "Bearer synthetic");
            Ok(TunnelSummary {
                tunnel_id,
                status: TunnelStatus::Waiting,
                expires_at: "2026-08-25T17:00:00Z <unsafe>".to_owned(),
                transport: self.mode,
            })
        }
    }

    fn app(mode: TransportMode) -> Router {
        control_router(ControlState::new(
            Arc::new(FakeAuth),
            Arc::new(FakeBackend { mode }),
        ))
    }

    async fn get(path: &str, authenticated: bool, mode: TransportMode) -> Response {
        let mut request = Request::get(path);
        if authenticated {
            request = request.header(AUTHORIZATION, "Bearer synthetic");
        }
        app(mode)
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn renders_full_ssr_pagelet_and_island_with_escaping() {
        let full = get(
            &format!("/control/tunnels/{TUNNEL_ID}"),
            true,
            TransportMode::Http,
        )
        .await;
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()["cache-control"], "private, no-store");
        let body = axum::body::to_bytes(full.into_body(), MAX_RESPONSE_BYTES)
            .await
            .expect("body");
        let body = String::from_utf8(body.to_vec()).expect("UTF-8");
        assert!(body.starts_with("<!DOCTYPE html>"));
        assert!(body.contains("&lt;unsafe&gt;"));

        let pagelet = get(
            &format!("/control/pagelets/tunnels/{TUNNEL_ID}"),
            true,
            TransportMode::Tcp,
        )
        .await;
        let pagelet = axum::body::to_bytes(pagelet.into_body(), MAX_RESPONSE_BYTES)
            .await
            .expect("pagelet");
        assert!(String::from_utf8(pagelet.to_vec())
            .expect("UTF-8")
            .starts_with("<section"));

        let island = get(
            &format!("/control/islands/tunnel-status/{TUNNEL_ID}"),
            true,
            TransportMode::Nats,
        )
        .await;
        let island = axum::body::to_bytes(island.into_body(), MAX_RESPONSE_BYTES)
            .await
            .expect("island");
        assert!(String::from_utf8(island.to_vec())
            .expect("UTF-8")
            .contains("data-island=\"tunnel-status\""));
    }

    #[tokio::test]
    async fn missing_auth_fails_closed() {
        let response = get(
            &format!("/control/tunnels/{TUNNEL_ID}"),
            false,
            TransportMode::DirectRead,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn every_backend_mode_is_explicitly_preserved() {
        for mode in [
            TransportMode::DirectRead,
            TransportMode::Http,
            TransportMode::Tcp,
            TransportMode::Nats,
        ] {
            let response = get(&format!("/control/tunnels/{TUNNEL_ID}"), true, mode).await;
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[test]
    fn remote_cleartext_http_is_rejected() {
        assert!(HttpBackend::new("http://api.example.test").is_err());
        assert!(HttpBackend::new("http://127.0.0.1:8080").is_ok());
        assert!(HttpBackend::new("https://api.example.test").is_ok());
    }

    #[test]
    fn envelopes_are_bounded_correlated_and_redacted() {
        assert!(validate_authorization("Bearer synthetic").is_ok());
        assert!(validate_authorization("synthetic").is_err());
        assert!(validate_request_id("request-0001").is_ok());
        assert!(validate_request_id("bad id").is_err());
        assert_eq!(redacted_diagnostic("authorization", "secret"), "[REDACTED]");
    }

    #[test]
    fn direct_projection_is_select_only() {
        let statement = READ_PROJECTION_SQL.to_ascii_uppercase();
        assert!(statement.trim_start().starts_with("SELECT "));
        for prohibited in [
            "INSERT INTO",
            "UPDATE ",
            "DELETE FROM",
            "DROP ",
            "TRUNCATE ",
        ] {
            assert!(!statement.contains(prohibited), "found {prohibited}");
        }
    }
}

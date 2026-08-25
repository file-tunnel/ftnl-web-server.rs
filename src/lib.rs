pub mod control;
mod observability;
pub mod runtime;

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use uuid::Uuid;

const HTML: &str = include_str!("../web/index.html");
const JAVASCRIPT: &str = include_str!("../web/app.js");
const CSS: &str = include_str!("../web/app.css");
const MANIFEST: &str = include_str!("../web/manifest.webmanifest");

#[derive(Clone)]
pub struct AppState {
    api_origin: Arc<str>,
}

impl AppState {
    pub fn new(api_origin: impl Into<String>) -> Self {
        Self {
            api_origin: Arc::from(api_origin.into().trim_end_matches('/').to_owned()),
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/t/{tunnel_id}", get(tunnel))
        .route("/config.js", get(config))
        .route("/assets/app.js", get(javascript))
        .route("/assets/app.css", get(css))
        .route("/manifest.webmanifest", get(manifest))
        .route("/healthz", get(health))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::new(
            header::HeaderName::from_static("x-request-id"),
            MakeRequestUuid,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn index() -> Response {
    secure_html(Html(HTML))
}

async fn tunnel(Path(_tunnel_id): Path<Uuid>) -> Response {
    secure_html(Html(HTML))
}

async fn config(State(state): State<AppState>) -> Response {
    let encoded = encode_runtime_config_string(&state.api_origin);
    let body = format!("globalThis.__FTNL_CONFIG__ = Object.freeze({{ apiOrigin: {encoded} }});\n");
    asset_response(
        body,
        "application/javascript; charset=utf-8",
        "no-store, max-age=0",
    )
}

async fn javascript() -> Response {
    asset_response(
        JAVASCRIPT,
        "application/javascript; charset=utf-8",
        "public, max-age=300",
    )
}

async fn css() -> Response {
    asset_response(CSS, "text/css; charset=utf-8", "public, max-age=300")
}

async fn manifest() -> Response {
    asset_response(
        MANIFEST,
        "application/manifest+json; charset=utf-8",
        "public, max-age=3600",
    )
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"status":"ok"}"#,
    )
}

fn secure_html(body: Html<&'static str>) -> Response {
    let mut response = body.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; connect-src http://127.0.0.1:8080 ws://127.0.0.1:8080 https: wss:; font-src 'self'; manifest-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=()",
        ),
    );
    headers.insert(
        header::HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn asset_response(
    body: impl Into<axum::body::Body>,
    content_type: &'static str,
    cache_control: &'static str,
) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
            (
                header::HeaderName::from_static("x-content-type-options"),
                "nosniff",
            ),
        ],
        body.into(),
    )
        .into_response()
}

/// Encodes an untrusted runtime value as one JSON-compatible JavaScript string.
#[must_use]
pub fn encode_runtime_config_string(value: &str) -> String {
    serde_json::to_string(value)
        .expect("serializing a string cannot fail")
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_cannot_break_out_of_script_string() {
        let encoded = encode_runtime_config_string("</script><script>alert(1)</script>");
        assert!(!encoded.contains('<'));
        assert!(encoded.contains("\\u003c/script>"));
    }

    proptest::proptest! {
        #[test]
        fn runtime_config_encoding_round_trips_every_utf8_string(value in ".*") {
            let encoded = encode_runtime_config_string(&value);
            let decoded: String = serde_json::from_str(&encoded).unwrap();
            proptest::prop_assert_eq!(decoded, value);
            proptest::prop_assert!(!encoded.contains('<'));
            let line_separator = char::from_u32(0x2028).unwrap();
            let paragraph_separator = char::from_u32(0x2029).unwrap();
            proptest::prop_assert!(!encoded.contains(line_separator));
            proptest::prop_assert!(!encoded.contains(paragraph_separator));
        }
    }

    #[test]
    fn tunnel_route_requires_uuid_shape() {
        assert!("018f47d2-2d9f-7a41-a2aa-1aef7d847001"
            .parse::<Uuid>()
            .is_ok());
        assert!("not-a-tunnel".parse::<Uuid>().is_err());
    }

    #[test]
    fn zed_package_declares_the_schema_tooling_boundary() {
        let manifest = include_str!("../.zpkg.toml");
        assert!(manifest.contains("\"file-tunnel/ftnl-lib-core\" = \"^0.1.0\""));
    }
}

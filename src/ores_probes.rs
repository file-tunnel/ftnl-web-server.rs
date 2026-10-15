//! Service runtime probe contract — `ores.service.runtime.v1` (Linear DEN-3443).
//!
//! * `/healthz` reports process liveness only. It never blocks on a dependency,
//!   so a slow database can never make Kubernetes restart a healthy process.
//! * `/readyz` fails closed. It answers 503 until the process declares its
//!   dependencies usable, and returns to 503 the moment drain begins, so a load
//!   balancer stops sending work before the listener closes.
//! * `/version` exposes non-secret build identity only.
//!
//! A service with downstream dependencies (database, cache, object store) must
//! call [`set_ready`] with `false` during startup and pass `true` only after
//! those dependencies have actually been probed, and must call [`begin_drain`]
//! from its shutdown-signal handler. Readiness must never report success
//! because a dependency check timed out or was skipped.

use std::sync::atomic::{AtomicBool, Ordering};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;

/// Name reported by every probe response.
pub const SERVICE: &str = "ftnl-web-server";
/// Version of the probe contract these routes implement.
pub const CONTRACT_VERSION: &str = "ores.service.runtime.v1";

// Readiness starts closed. A process that has not yet proved its dependencies
// usable must not be routable, so the flag opens only through `set_ready(true)`.
static READY: AtomicBool = AtomicBool::new(false);
static DRAINING: AtomicBool = AtomicBool::new(false);

/// Declare whether the service's dependency checks have passed.
pub fn set_ready(ready: bool) {
    READY.store(ready, Ordering::Release);
}

/// Mark the process as draining; readiness fails closed from this point on.
pub fn begin_drain() {
    DRAINING.store(true, Ordering::Release);
}

/// True only when the process is not draining and dependencies are declared usable.
pub fn is_ready() -> bool {
    !DRAINING.load(Ordering::Acquire) && READY.load(Ordering::Acquire)
}

/// Probe routes, generic over the application state so they compose with any
/// `Router<S>` through `.merge(...)` regardless of where state is attached.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route("/version", get(version))
}

async fn version() -> Response {
    (
        StatusCode::OK,
        [("cache-control", "no-store")],
        Json(json!({
            "service": SERVICE,
            "contractVersion": CONTRACT_VERSION,
            "version": env!("CARGO_PKG_VERSION"),
            "gitSha": option_env!("GIT_SHA"),
            "buildTimestamp": option_env!("BUILD_TIMESTAMP"),
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // READY and DRAINING are process-global and cargo runs tests on parallel
    // threads, so every test that writes them holds this lock throughout.
    static PROBE_STATE: Mutex<()> = Mutex::new(());

    #[test]
    fn readiness_fails_closed_once_drain_starts() {
        let _guard = PROBE_STATE.lock().unwrap_or_else(|e| e.into_inner());
        set_ready(true);
        assert!(is_ready());
        begin_drain();
        assert!(!is_ready(), "drain must make readiness fail closed");
        DRAINING.store(false, Ordering::Release);
    }

    #[test]
    fn readiness_fails_closed_until_dependencies_are_declared() {
        let _guard = PROBE_STATE.lock().unwrap_or_else(|e| e.into_inner());
        DRAINING.store(false, Ordering::Release);
        set_ready(false);
        assert!(!is_ready());
        set_ready(true);
        assert!(is_ready());
    }
}

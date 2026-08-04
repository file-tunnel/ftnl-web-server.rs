# File Tunnel portal runtime

The portal is a static/browser-facing Axum service. It does not own tunnel
creation, upload authorization, object storage, relay traffic, or API secrets.

## Ownership

- `src/main.rs` is the Tokio executable adapter only.
- `src/runtime.rs` owns tracing, environment configuration, listener binding,
  graceful shutdown, and process-level error propagation.
- `src/lib.rs` owns the HTTP application, state, static assets, runtime config
  encoding, request IDs, browser security headers, and route handlers.

`FTNL_WEB_BIND` and `FTNL_API_ORIGIN` are parsed before socket creation. Their
existing defaults remain `127.0.0.1:3000` and `http://127.0.0.1:8080`.
Malformed bind addresses fail before any listener or background task exists.

## Security boundary

The extraction does not change CSP, Permissions Policy, request IDs, cache
headers, UUID path validation, runtime JavaScript encoding, or the API-origin
normalization performed by `AppState`.

The runtime must not gain tunnel credentials, provider clients, database
connections, object-storage access, or upload authorization. Those remain in
the File Tunnel API/control plane.

## Regression gate

Permanent CI requires:

- a six-line-or-smaller executable;
- formatting and warnings-denied Clippy;
- all locked unit/property tests;
- Rustdoc with warnings denied and a locked release build;
- source checks preventing configuration, listener, Axum serving, or tracing
  initialization from returning to `src/main.rs`.

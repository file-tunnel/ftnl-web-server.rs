# ftnl-web-server.rs

Rust mobile upload portal for [File Tunnel](https://github.com/file-tunnel).
Scanning a desktop QR opens this responsive, install-free page on the phone;
the user chooses photos or files and sees upload progress while the desktop
receives the same transfer events.

The portal is intentionally small:

- no account, cookies, analytics, third-party scripts, or application install;
- the pairing secret is read from `#c=...` and removed from the visible URL
  before any API request;
- the one-time secret is exchanged for a phone-scoped capability;
- capabilities live in `sessionStorage`, not persistent storage;
- portal HTML and configuration are `no-store`;
- restrictive CSP, Permissions Policy, and `Referrer-Policy: no-referrer`;
- XHR upload is used because it exposes reliable byte progress across mobile
  browsers;
- file bytes are never cached by this server or a service worker.

## Run locally

Start `ftnl-backend-api.rs` on port 8080, then:

```bash
nix develop
cp .env.example .env
cargo run
```

Open a pairing URI returned by the backend, for example
`http://127.0.0.1:3000/t/{uuid}#c={secret}`.

## Configuration

- `FTNL_WEB_BIND` defaults to `127.0.0.1:3000`.
- `FTNL_API_ORIGIN` defaults to `http://127.0.0.1:8080`.

Production should serve the portal only over HTTPS and configure the backend's
CORS allowlist to the exact portal origin.

Production environment bundles use SOPS ciphertext in `env/enc` and ignored
plaintext in `env/dec`. Run `nix develop --command just edit prod` only with
freshly rotated provider credentials. Supabase publishable configuration may
reach the portal; secret/service-role credentials must never do so.

The Zed package graph imports `ftnl-lib-core` for shared generated schema
artifacts without linking its SQL/ORM engine into the portal binary. Service
lifecycle records use the ORES OpenTelemetry wrapper and contain constant event
names only; request URLs, tunnel/file identifiers, capabilities, filenames,
remote addresses, and bytes are excluded.

## Validate

```bash
nix develop --command agent-check
```

The dedicated formal-boundary workflow uses randomized UTF-8 inputs to prove
that runtime configuration is JSON-round-trippable and cannot emit raw script
terminators or JavaScript line separators.

The checked-in Nix lock provides Rust, Cargo, formatters, linters, and CI
tooling on macOS and Linux.

MIT licensed.

## Cross-surface delivery

Changes to pairing, transfer state, progress, validation, permissions,
notifications, imports, navigation, or deep links in this Rust mobile-web portal
must be evaluated for the planned first-class clients:

- `file-tunnel/ftnl-flutter` for Android, iOS, Flutter Web/mobile web, and
  Flutter desktop;
- `file-tunnel/ftnl-desktop.rs` for the native GPUI desktop app; and
- File Tunnel interfaces, clients, transfer manifests, route types, and
  interruption/resume fixtures.

This is not automatic screen-for-screen parity. Install-free upload UX and
browser-specific picker behavior may remain web-only. Native tray, filesystem,
drag/drop, notifications, and background-transfer behavior may remain
native-only. Transfer identity, capability semantics, status/progress,
integrity, retry/resume, authorization, and user-directed navigation normally
require coordinated work or an explicit no-change rationale and parity issue.

Deep links are HTTPS-first:

```text
https://<verified-file-tunnel-owned-host>/open/<route>?<bounded-query>
```

with `ftnl://` fallback. Rust web, Flutter, and GPUI must share one versioned
route model and support cold start, already-running delivery, authentication or
pairing resume, replay/expiry rejection, and browser fallback. File bytes,
absolute private paths, transfer capabilities, credentials, encryption keys,
and bearer tokens are prohibited in URLs. Pairing and transfer handoffs use
short-lived, single-use, audience-bound codes.

See [`docs/CROSS_SURFACE_DELIVERY.md`](docs/CROSS_SURFACE_DELIVERY.md) and the
[portfolio policy](https://github.com/ORESoftware/project-registry/blob/main/docs/cross-surface-delivery.md).

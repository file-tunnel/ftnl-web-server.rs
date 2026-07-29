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

## Validate

```bash
nix develop --command agent-check
```

The checked-in Nix lock provides Rust, Cargo, formatters, linters, and CI
tooling on macOS and Linux.

MIT licensed.

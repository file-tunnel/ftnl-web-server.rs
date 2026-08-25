# File Tunnel web server agent instructions

These instructions apply to this repository and every directory beneath it.

## Repository role

- This repository owns the install-free mobile upload portal.
- The optional `ftnl-control-web` binary is a separate account-authenticated,
  metadata-only surface. Its database, API, TCP, and NATS providers must never
  enter the anonymous portal runtime or receive pairing/capability material.
- Read the one-time pairing secret only from the URI fragment, scrub it from
  the visible URL before network activity, and exchange it for a phone-scoped
  capability.
- Keep capabilities session-scoped and out of URLs, logs, telemetry, caches,
  persistent browser storage, and rendered error details.
- Preserve no-store responses, restrictive CSP and permissions headers,
  no-referrer behavior, exact-origin CORS expectations, safe runtime-config
  encoding, and cancellation/expiry failure boundaries.
- Do not add analytics, third-party runtime scripts, service-worker file
  caching, or server-side storage of user file bytes.

## Validation

- Run `nix develop --command agent-check` before completing a change.
- Run the cross-browser E2E suite in `ftnl-e2e` for user-visible transfer or
  browser security behavior changes.
- Never commit credentials, runtime `.env` files, user content, or build
  output.

## Git workflow

- Keep changes focused and reviewable.
- Pull and merge remote work before pushing; avoid git rebase in favor of git merge.
- Never discard unrelated or uncommitted user work.

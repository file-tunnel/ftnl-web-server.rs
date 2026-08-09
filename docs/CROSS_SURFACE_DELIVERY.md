# Cross-surface delivery

Verified **2026-08-06**.

## Surfaces

- Rust mobile-web portal: `file-tunnel/ftnl-web-server.rs`
- Flutter Android/iOS, Flutter Web, and Flutter desktop: `file-tunnel/ftnl-flutter` — planned
- Rust desktop: `file-tunnel/ftnl-desktop.rs` — planned GPUI/no-WebView app
- Shared contracts: File Tunnel interfaces, clients, transfer manifests, route types, capability and interruption/resume fixtures

## Judgment-based propagation

Evaluate every user-visible or contract-changing portal change across mobile, Flutter Web, Flutter desktop, GPUI desktop, and shared contracts. Browser file-pickers and install-free upload presentation may remain web-only. Tray, filesystem, drag/drop, background execution, and native notifications may remain native-only. Pairing, capability, transfer identity, progress/status, integrity, retry/resume, authorization, error, and navigation changes normally propagate or require an explicit rationale and parity follow-up.

## Deep links

```text
https://<verified-file-tunnel-owned-host>/open/<route>?<bounded-query>
ftnl://<route>?<bounded-query>
```

The HTTPS host must be verified before publication. All surfaces share versioned route types and golden fixtures and support cold start, already-running delivery, pairing/authentication resume, replay and expiry rejection, and browser fallback.

Never place file bytes, private absolute paths, capabilities, credentials, encryption keys, bearer tokens, or sensitive metadata in URLs. Use short-lived, single-use, audience-bound codes. Validate route version, transfer/share IDs, action, destination, authorization, bounds, and user intent; require confirmation before importing, downloading, overwriting, revealing, or moving files.

## Review checklist

- [ ] Flutter Android/iOS impact evaluated.
- [ ] Flutter Web/mobile-web impact evaluated.
- [ ] Flutter desktop impact evaluated.
- [ ] GPUI Rust desktop impact evaluated.
- [ ] Shared transfer/client/route/fixture impact evaluated.
- [ ] Deep-link and pairing compatibility tested where relevant.
- [ ] Omitted surfaces have a rationale and follow-up when needed.

## Routing

- GitHub Project: [`file-tunnel-project` — Project 1](https://github.com/orgs/file-tunnel/projects/1)
- Linear project: [`github.com/file-tunnel`](https://linear.app/denman/project/githubcomfile-tunnel-f46884af1012)
- Central policy: [`cross-surface-delivery.md`](https://github.com/ORESoftware/project-registry/blob/main/docs/cross-surface-delivery.md)
- Desktop registry: [`desktop-applications.json`](https://github.com/ORESoftware/project-registry/blob/main/registry/desktop-applications.json)

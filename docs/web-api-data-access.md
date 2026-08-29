# Web/API data-access decision

The File Tunnel portal adopts the
[portfolio four-path ADR](https://github.com/ORESoftware/k8s-cluster/blob/main/docs/architecture/web-api-data-access.md)
for [ORESoftware/k8s-cluster#1399](https://github.com/ORESoftware/k8s-cluster/issues/1399)
and [DEN-3960](https://linear.app/denman/issue/DEN-3960/document-4-web-server-to-api-server-data-access-patterns-across-10).
The portal is intentionally a static browser bootstrap, not a data authority.

## Current boundary

`ftnl-web-server` serves HTML, assets, and a no-store runtime API origin. It has
no database, product-data credential, server-side session, file cache, or API
proxy. The browser exchanges the one-time fragment secret for a phone-scoped
capability, keeps that capability in `sessionStorage`, and talks directly to the
File Tunnel API.

| Operation | Path | Decision |
| --- | --- | --- |
| Claim a tunnel and create an upload descriptor | P2: stateless HTTP | Browser calls the configured API origin; capability and request validation belong to the API. |
| Upload bytes to the descriptor URL | P2: bounded HTTP PUT | XHR provides progress, but one HTTP request is not P3. The portal never receives or stores the bytes. |
| Direct database read | Not deployed | The portal has no database dependency or credential. |
| Persistent web-to-API connection | Not deployed | CSP allowance for `wss:` is not evidence of a P3 implementation. |
| NATS/MQ command | Not deployed | Browser and portal never receive broker credentials. |

## Path 1: constrained direct reads

P1 is prohibited for this install-free portal. Browser code can never hold a
database credential. Adding server-side direct reads would require a separate
reviewed service boundary, distinct read-only role with no DML/DDL/ownership or
`BYPASSRLS`, stable allow-listed views, verified tenant scope, cross-tenant
negative tests, bounded pool/query timeout, cancellation, and an explicit stale
read contract. P2 remains the default for authoritative transfer state.

## Path 2: stateless HTTP

P2 is the deployed path. Production uses HTTPS, an exact portal-origin CORS
allowlist, bounded versioned routes, and the short-lived phone capability. The
pairing secret is read from the URL fragment and removed before any request;
capabilities, file bytes, private paths, credentials, keys, and bearer tokens
must not appear in URLs, telemetry, or persistent browser storage.

Upload-descriptor creation sends an `Idempotency-Key`; any automatic retry must
reuse that same value. The content PUT uses the API-issued file URL and must
resume or repeat only according to the backend's integrity contract. Clients
bound file size, response size, connect/idle/total lifetime, and concurrent
uploads; cancellation aborts the request. Retry only transient failures with a
total budget, capped jitter, and `Retry-After`. Pairing, authorization,
validation, expiry, and integrity failures are not blindly retried.

Browser trace context and request IDs may propagate only if the API explicitly
allows those headers. Record route template, status class, latency, timeout,
retry and byte-count buckets, never raw URLs, tunnel/file identifiers,
capabilities, filenames, remote addresses, or payload bytes. HTTP/API failure is
shown explicitly and never falls back to local file persistence or another
transport.

## Path 3: bounded stateful API connection

No P3 connection exists. XHR upload progress describes one bounded P2 request;
it is not a persistent web-server-to-API session. If live transfer events later
require P3, cap connections per client and API pod, authenticate the handshake,
set connect/idle/lifetime deadlines, heartbeat, bound both directions, reconnect
with jitter, and drain cleanly. Lost/overflowed events force an authoritative P2
resync. A P3 failure never exposes a broader capability or switches to P1.

## Path 4: asynchronous NATS or message queue

No P4 path exists in this portal. A browser must never connect directly to NATS
or receive a broker credential. A future API-owned P4 command needs a versioned
envelope, tenant/actor identity, trace context, stable message/idempotency ID,
bounded metadata-only payload, durable consumer, commit-before-ack, retry budget,
dead-letter policy, graceful drain, and queue age/redelivery/DLQ metrics. Broker
acceptance is not completion; the browser reads authoritative status through P2.
File bytes and capabilities do not belong in broker messages.

## Consistency, backpressure, and shutdown

- The API owns transfer state. The portal owns only presentation and ephemeral
  browser capability storage.
- P2 returns the API's committed or accepted result. Future P3 events are hints;
  future P4 publication is acceptance, not completion.
- The browser bounds concurrent uploads and the API must reject excess work.
  Neither side creates an unbounded in-memory queue.
- Portal shutdown is stateless. Browser navigation/abort cancels in-flight work;
  resumability follows the API contract rather than hidden local persistence.

## Schema and migrations

This repository owns no product database schema and runs no migration. The Zed
package graph imports `file-tunnel/ftnl-lib-core` only for shared generated
schema artifacts; it does not link SQL or ORM machinery into the portal. The
File Tunnel backend remains the persistence authority and applies its schema
through a separately reviewed, one-shot migration identity.

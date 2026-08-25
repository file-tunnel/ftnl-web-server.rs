# Account control web and four API modes

`ftnl-web-server` contains two intentionally separate binaries:

- `ftnl-web-server` remains the install-free, anonymous, capability-based
  upload portal. It has no database, Shared Auth, broker, or API proxy.
- `ftnl-control-web` is an account-authenticated, metadata-only status surface.
  It never receives pairing secrets, transfer capabilities, filenames, file
  bytes, or object-storage credentials.

The control binary renders full SSR at `/control/tunnels/{uuid}`, an
HTMX-compatible pagelet at `/control/pagelets/tunnels/{uuid}`, and an island at
`/control/islands/tunnel-status/{uuid}`. All protected responses are private,
`no-store`, and vary on authorization.

Set `FTNL_CONTROL_MODE` to exactly one mode. Selection occurs at startup and
there is no automatic fallback.

| Mode | Required setting | Contract |
| --- | --- | --- |
| `direct_read` | `FTNL_READONLY_DATABASE_URL` | SELECT-only Postgres role reads the allow-listed `account_tunnel_summaries` view, filtered by verified Shared Auth subject |
| `http` | `FTNL_API_BASE_URL` | stateless HTTPS GET, three-second deadline, no redirects, bounded JSON response |
| `tcp` | `FTNL_API_TCP_ADDR` | one reused 16 KiB newline-framed connection, fresh request ID and bearer verification per frame |
| `nats` | `FTNL_NATS_URL` | bounded asynchronous request/reply on `ftnl.tunnel.read.v1` with a three-second deadline |

Shared Auth local JWKS verification is the normal inbound path. Authentication
does not grant product access: direct mode filters the product-owned projection,
while HTTP/TCP/NATS cause the API to repeat both authentication and product
authorization. Degraded authority status fails closed.

Remote Shared Auth and API HTTP endpoints require TLS. TCP is loopback-only
unless `FTNL_TRUSTED_MESH_TCP=true` attests to a separately reviewed mTLS
service mesh. That setting does not itself provide encryption.

Core NATS request/reply is at-most-once and is used only for an idempotent read.
Future durable work requires a transactional outbox plus JetStream, stable
operation and content IDs, deduplication, retention, explicit acknowledgements,
bounded redelivery, dead-letter handling, and commit-before-ack. Broker
acceptance is not business completion.

ORES helpers validate bounded correlation IDs and redact sensitive diagnostic
fields. Logs contain mode and coarse outcome only—never authorization headers,
cookies, identities, tunnel IDs, filenames, payloads, database URLs, or raw
request URLs.

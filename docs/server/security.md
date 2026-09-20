# Security & Production Checklist

Recached ships **insecure by default**, like Redis: no password, no TLS, no IP restrictions. That is
convenient for `localhost` development and dangerous anywhere else. This page is the checklist to
work through before exposing it.

::: danger Never expose an unauthenticated Recached to the internet
With no `RECACHED_PASSWORD`, anyone who can reach port 6379 can read every key, write any key, and
run `FLUSHDB`. This is the single most common way self-hosted caches get compromised.
:::

## Minimum production checklist

- [ ] **Set `RECACHED_PASSWORD`** to a long random secret.
- [ ] **Bind to a private interface** — `RECACHED_BIND=127.0.0.1` or a VPC address, never `0.0.0.0`
      on a public host.
- [ ] **Enable TLS** (`RECACHED_TLS_CERT` + `RECACHED_TLS_KEY`) if traffic crosses any network you do
      not control. This covers the RESP, WebSocket and replication listeners; replicas additionally
      need `RECACHED_REPL_TLS_CA` — see [Replication](#replication).
- [ ] **Firewall the metrics port** — it has no authentication of its own, and
      `RECACHED_ALLOW_IPS` does not apply to it.
- [ ] **Set `RECACHED_SYNC_SECRET`** before any browser connects to a multi-tenant deployment.
- [ ] **Set `RECACHED_ALLOWED_ORIGINS`** to the origins your application is served from, so that
      other pages in a user's browser cannot open the sync socket.
- [ ] **Set `RECACHED_MAX_MEMORY` and/or `RECACHED_MAX_KEYS`** so an unbounded keyspace cannot
      exhaust the host.
- [ ] **Set `RECACHED_REPL_PASSWORD`** if you enable the replication listener. The server refuses to
      start without it on any interface other than loopback.
- [ ] Review [what is still beta](/guide/introduction#maturity) before putting the sync layer in
      front of untrusted users.

## Authentication

```bash
RECACHED_PASSWORD="$(openssl rand -base64 32)" recached-server
```

Clients must `AUTH` before any other command; unauthenticated commands are refused with
`NOAUTH Authentication required.`

Two properties worth knowing:

- **Password comparison is constant-time**, so it does not leak the secret through timing.
- **Five consecutive failures close the connection.** This slows brute force but does not ban the
  source — an attacker can reconnect. Pair it with an IP allowlist or a firewall if you are exposed
  to untrusted networks.

## Transport encryption

```bash
RECACHED_TLS_CERT=./cert.pem RECACHED_TLS_KEY=./key.pem recached-server
```

TLS applies to **both client ports** when enabled: clients use `rediss://` for RESP and `wss://` for
the browser sync socket. It also covers the replication listener; a replica must be
given `RECACHED_REPL_TLS_CA` to use it. The metrics port is always plaintext.

::: tip TLS is all-or-nothing, and fails loudly
Set both variables or neither. If exactly one is present the server **refuses to start** with an
explicit error, rather than falling back to plaintext — an operator who set `RECACHED_TLS_CERT`
intends TLS, and silently serving unencrypted traffic because the key variable was misspelled is a
failure you would not notice until traffic had already been exposed.

Prior to v0.2.1 this fell back to plain TCP and plain WebSocket silently. If you are on an earlier
version, verify TLS took effect rather than assuming: connect with `rediss://` and confirm a
plaintext client is refused.
:::

## IP allowlisting

```bash
RECACHED_ALLOW_IPS="10.0.1.5,10.0.1.6" recached-server
```

::: warning Exact IP addresses only — CIDR is not supported
Each entry must parse as a single IP address. Anything else — `10.0.0.0/8`, a hostname, a typo —
causes the server to **refuse to start**, naming the offending entry.

Prior to v0.2.1 invalid entries were logged and dropped, which quietly produced a *narrower*
allowlist than configured: a mistyped CIDR range excluded every host it was meant to admit, and a
wholly invalid list produced an empty allowlist that rejected every connection while the process
still started and passed health checks. Failing to start makes the misconfiguration unmissable.
:::

Treat the allowlist as defence in depth, not a primary control. In cloud environments where addresses
rotate, prefer TLS plus authentication and enforce network boundaries with security groups.

## The sync port is a different threat model

Port 6379 is reached by your backend. Port 6380 is reached by **browsers** — that is, by code running
on machines you do not control, in the hands of users who may be adversarial.

Without `RECACHED_SYNC_SECRET`, any browser that can open the WebSocket can read and write the entire
keyspace. For a single-tenant internal dashboard that may be fine. For anything multi-tenant it is a
full data breach.

```bash
RECACHED_SYNC_SECRET="$(openssl rand -base64 32)" recached-server
```

### Restrict which pages may connect

Browsers apply neither CORS nor a preflight to WebSockets. That makes this port different from every
HTTP endpoint you have secured before: **any page a user visits can open a socket to any Recached
that page's browser can reach**, and the request carries the user's network position. On a developer
machine running `ws://localhost:6380`, that is every site in every tab.

```bash
RECACHED_ALLOWED_ORIGINS="https://app.example.com,https://admin.example.com" recached-server
```

A handshake from any other origin is refused with `403` before a single frame is exchanged. Three
things to understand about the limits of this control:

- **It only defends against browsers.** A native client omits `Origin`, and an attacker with a raw
  socket can send whatever they like — so a client that sends no `Origin` is admitted. What the
  allowlist separates is *the application you deployed* from *another page in the same browser*,
  which is the threat that is otherwise unaddressed.
- **It is not a substitute for `RECACHED_SYNC_SECRET`.** Origin says which page connected; a scope
  token says which keys that page may touch. A multi-tenant deployment needs both.
- **Unset means allow-all**, with a warning at startup. Set it before exposing 6380 to a browser.

With the secret set, connections present an HMAC-signed token minted by your backend, and **every
command — reads included — is checked against the granted patterns**. Keyspace-wide and
administrative commands (`KEYS`, `SCAN`, `DBSIZE`, `FLUSHDB`, `SAVE`, `BGSAVE`, `REPLICAOF`) are
refused outright on scoped connections. Out-of-scope key access is refused with
`NOSCOPE key '<key>' is outside this connection's sync scopes`.

**Each grant also states its access**: `r=catalog:*` is read-only, `rw=cart:42:*` is read-write, and
a bare pattern is read-write. Grant `r=` to anything the browser should follow but never change —
catalogs, feature flags, tenant config, prices — and a write to it is refused with
`NOSCOPE key '<key>' is read-only on this connection`. Read-only grants still receive the mutation
fan-out and may hold live queries, so the page stays current without being able to rewrite what it
is reading.

Read [Sync Scopes](/server/sync-scopes) in full before you design your token scheme — the model is
prefix-based and the details matter.

### Minting tokens

Tokens are minted **server-side**, from a session your backend has already authenticated. The
secret must never reach the browser:

1. User authenticates with your application as normal.
2. Your backend derives the grants that user gets — for example `rw=cart:42:*`, `rw=user:42:*`,
   `r=catalog:*` — writing each one at the narrowest access that still works.
3. Your backend signs a token with `RECACHED_SYNC_SECRET` and returns it.
4. The browser passes it to `createCache({ connect: { syncToken } })`.

If a token leaks, it grants its scopes until it expires — scope narrowly and keep lifetimes short.

## Replication

::: danger The replication port bypassed authentication entirely before 0.2.4
Every release up to and including 0.2.3 bound `${RECACHED_BIND}:6381` — default `0.0.0.0` —
**unconditionally on every node**, whether or not replication was configured, and skipped the
handshake completely when `RECACHED_REPL_PASSWORD` was unset, which was also the default. Any peer
that connected received a full dump of the keyspace followed by a live stream of every subsequent
write, regardless of `RECACHED_PASSWORD`. If you are on an earlier version, upgrade or firewall
6381 now.
:::

The listener is opt-in from 0.2.4 onward:

```bash
RECACHED_REPL_ENABLE=1 \
RECACHED_REPL_PASSWORD="$(openssl rand -base64 32)" \
recached-server
```

- **It binds only when `RECACHED_REPL_ENABLE` is set** — on the primary, and on any replica that
  serves sub-replicas. A replica that merely consumes replication does not need it.
- **Enabling it off-loopback without a password refuses to start.** The port serves the whole
  keyspace to whoever connects, so it is not a thing to leave unauthenticated on a reachable
  interface.
- **`RECACHED_ALLOW_IPS` and `RECACHED_MAX_CONNECTIONS` apply to it**, which they did not before.
- **Failed auth is throttled per source address.** The handshake is one-shot, so before this a wrong
  guess cost an attacker only a reconnect.

### Encrypting replication

Replication is **plaintext unless you configure it otherwise**, and it carries the password followed
by the entire keyspace. Two variables turn it into an authenticated, encrypted channel:

::: warning A single self-signed certificate will not work here
Replication TLS needs a **two-certificate chain**: a CA certificate, and a separate leaf certificate
for the primary that the CA signed. The usual one-liner —
`openssl req -x509 -newkey rsa:2048 -keyout key.pem -out cert.pem -nodes` — produces a certificate
marked `CA:TRUE`, and the replica's TLS stack refuses to accept a CA certificate as a server
certificate (`CaUsedAsEndEntity`). OpenSSL clients such as `redis-cli --cacert` are more forgiving,
so the same certificate can work for `rediss://` while failing for replication. Generate the chain
below rather than reusing a self-signed file.
:::

```bash
# 1. A private CA. Keep ca-key.pem off both servers.
openssl req -x509 -newkey rsa:2048 -keyout ca-key.pem -out ca.pem -days 3650 -nodes \
  -subj "/CN=my-recached-ca"

# 2. A leaf certificate for the primary, signed by that CA. The SANs must list
#    every name or address a replica will use in RECACHED_REPLICAOF.
openssl req -newkey rsa:2048 -keyout server-key.pem -out server.csr -nodes \
  -subj "/CN=primary.internal"
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca-key.pem -CAcreateserial \
  -out server.pem -days 3650 -sha256 -extfile <(printf '%s\n' \
    'basicConstraints=critical,CA:FALSE' \
    'subjectAltName=DNS:primary.internal,IP:10.0.1.5' \
    'extendedKeyUsage=serverAuth')
```

```bash
# Primary — the replication listener reuses the server certificate.
RECACHED_TLS_CERT=./server.pem RECACHED_TLS_KEY=./server-key.pem \
RECACHED_REPL_ENABLE=1 \
RECACHED_REPL_PASSWORD="$(openssl rand -base64 32)" \
recached-server

# Replica — trusts the CA, and verifies the primary against it.
RECACHED_REPLICAOF="primary.internal:6381" \
RECACHED_REPL_TLS_CA=./ca.pem \
RECACHED_REPL_PASSWORD="…same…" \
recached-server
```

The point is not only confidentiality. Without TLS the replica has no way to check *who* it is
following, so a DNS hijack or an on-path attacker can feed it an arbitrary keyspace and it will load
it. Certificate verification is what closes that, which is why the trust anchor is required rather
than optional — there is no "encrypt but do not verify" mode.

`RECACHED_REPL_TLS_CA` is an explicit file rather than the system root store on purpose: replication
links two hosts the same operator runs, so trusting one private CA is both simpler and tighter than
trusting every public CA to vouch for a host that streams your whole dataset. A public bundle such as
`/etc/ssl/certs/ca-certificates.crt` works too if the primary's certificate is publicly issued.

If `RECACHED_REPLICAOF` names an IP but the certificate names a host, either add the IP as a SAN (as
above) or set `RECACHED_REPL_TLS_SERVERNAME` to the certificate's name. A mismatch is refused with a
message naming both what was expected and what the certificate actually covers.

::: warning A plaintext replication link is still the default
Setting `RECACHED_TLS_CERT` on the primary encrypts the RESP, WebSocket **and** replication
listeners, but a replica that has no `RECACHED_REPL_TLS_CA` connects in the clear — and against a
TLS-enabled primary its handshake simply fails. The server logs a warning at startup on any replica
following a primary without TLS. If you cannot use TLS, keep replication on a private network or a
tunnel (WireGuard, SSH, a service mesh), and treat the replication password as protection against a
rogue replica rather than against an observer.

The metrics port remains plaintext and unauthenticated regardless — firewall it.
:::

Recached never promotes a replica from a timeout. Before sending `REPLICAOF NO ONE`, fence the old primary by stopping it or revoking client access. Without fencing, a network partition can leave two writable primaries. See [manual failover](/server/configuration#manual-failover).

## Resource limits

An unbounded cache is a denial-of-service vector against its own host.

| Setting | Why |
|---|---|
| `RECACHED_MAX_MEMORY` | Caps memory; pair with `RECACHED_EVICTION` to choose behaviour at the cap. |
| `RECACHED_MAX_KEYS` | Caps keyspace size independently of value sizes. |
| `RECACHED_MAX_CONNECTIONS` | Defaults to 1024. Connections beyond the limit are rejected outright. |

See [Operations → Capacity limits](/server/operations#capacity-limits) for the limits that are
compiled in and cannot be configured.

## Threat model, stated plainly

What Recached defends against today:

- Unauthenticated access (password, constant-time comparison)
- Network eavesdropping on every port except metrics (TLS on RESP, WebSocket and replication)
- Browser clients reading data they should not (sync scopes, signed tokens)
- Cross-origin WebSocket hijacking (`RECACHED_ALLOWED_ORIGINS`)
- Brute-force password guessing (connection dropped after 5 failures; replication auth throttled
  per source address)
- Rogue replicas (replication password, and an opt-in listener that will not run unauthenticated
  off-loopback)
- A rogue *primary* feeding a replica an arbitrary keyspace, when replication TLS is configured
  (certificate verification)
- Connection-slot exhaustion by half-open sockets (handshake deadline)
- Local users reading the cache off disk (snapshot, AOF and dedup files created `0600`)

What it does **not** defend against, and you should not assume:

- **No per-command ACLs.** Unlike Redis 6+ ACLs, authentication is all-or-nothing on the RESP port —
  any authenticated client can run any command. Scoping exists only on the WebSocket sync path.
- **No audit log.** There is no record of who read or wrote what.
- **Replication is plaintext by default.** It can be encrypted and authenticated end to end
  (`RECACHED_REPL_TLS_CA`), but a replica configured without it connects in the clear. See
  [Replication](#replication).
- **No encryption at rest.** Snapshots and AOF files are plaintext MessagePack. They are created
  `0600` so other local users cannot read them, but anyone who can read them as the server's user,
  or who obtains the disk, has the whole keyspace. Use disk encryption.
- **No rate limiting on the RESP port.** `RLSET`/`RLCHECK` are commands you can use for *your*
  application's rate limiting; they do not throttle clients of the cache itself.
- **No third-party security review.** The sync layer in particular is young. See
  [Maturity](/guide/introduction#maturity).

## Reporting a vulnerability

Report security issues privately to [dennis@thinkgrid.dev](mailto:dennis@thinkgrid.dev) rather than
in a public issue.

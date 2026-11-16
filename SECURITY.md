# Security Policy

## Supported versions

Recached is pre-1.0. Only the latest released version receives security fixes.

| Version | Supported |
| ------- | --------- |
| 0.5.x   | yes       |
| < 0.5   | no        |

## Reporting a vulnerability

**Do not open a public issue.**

Report privately through GitHub's [private vulnerability
reporting](https://github.com/recached-sh/recached/security/advisories/new) on
this repository. If that is unavailable to you, email the maintainers via the
address on the organisation profile.

Please include:

- what an attacker can do, and what access they need to do it
- the affected version or commit
- a reproduction — a command sequence, a script, or a failing test
- the configuration it needs, especially any non-default environment variables

You should get an acknowledgement within 72 hours and an assessment within
seven days. We will tell you what we intend to do and when, and credit you in
the advisory unless you would rather we did not.

## Scope

In scope: the server (`recached-server`), the engine (`core-engine`), the sync
protocol and its clients (`sync-client`, `recached-embed`, `recached-edge`),
the published Docker images, and the release artifacts.

Out of scope, because they are documented properties rather than defects — see
[the security guide](https://recached.dev/server/security):

- Recached has no ACLs, no per-user authentication and no audit log. A client
  that authenticates has full access to everything its sync scope allows.
- There is no encryption at rest. RDB and AOF files are plaintext.
- There is no rate limiting on the RESP port.
- Running any listener on an untrusted network without `RECACHED_PASSWORD`,
  TLS and `RECACHED_ALLOW_IPS` is a deployment choice, not a vulnerability.

A finding that a *documented* guarantee is not actually enforced is very much
in scope, and is the most valuable kind of report.

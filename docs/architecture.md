# Architecture

The application owns its HTTP server. Stargate handles authentication in the same process and responds directly to account routes. Application routes receive a normalized optional identity and response headers, then continue to the host handler.

```text
host HTTP server -> framework adapter -> http -> core
                                        |         |
                                        ui     AuthStore
                                                  |
                                     storage::adapters::turso
```

## Workspace boundaries

| Crate | Responsibility |
| --- | --- |
| `core` | Explicit validated configuration, identity, authorization, OIDC, sessions, API keys and structured audit events |
| `storage` | Engine-independent `AuthStore` contract and records, plus optional engine adapters, migrations and bounded MVCC retries |
| `http` | HTTP-neutral requests/outcomes, account routing, cookies, CSRF and response hardening |
| `ui` | Production HTML, CSS and JavaScript embedded at compile time |
| `rust` | Native construction, Axum middleware and shared binding transport |
| `python` | PyO3 extension; Python's `stargate` package handles ASGI/decorator integration |
| `node` | napi-rs extension; JavaScript handles Express integration |
| `go` | Versioned C ABI; Go handles net/http integration |

All crate package names are unprefixed. Cargo dependency aliases `runtime-core` and `runtime-http` distinguish the local packages from Rust's built-in `core` crate and the crates.io `http` package. The matching library names are `runtime_core` and `runtime_http`, so generated documentation and tests avoid the same collisions. The external HTTP types are imported as `http-types`.

Storage implementations live inside `storage/src/adapters`, rather than in separate packages. Turso is available as `storage::adapters::turso` through the optional `turso` Cargo feature. The storage package has no default engine feature, so domain consumers can depend on the contract without compiling Turso. Future engines should be sibling modules with optional dependencies and their own migrations.

The core has no Axum, FastAPI, Express or Go dependency. Turso types never appear in the storage trait or domain APIs. Domain records are kept in `storage` to avoid a core/store dependency cycle. `AuthStore::resolve_identity` combines creation/linking into one atomic operation; `consume_oidc_transaction` atomically matches the browser binding, checks expiry, deletes and returns the record once. User-scoped revoke operations enforce ownership in the storage predicate.

## Contract and native transport

`auth.wit` is the language-independent contract, versioned as `stargate:runtime@0.1.0`. The implementation is native: Rust APIs, PyO3, napi-rs and C ABI/cgo. A WIT parser test validates the world and a shared conformance sequence validates its semantics in every language.

The current transport encoding maps WIT kebab-case field names to JSON snake_case. Header records become two-element `[name, value]` arrays; repeated headers stay repeated, especially `Set-Cookie`. `list<u8>` bodies become arrays of integers from 0 to 255. Optional fields become `null` or omitted input defaults. Claim pairs become a JSON object with equivalent JSON values. URLs, peer IPs and trusted CIDRs are parsed into Rust types. Native convenience wrappers fill the same defaults before creation.

Variants use a `type` discriminator: `respond` contains `response`, `continue` contains `identity` and `response_headers`, storage uses `turso`, and policies use `anonymous`, `authenticated` or `scopes`. The WIT scopes-policy payload maps to the `scopes` array. Malformed transport data fails at the native boundary. Valid requests with invalid credentials, denied mutations or service failures produce an HTTP response. Native authorization returns an equivalent decision with `allowed` and an HTTP status.

WIT declares the resolved configuration; native constructors additionally accept omitted optional settings and apply documented defaults. JSON is a native wire encoding, not runtime configuration-file support. Adapters only marshal data, manage body streams and map the outcome to their framework. They never implement login validation, token hashing, scope checks or revocation.

## Storage and concurrency

The initial migration creates `users`, `identities`, `sessions`, `api_keys`, `oidc_transactions`, `audit_events` and their indexes. `schema_migrations` tracks the applied schema version. Startup rejects a newer schema and runs pending migration SQL in a transaction before opening the runtime.

Operational writes use `BEGIN CONCURRENT` in MVCC mode. Each operation acquires a bounded concurrency permit and opens a separate connection. The whole transaction is retried only after rollback, with an exponential delay capped at 128 ms plus jitter. Default capacity is 16 connections and 32 retries. Configuration limits retries to at most 64. A failed commit never returns intermediate `RETURNING` rows. Generic Turso 0.8 MVCC conflict messages are recognized explicitly; arbitrary errors are never blindly retried.

Tables maintain indexed lifecycle columns and a JSON record containing the remaining versioned metadata. Secret hashes are stored in both indexed fields and record data; full session/API key secrets are never stored. Lifecycle updates atomically update both representations. The database and its journal contain hashes, provider identity metadata and short-lived OIDC nonce/PKCE material.

Audit events are persisted and optionally passed to a Rust host callback. Storage retry events are emitted to the configured callback. The library does not print them. The host can bridge the callback to tracing or OpenTelemetry.

## Scope and future work

v0.1 covers the embedded experience, generic OIDC authorization code flow, opaque sessions, API keys, scope checks, account UI and all four native integrations. No password authentication, MFA, WebAuthn, SAML, LDAP, organizations, billing, policy language, Redis, distributed sessions, PostgreSQL, MySQL or WASM runtime is implemented.

A future `stargate-d` binary can reuse the same core, HTTP, UI and store crates. A future `storage::adapters::postgres` module should implement `AuthStore`; a future WIT/WASM component can implement the same public world. Neither is on the v0.1 path. The project contains no AI, agent or model functionality.

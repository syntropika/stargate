# Configuration and security behavior

All values are passed to the constructor. The library does not read environment variables, `.env`, TOML, YAML or JSON files. The host supplies storage paths and secrets and controls how they are obtained.

| Setting | Default | Constraint |
| --- | --- | --- |
| `base_url` | Required | HTTPS origin without credentials, query, fragment or path prefix |
| `storage` | Required | `TursoConfig` with an explicit file path |
| `path_prefix` | `/auth` | Absolute, non-root, segment-based path without a trailing slash |
| `oidc` | Empty | Named providers, exact issuer string, client ID and client secret |
| `session.ttl_seconds` | 86400 | Between 1 second and 1 year |
| `session.scopes` | Empty | Explicit host grants assigned to OIDC sessions |
| `branding.app_name` | `Stargate` | Nonempty, at most 128 bytes, no control characters |
| `branding.logo` | None | HTTPS URL; rendered with escaped attributes |
| `branding.accent` | `#c4a882` | Six-digit hexadecimal color |
| `branding.stylesheet` | None | HTTPS stylesheet URL on the same origin as `base_url`; loaded after the embedded CSS |
| `max_body_bytes` | 65536 | Between 1 byte and 16 MiB |
| `max_header_bytes` | 16384 | Between 1 byte and 1 MiB |
| `trusted_proxies` | Empty | Explicit parsed IP networks |
| `allow_insecure_loopback` | False | Permits HTTP only on localhost or loopback for development |
| `storage.connections` | 16 | Between 1 and 128 |
| `storage.retry_limit` | 32 | Between 0 and 64 additional attempts |

Scope names contain ASCII letters, digits, `:`, `.`, `_` and `-`; there are at most 64 names and each is at most 128 bytes. API key expiry is optional. Expired and revoked credentials fail authentication; an invalid bearer credential never falls back to a valid cookie.

The host can serve a brand stylesheet, images and fonts from its own static routes.
Set `branding.stylesheet` to that stylesheet's absolute URL. It overrides presentation
without replacing Stargate's account pages, forms or authentication behavior. External
stylesheets and inline styles remain blocked; self-hosted fonts are allowed by the
account UI's Content Security Policy. Omit the setting to retain the default UI.

## OIDC

Provider discovery and JWKS retrieval use a 15-second HTTP timeout and never follow redirects. Endpoint URL schemes are validated. The issuer uses `IssuerUrl`, which preserves its exact spelling; a trailing slash is significant. The registered callback is derived exclusively from `base_url` and `path_prefix`.

Login uses PKCE S256, unpredictable state and nonce, a ten-minute stored transaction and a separate opaque browser-binding cookie. Callback atomically consumes the transaction, exchanges the code with the stored verifier and validates the signed ID token, issuer, audience, expiry and nonce using `openidconnect`. An access-token hash is verified when present. JWKS/discovery are refreshed on callback to honor key rotation. External identities are resolved by exact issuer plus subject; matching emails do not link accounts.

`return_to` accepts local absolute paths, optionally with a query. It rejects network-path references, remote URLs, fragments, whitespace, backslashes, non-ASCII input and percent escapes. This deliberately conservative v0.1 rule prevents ambiguous redirect interpretation. Callers can use ordinary ASCII paths such as `/projects?tab=active`.

## Cookies and CSRF

Session and OIDC binding cookies use `__Host-` names, `HttpOnly`, `Secure`, `SameSite=Lax`, `Path=/` and a bounded `Max-Age`. All cookies remain Secure in development. Sessions and keys use 256 bits of CSPRNG entropy; storage retains SHA-256 hashes of these high-entropy secrets. Human passwords are outside v0.1.

Account API mutations and logout require a session, an exact origin match and a constant-time comparison of `X-Stargate-CSRF`. The CSRF token is domain-separated from the opaque session token and returned through `/auth/api/me`. Neither a key nor its scopes grant access to account-management operations. GET logout only opens the UI; sign-out is a protected POST.

## HTTP and proxies

Owned routes are buffered only within the configured body limit. Application body streams remain the host's responsibility and are preserved by adapters. The shared boundary enforces method, path, header and size constraints. Duplicate single-value authentication headers and duplicate Stargate cookies are rejected. Noncanonical percent-encoded aliases of owned routes are rejected to prevent framework-specific route interpretation.

Account responses include `no-store`, a restrictive Content Security Policy, `nosniff`, frame blocking and `no-referrer`. HTML/branding are escaped and the UI uses DOM text nodes for dynamic values. API key secrets appear only in the creation response and the current page until dismissed or navigated away.

`X-Forwarded-Host` and `X-Forwarded-Proto` never determine origins, redirect URIs or cookie security. `AuthRequest::client_ip` considers `X-Forwarded-For` only for an immediate peer inside `trusted_proxies`, traversing the chain from the trusted end. The host supplies the actual peer address from its socket, never from a request header. Proxies and TLS terminate at the application's deployment layer.

## Operational boundaries

Use one shared runtime/store per embedded database file in a process. The runtime migrates the schema before serving requests. Audit failures and exhausted storage retries fail closed with service-unavailable responses; underlying database and provider details are not exposed to clients. Session revocation checks apply when a request authenticates, rather than retroactively interrupting handlers already executing.

Rust hosts can pass an audit sink to `Stargate::with_audit`. Other bindings receive the same persisted events in Turso; host callback registration for those languages is a future extension. Hosts remain responsible for TLS, secret provisioning, application request limits, deployment permissions and their own logging. Never log the supplied config, Authorization/Cookie headers or key-creation responses.

# Stargate

<!-- impeccable:product-schema 1 -->

## Platform

web

## Product Purpose

Embed authentication in an application's existing HTTP server. Developers evaluate and install Stargate through the public website and documentation; signed-in users manage their profile, API keys and sessions through the embedded account panel.

## Users

Application developers integrating Rust/Axum, Node/Express, Python/FastAPI or Starlette, and Go/net/http. Account owners manage their access; administrators manage local users when local accounts are enabled.

## Positioning

One Rust runtime runs in the application's process and stores authentication state in embedded Turso. The host owns the server, TLS, configuration and branding. The account UI is compiled into the runtime and needs no separate frontend service.

## Capabilities and Constraints

Support local email/password accounts, OIDC, opaque cookie sessions, scoped API keys, administrator user management and account password changes. Key secrets are shown once and stored as hashes. Account mutations use same-origin session cookies and CSRF protection. Preserve configurable route prefixes, application names, logos, accent colors and host stylesheets.

Email verification, invitations, password recovery, MFA, organizations and distributed sessions are not current product capabilities. Do not invent commercial pricing, adoption counts, uptime claims or testimonials.

## Brand Commitments

The approved direction uses warm charcoal, ivory and bronze, editorial serif headings, neutral interface text and fine structural rules. Retain Stargate's existing portal-and-star geometry. The public hero renders this mark with optional animated ASCII characters; account management uses a persistent desktop sidebar and compact credential register. All browser assets and fonts are served locally.

## Product Principles

- Keep authentication within the host application's control.
- Make credential scopes, expiration and revocation clear.
- Preserve usable loading, error and empty states, keyboard access and reduced-motion support.
- Present actual product capabilities and data rather than unsupported promotional claims.

## Evidence on Hand

The README, consumer documentation, four language integrations and existing account API establish product behavior. Branding assets and bundled font licenses establish reusable visual resources.

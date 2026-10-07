# Stargate

**Authentication inside your app.**

Embed OIDC sign-in, sessions, API keys and an account UI in your existing HTTP server. Stargate runs in the same process, keeps authentication logic in Rust and stores state in an embedded Turso database. Your application owns the server, TLS and configuration.

Use it with Rust / Axum, Python / FastAPI or Starlette, Node / Express, and Go / net/http.

## How it works

Mount Stargate in your application, pass your identity provider settings and protect the routes that need authentication. Users sign in through your OIDC provider and manage their profile, API keys and sessions at `/auth/`.

- **OIDC sign-in:** authorization code flow with PKCE, state and nonce validation.
- **Sessions:** opaque cookies, expiration and revocation.
- **API keys:** scoped credentials, shown once and stored as hashes.
- **Authorization:** require a signed-in identity or explicit scopes.
- **Account UI:** embedded HTML, CSS and JavaScript with configurable branding.
- **Storage:** embedded Turso with migrations; an engine-independent interface for custom stores.

All four integrations use the same Rust runtime. Configure it through the host application; Stargate does not discover configuration files or read environment variables.

## Install

Choose the package for your application:

```sh
# Python / FastAPI
python -m pip install syntropika-stargate fastapi

# Node / Express
npm install @syntropika/stargate express

# Rust / Axum
cargo add syntropika-stargate --rename stargate
```

Python's import remains `stargate`; the Rust command also makes the import `stargate`. npm selects an optional native binary automatically. Prebuilt Node and Python packages target Linux x64, macOS arm64 and Windows x64. Rust applications compile the runtime from source. Go integration is available from this repository and links to the Rust C ABI.

See [integration guides](docs/integrations.md) for configuration and [publishing](docs/publishing.md) for platform requirements and the GitHub Actions release workflow.

## Build from source

Native bindings must be built for the target operating system and architecture.

```sh
git clone https://github.com/syntropika/stargate.git
cd stargate
```

You need Rust 1.90+ and a C/C++ build toolchain. For native bindings, install Python 3.10+ and the tools for your host language: Node.js 20+ or Go 1.22+ with cgo. Linux builds also need libclang development libraries.

### Python / FastAPI

Create a virtual environment and install the local package:

```sh
python3 -m venv .venv
. .venv/bin/activate
python -m pip install maturin fastapi
(cd packages/python && maturin develop)
```

Mount authentication and protect a route:

```python
from fastapi import FastAPI
from stargate import Auth, OIDC, Turso

app = FastAPI()
auth = Auth(
    base_url="https://app.example.com",
    storage=Turso("./stargate.db"),
    oidc=OIDC(
        issuer="https://identity.example.com",
        client_id="your-client-id",
        client_secret="your-client-secret",
    ),
)
auth.mount(app)

@app.get("/private")
@auth.required()
async def private(user):
    return {"user": user.id}
```

Replace the provider placeholders with values supplied by your application. Register `https://app.example.com/auth/callback` with the provider and serve the application at the configured HTTPS origin. Cookies always use `Secure`, including during local development.

### Other integrations

| Host | Entry point | Example |
| --- | --- | --- |
| Rust / Axum | `Stargate::new`, `Stargate::mount`, `Stargate::require` | [Axum](examples/axum.rs) |
| Python / Starlette | `Auth.mount`, `Auth.required`, `Auth.require_scope` | [Integration guide](docs/integrations.md#python--fastapi) |
| Node / Express | `createAuth`, `auth.middleware`, `auth.required` | [Express](examples/express.js) |
| Go / net/http | `stargate.New`, `auth.Handler`, `auth.Require` | [net/http](packages/go/example/main.go) |

The [integration guide](docs/integrations.md) includes build commands and usage for each language. Mount Express middleware before body parsers. Go links to the Rust C ABI through cgo.

## Scope and security

v0.1 targets one process with one embedded database and an external OIDC identity provider. It includes session and API key management, scope checks and the account UI. Password authentication, MFA, organizations, distributed sessions and additional database engines are future work.

The host explicitly grants session scopes; API keys can request only a subset of those grants. Account mutations require a browser session and CSRF protection. The host supplies TLS, secrets and trusted proxy settings. See [configuration and security](docs/configuration.md) for defaults and constraints.

## Development

Run the checks from the repository root in an active Python virtual environment, with Rust, Node.js and Go available:

```sh
scripts/check.sh
```

This builds the native bindings and runs formatting, Clippy, Rust tests, storage migrations, concurrency checks, a mock OIDC flow, shared conformance tests across all four languages, framework integration tests and TypeScript declaration checks. Test databases use the system temporary directory; set `TMPDIR` to choose another location.

The account panel uses React, TypeScript and Tailwind, built with Vite. Its compiled
JavaScript and CSS are checked in and embedded by the `ui` crate. Installing or
running Stargate does not require Node.js or a separate frontend server. See the
[frontend development guide](docs/frontend.md) for build and preview commands.

The workspace contains `core`, `http`, `storage`, `ui`, `rust`, `python`, `node` and `go`. Database adapters live inside `storage`, with optional Cargo features. Turso is enabled by the Rust host integration.

## Documentation

The public landing and documentation site are built from `website/`. See the
[website deployment guide](docs/website.md) for local development and the
Cloudflare Workers pipeline.

- [Build and integrate each language](docs/integrations.md)
- [Package names and Trusted Publishing setup](docs/publishing.md)
- [Configuration and security behavior](docs/configuration.md)
- [Architecture and workspace boundaries](docs/architecture.md)
- [Shared runtime contract](auth.wit)

Licensed under [Apache 2.0](LICENSE).

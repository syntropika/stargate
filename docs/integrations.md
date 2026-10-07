# Integration guide

Build Stargate from a checkout and mount it in your existing application. Run the build commands from the repository root unless a section says otherwise.

## Python / FastAPI

The registry distribution is `syntropika-stargate`; its import remains `stargate`. The commands below build the local checkout.

Build and install from this checkout in an active virtual environment:

```sh
python -m pip install maturin fastapi
cd packages/python
maturin develop
```

```python
from fastapi import FastAPI
from stargate import Auth, OIDC, Turso

app = FastAPI()
auth = Auth(
    base_url="https://example.com",
    storage=Turso("./stargate.db"),
    oidc=OIDC(
        issuer="https://your-provider.example.com",
        client_id=client_id,
        client_secret=client_secret,
    ),
    session={"scopes": ["projects:read", "projects:write"]},
)
auth.mount(app)

@app.get("/private")
@auth.required()
async def private(user):
    return {"user": user.id}

@app.post("/projects")
@auth.require_scope("projects:write")
async def create_project(user):
    return {"owner": user.id}
```

`client_id` and `client_secret` above are values supplied by the host application. Stargate does not load files, inspect environment variables or discover configuration. Protected FastAPI handlers accept an injected `user` argument; normal path, query and dependency arguments remain available. Sync and async handlers are supported.

Set the provider's registered redirect URI to `https://example.com/auth/callback`. Use HTTPS for the public base URL. All cookies remain `Secure`, including when `allow_insecure_loopback=True` is used for a local mock provider. That option permits HTTP only on literal loopback addresses or `localhost`.

## Rust / Axum

The published crate is `syntropika-stargate`. Alias it as `stargate` in your dependencies and change the example import to `use stargate::{...}`. The source workspace uses the local `rust` package name.

```rust,no_run
use rust::{AuthConfig, IssuerUrl, OidcProviderConfig, Secret,
    Stargate, StorageConfig, TursoConfig, Policy};
use axum::{Router, routing::get};

async fn run() -> Result<(), Box<dyn std::error::Error>> {
let mut config = AuthConfig::new(
    "https://example.com".parse()?,
    StorageConfig::Turso(TursoConfig::new("./stargate.db")),
);
config.oidc.push(OidcProviderConfig {
    name: "default".into(),
    issuer: IssuerUrl::new("https://your-provider.example.com".into())?,
    client_id: "your-client-id".into(),
    client_secret: Secret::new("your-client-secret"),
});
let stargate = Stargate::new(config).await?;
let private = Stargate::require(
    Router::new().route("/private", get(|| async { "private" })),
    Policy::Authenticated,
);
let app = stargate.mount(Router::new().merge(private));
Ok(())
}
```

Authenticated identity is available as `Extension<Identity>`. Apply `Stargate::require` to application routes, then mount Stargate around the combined router so authentication runs before authorization. Serve this router with the host's existing listener. [The Axum example](../examples/axum.rs) shows the listener integration.

For a custom store, construct `runtime_core::Auth::new(config, Arc<dyn AuthStore>, sink)` and wrap it with `Runtime::new`. The domain and HTTP layers depend only on `AuthStore`.

## Node / Express

The registry package is `@syntropika/stargate`; use `require('@syntropika/stargate')` after installing a published release. The commands below build the local checkout.

```sh
python scripts/build-bindings.py
cd packages/node
npm install
```

```javascript
const express = require('express');
const {createAuth, turso} = require('./packages/node');

const auth = await createAuth({
  baseUrl: 'https://example.com',
  storage: turso('./stargate.db'),
  oidc: {issuer, clientId, clientSecret},
  session: {scopes: ['projects:read', 'projects:write']},
});
const app = express();
app.use(auth.middleware());
app.use(express.json());
app.get('/private', auth.required(), (req, res) => {
  res.json({user: req.stargateIdentity.user_id});
});
```

Mount Stargate before body parsers. Its middleware serves owned routes itself and leaves application body streams available to subsequent middleware. `auth.handler()` is also available for mounting `/auth` explicitly. Native work runs outside the JavaScript event loop. Type declarations are included.

## Go / net/http

Build the C library with `python scripts/build-bindings.py`, then point the C linker and system loader to `target/debug` (or `target/release` for a release build). For Linux:

```sh
export CGO_LDFLAGS="-L$PWD/target/debug -Wl,-rpath,$PWD/target/debug"
cd packages/go
go test ./...
```

```go
import stargate "github.com/syntropika/stargate/packages/go"

auth, err := stargate.New(stargate.Config{
    BaseURL: "https://example.com",
    Storage: stargate.Turso("./stargate.db"),
    OIDC: []stargate.OIDC{{Issuer: issuer, ClientID: clientID, ClientSecret: clientSecret}},
})
if err != nil { return err }
defer auth.Close()

mux.Handle("/auth/", auth.Handler())
mux.Handle("/private", auth.Require(privateHandler))
```

`Require` authenticates requests when used directly. For optional identity on all application routes, wrap the mux with `auth.Middleware(mux)`. Read identity through `stargate.IdentityFromContext`. The Go package is source-based and links to the Rust library; it does not reimplement authentication.

## Account routes

The default prefix is `/auth`; configure `path_prefix` (`pathPrefix` in Node) to change it.

| Route | Behavior |
| --- | --- |
| `GET /auth/`, `/profile`, `/keys`, `/sessions` | Embedded account UI |
| `GET /auth/login` | Start OIDC with PKCE, state, nonce and a browser-binding cookie |
| `GET /auth/callback` | Validate the provider response, create a session and redirect |
| `GET /auth/logout` | Open the account UI with a sign-out action |
| `POST /auth/logout` | Revoke the current session and clear its cookie |
| `GET /auth/api/config` | Public branding/provider names |
| `GET /auth/api/me` | Identity, current session ID and CSRF token |
| `GET, POST /auth/api/keys` | List or create keys |
| `DELETE /auth/api/keys/{id}` | Revoke an owned key |
| `GET /auth/api/sessions` | List owned sessions |
| `DELETE /auth/api/sessions/{id}` | Revoke an owned session |
| `DELETE /auth/api/sessions` | Revoke every session, including the current one |

Mutations require a session, an `Origin` matching the configured base URL and `X-Stargate-CSRF` from `/auth/api/me`. The embedded UI supplies these automatically. API key credentials cannot manage keys or sessions. Key creation returns a full `ak_live_` secret once; subsequent lists contain only metadata. Send the secret as `Authorization: Bearer ak_live_...` to application routes.

`session.scopes` explicitly grants the scopes assigned to every OIDC session in this runtime. The default is empty. Keys can request only a subset of the owner's session scopes. These are host-level grants, not permissions inferred from an email address or provider role claim.

## Build and verification

Native bindings must be built for each target operating system and architecture before distribution.

From a Python virtual environment, run:

```sh
scripts/check.sh
```

The checks include formatting, Clippy, Rust domain and HTTP tests, Turso migrations, 100 concurrent sessions/keys/revocations, concurrent identity resolution, single-use OIDC consumption, a mock RSA OIDC flow, common conformance tests through all four languages, and real framework integration tests. Test tooling and the mock IdP are development-only.

Workspace packages are named `core`, `http`, `storage`, `ui`, `rust`, `python`, `node` and `go`. Storage adapters live inside `storage`; enable its `turso` feature to use `storage::adapters::turso::TursoStore`. The Rust host integration enables that feature automatically.

The canonical public contract is [auth.wit](../auth.wit). Native bindings use an explicitly passed JSON transport encoding of that contract. See [architecture](architecture.md) and [configuration and security](configuration.md). This JSON boundary does not load JSON configuration files and does not require WASM.

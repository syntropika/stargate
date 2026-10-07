//! Native construction, the shared binding transport, and Axum integration.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
};
use runtime_core::{AuditSink, Auth, Result};
pub use runtime_core::{
    AuthConfig, AuthType, BrandingConfig, Identity, IssuerUrl, OidcProviderConfig, Policy, Secret,
    SessionConfig, StorageConfig, TursoConfig,
};
pub use runtime_http::{AuthOutcome, AuthRequest, AuthResponse, Runtime};
use std::sync::Arc;

#[derive(Clone)]
pub struct Stargate {
    pub runtime: Runtime,
}
impl Stargate {
    pub async fn new(config: AuthConfig) -> Result<Self> {
        Self::with_audit(config, None).await
    }
    pub async fn with_audit(config: AuthConfig, sink: Option<AuditSink>) -> Result<Self> {
        config.validate()?;
        let StorageConfig::Turso(t) = &config.storage;
        let store = Arc::new(
            storage::adapters::turso::TursoStore::open(
                &t.path,
                t.connections,
                t.retry_limit,
                sink.clone(),
            )
            .await?,
        );
        let auth = Auth::new(config, store, sink).await?;
        Ok(Self {
            runtime: Runtime::new(Arc::new(auth)),
        })
    }
    pub fn mount(&self, app: Router) -> Router {
        app.layer(middleware::from_fn_with_state(
            self.runtime.clone(),
            dispatch,
        ))
    }
    /// Apply to application routes before mounting the authentication middleware.
    pub fn require(app: Router, policy: Policy) -> Router {
        app.layer(middleware::from_fn_with_state(policy, enforce))
    }
}
async fn enforce(State(policy): State<Policy>, req: Request, next: Next) -> Response {
    let identity = req.extensions().get::<Identity>();
    let runtime = req.extensions().get::<Runtime>();
    let decision = if let Some(runtime) = runtime {
        runtime.authorize(identity, &policy).await
    } else {
        runtime_core::authorize(identity, &policy)
    };
    match decision {
        Ok(()) => next.run(req).await,
        Err(error) => render(runtime_http::response(
            if matches!(error, runtime_core::Error::Unauthorized) {
                401
            } else if matches!(error, runtime_core::Error::Forbidden) {
                403
            } else {
                503
            },
            "application/json",
            br#"{"error":"access denied"}"#.to_vec(),
        )),
    }
}
async fn dispatch(State(runtime): State<Runtime>, req: Request, next: Next) -> Response {
    let (mut parts, body) = req.into_parts();
    // The host retains its application body stream. Owned routes are bounded before buffering.
    let owned = runtime.owns(parts.uri.path());
    let headers = parts
        .headers
        .iter()
        .map(|(k, v)| v.to_str().map(|v| (k.to_string(), v.to_string())))
        .collect::<std::result::Result<Vec<_>, _>>();
    let Ok(headers) = headers else {
        return render(runtime_http::response(400, "application/json", Vec::new()));
    };
    let peer_ip = parts
        .extensions
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|p| p.0.ip());
    let (body_bytes, body) = if owned {
        match to_bytes(body, runtime.auth.config.max_body_bytes).await {
            Ok(bytes) => (bytes.to_vec(), Body::from(bytes)),
            Err(_) => return render(runtime_http::response(413, "application/json", Vec::new())),
        }
    } else {
        (Vec::new(), body)
    };
    let input = AuthRequest {
        method: parts.method.to_string(),
        path: parts.uri.path().into(),
        query: parts.uri.query().map(str::to_owned),
        headers,
        body: body_bytes,
        peer_ip,
    };
    match runtime.handle(input).await {
        AuthOutcome::Respond { response } => render(response),
        AuthOutcome::Continue {
            identity,
            response_headers,
        } => {
            if let Some(identity) = identity {
                parts.extensions.insert(identity);
            }
            parts.extensions.insert(runtime);
            let mut response = next.run(Request::from_parts(parts, body)).await;
            for (name, value) in response_headers {
                if let (Ok(n), Ok(v)) = (name.parse::<axum::http::HeaderName>(), value.parse()) {
                    response.headers_mut().append(n, v);
                }
            }
            response
        }
    }
}
fn render(r: AuthResponse) -> Response {
    let mut response = Response::new(Body::from(r.body));
    *response.status_mut() = axum::http::StatusCode::from_u16(r.status)
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    for (name, value) in r.headers {
        if let (Ok(n), Ok(v)) = (name.parse::<axum::http::HeaderName>(), value.parse()) {
            response.headers_mut().append(n, v);
        }
    }
    response
}

/// JSON is a transport encoding of auth.wit, never a configuration discovery mechanism.
/// Native bindings share this implementation so parsing, errors and authorization cannot diverge.
pub struct NativeRuntime {
    executor: tokio::runtime::Runtime,
    pub stargate: Stargate,
}
impl NativeRuntime {
    pub fn create(config: &str) -> std::result::Result<Self, String> {
        let config: AuthConfig =
            serde_json::from_str(config).map_err(|_| "invalid configuration".to_string())?;
        let executor =
            tokio::runtime::Runtime::new().map_err(|_| "runtime unavailable".to_string())?;
        let stargate = executor
            .block_on(Stargate::new(config))
            .map_err(|e| e.to_string())?;
        Ok(Self { executor, stargate })
    }
    pub fn handle(&self, input: &str) -> std::result::Result<String, String> {
        let req = serde_json::from_str(input).map_err(|_| "invalid request".to_string())?;
        serde_json::to_string(&self.executor.block_on(self.stargate.runtime.handle(req)))
            .map_err(|_| "response encoding failed".to_string())
    }
    pub fn authorize(&self, input: &str) -> std::result::Result<String, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            identity: Option<Identity>,
            policy: Policy,
        }
        let input: Input =
            serde_json::from_str(input).map_err(|_| "invalid request".to_string())?;
        let result = self.executor.block_on(
            self.stargate
                .runtime
                .authorize(input.identity.as_ref(), &input.policy),
        );
        let decision = match result {
            Ok(()) => serde_json::json!({"allowed":true,"status":200}),
            Err(error) => {
                serde_json::json!({"allowed":false,"status":match error {runtime_core::Error::Unauthorized=>401,runtime_core::Error::Forbidden=>403,runtime_core::Error::BadRequest=>400,_=>503}})
            }
        };
        Ok(decision.to_string())
    }
}

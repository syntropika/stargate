use axum::{Extension, Json, Router, routing::get};
use rust::{
    AuthConfig, Identity, IssuerUrl, OidcProviderConfig, Policy, Secret, Stargate, StorageConfig,
    TursoConfig,
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The host chooses how to obtain its configuration. Replace the provider placeholders before starting the example.
    let mut config = AuthConfig::new(
        "https://localhost:3000".parse()?,
        StorageConfig::Turso(TursoConfig::new("./stargate.db")),
    );
    config.oidc.push(OidcProviderConfig {
        name: "default".into(),
        issuer: IssuerUrl::new("https://identity.example.com".into())?,
        client_id: "your-client-id".into(),
        client_secret: Secret::new("your-client-secret"),
    });
    let stargate = Stargate::new(config).await?;
    let private = Stargate::require(
        Router::new().route(
            "/private",
            get(|Extension(user): Extension<Identity>| async move { Json(user) }),
        ),
        Policy::Authenticated,
    );
    let app = stargate.mount(
        Router::new()
            .route("/", get(|| async { "Stargate example" }))
            .merge(private),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    // Use a TLS terminator at the configured base URL for browser login; this listener is host-owned.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

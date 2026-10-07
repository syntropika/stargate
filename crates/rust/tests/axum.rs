use axum::{
    Extension, Json, Router,
    body::Body,
    routing::{get, post},
};
use rust::*;
use tower::ServiceExt;
#[tokio::test]
async fn axum_adapter_authenticates_and_preserves_host_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let config = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new(
            dir.path().join("stargate.db").to_str().unwrap(),
        )),
    );
    let auth = Stargate::new(config).await.unwrap();
    let private = Stargate::require(
        Router::new().route(
            "/private",
            get(|Extension(user): Extension<Identity>| async move { Json(user) }),
        ),
        Policy::Authenticated,
    );
    let app = auth.mount(
        private.merge(Router::new().route("/echo", post(|body: String| async move { body }))),
    );
    let req = |method: &str, path: &str, body: &str| {
        axum::http::Request::builder()
            .method(method)
            .uri(path)
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(req("GET", "/auth/", ""))
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        app.clone()
            .oneshot(req("GET", "/private", ""))
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        app.clone()
            .oneshot(req("POST", "/auth/api/keys", &"x".repeat(65537)))
            .await
            .unwrap()
            .status(),
        413
    );
    let response = app
        .oneshot(req("POST", "/echo", "host-body"))
        .await
        .unwrap();
    assert_eq!(
        axum::body::to_bytes(response.into_body(), 100)
            .await
            .unwrap(),
        "host-body"
    );
}

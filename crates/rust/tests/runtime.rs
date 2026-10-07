use runtime_core::{hash, now, random_token};
use runtime_http::SESSION_COOKIE;
use rust::*;
use serde_json::{Value, json};
use storage::adapters::turso::TursoStore;
use storage::{AuthStore, ExternalIdentity, OidcTransaction, User};

async fn setup() -> (tempfile::TempDir, Stargate, User, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new(
            dir.path().join("stargate.db").to_str().unwrap(),
        )),
    );
    config.session.scopes = vec!["projects:read".into(), "projects:write".into()];
    let runtime = Stargate::new(config).await.unwrap();
    let user = User {
        id: random_token(),
        email: Some("person@example.com".into()),
        created_at: now(),
    };
    let user = runtime
        .runtime
        .auth
        .store
        .resolve_identity(
            &ExternalIdentity {
                issuer: "https://issuer.example.com".into(),
                subject: "subject".into(),
                email: user.email.clone(),
                metadata: json!({}),
            },
            &user,
        )
        .await
        .unwrap();
    let (_, session) = runtime.runtime.auth.create_session(&user.id).await.unwrap();
    (dir, runtime, user, session.expose().into())
}
fn request(method: &str, path: &str, token: Option<&str>) -> AuthRequest {
    AuthRequest {
        method: method.into(),
        path: path.into(),
        query: None,
        headers: token
            .map(|s| vec![("cookie".into(), format!("{SESSION_COOKIE}={s}"))])
            .unwrap_or_default(),
        body: Vec::new(),
        peer_ip: None,
    }
}
fn respond(outcome: AuthOutcome) -> AuthResponse {
    match outcome {
        AuthOutcome::Respond { response } => response,
        _ => panic!("expected response"),
    }
}
fn body(response: &AuthResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}
fn mutation(method: &str, path: &str, token: &str) -> AuthRequest {
    let mut req = request(method, path, Some(token));
    req.headers.extend([
        ("origin".into(), "https://app.example.com".into()),
        ("x-stargate-csrf".into(), runtime_core::csrf(token)),
        ("content-type".into(), "application/json".into()),
    ]);
    req
}

#[test]
fn wit_contract_parses() {
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../auth.wit"))
        .unwrap()
        .0;
    let world = resolve
        .select_world(&[package], Some("embedded-runtime"))
        .unwrap();
    assert_eq!(resolve.worlds[world].exports.len(), 1);
}
#[test]
fn config_is_explicit_secure_and_redacted() {
    let valid = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new("./stargate.db")),
    );
    valid.validate().unwrap();
    for target in [
        "//evil.example",
        "https://evil.example",
        "/\\evil",
        "/%2f%2fevil",
        "/\r\nevil",
        "/a#fragment",
    ] {
        assert!(valid.validate_return_to(target).is_err(), "{target:?}");
    }
    valid.validate_return_to("/projects?tab=active").unwrap();
    let mut invalid = valid.clone();
    invalid.base_url = "http://app.example.com".parse().unwrap();
    assert!(invalid.validate().is_err());
    invalid.allow_insecure_loopback = true;
    assert!(invalid.validate().is_err());
    invalid.base_url = "http://127.0.0.1:3000".parse().unwrap();
    invalid.validate().unwrap();
    invalid.path_prefix = "/auth/".into();
    assert!(invalid.validate().is_err());
    let secret = Secret::new("sensitive-config-value");
    assert!(!format!("{secret:?}").contains("sensitive-config-value"));
}
#[tokio::test]
async fn sessions_keys_authorization_and_revocation() {
    let (_dir, runtime, user, token) = setup().await;
    assert!(matches!(
        runtime
            .runtime
            .handle(request("GET", "/private", None))
            .await,
        AuthOutcome::Continue { identity: None, .. }
    ));
    let outcome = runtime
        .runtime
        .handle(request("GET", "/private", Some(&token)))
        .await;
    let identity = match outcome {
        AuthOutcome::Continue {
            identity: Some(id), ..
        } => id,
        _ => panic!(),
    };
    assert_eq!(identity.auth_type, AuthType::Session);
    assert_eq!(identity.user_id.as_deref(), Some(user.id.as_str()));
    runtime
        .runtime
        .authorize(
            Some(&identity),
            &Policy::Scopes {
                scopes: vec!["projects:write".into()],
            },
        )
        .await
        .unwrap();
    assert!(
        runtime
            .runtime
            .authorize(
                Some(&identity),
                &Policy::Scopes {
                    scopes: vec!["admin".into()]
                }
            )
            .await
            .is_err()
    );
    let mut req = mutation("POST", "/auth/api/keys", &token);
    req.body = serde_json::to_vec(&json!({"name":"laptop","scopes":["projects:read"]})).unwrap();
    let created = respond(runtime.runtime.handle(req).await);
    assert_eq!(created.status, 201);
    let created = body(&created);
    let key = created["secret"].as_str().unwrap();
    assert_eq!(key.len(), 51);
    let mut bearer = request("GET", "/private", None);
    bearer
        .headers
        .push(("authorization".into(), format!("Bearer {key}")));
    let outcome = runtime.runtime.handle(bearer.clone()).await;
    let id = match outcome {
        AuthOutcome::Continue {
            identity: Some(id), ..
        } => id,
        _ => panic!(),
    };
    assert_eq!(id.auth_type, AuthType::ApiKey);
    assert!(
        runtime
            .runtime
            .authorize(
                Some(&id),
                &Policy::Scopes {
                    scopes: vec!["projects:write".into()]
                }
            )
            .await
            .is_err()
    );
    let mut api_only = bearer.clone();
    api_only.path = "/auth/api/keys".into();
    assert_eq!(respond(runtime.runtime.handle(api_only).await).status, 403);
    let listed = respond(
        runtime
            .runtime
            .handle(request("GET", "/auth/api/keys", Some(&token)))
            .await,
    );
    let text = String::from_utf8(listed.body).unwrap();
    assert!(!text.contains(key));
    assert!(!text.contains("secret_hash"));
    let revoke = mutation(
        "DELETE",
        &format!("/auth/api/keys/{}", created["key"]["id"].as_str().unwrap()),
        &token,
    );
    assert_eq!(respond(runtime.runtime.handle(revoke).await).status, 204);
    assert_eq!(respond(runtime.runtime.handle(bearer).await).status, 401);
    assert_eq!(
        respond(
            runtime
                .runtime
                .handle(mutation("DELETE", "/auth/api/sessions", &token))
                .await
        )
        .status,
        204
    );
    assert_eq!(
        respond(
            runtime
                .runtime
                .handle(request("GET", "/private", Some(&token)))
                .await
        )
        .status,
        401
    );
}
#[tokio::test]
async fn csrf_limits_ownership_and_secret_storage() {
    let (dir, runtime, user, token) = setup().await;
    assert_eq!(
        respond(
            runtime
                .runtime
                .handle(request("POST", "/auth/logout", Some(&token)))
                .await
        )
        .status,
        403
    );
    let mut cross = mutation("POST", "/auth/logout", &token);
    cross.headers.retain(|(k, _)| k != "origin");
    cross
        .headers
        .push(("origin".into(), "https://evil.example".into()));
    assert_eq!(respond(runtime.runtime.handle(cross).await).status, 403);
    let mut oversized = request("POST", "/auth/api/keys", Some(&token));
    oversized.body = vec![0; 65537];
    assert_eq!(respond(runtime.runtime.handle(oversized).await).status, 413);
    let mut duplicate = request("GET", "/private", Some(&token));
    duplicate
        .headers
        .push(("cookie".into(), format!("{SESSION_COOKIE}={token}")));
    assert_eq!(respond(runtime.runtime.handle(duplicate).await).status, 400);
    assert!(matches!(
        runtime
            .runtime
            .handle(request("GET", "/authentication", None))
            .await,
        AuthOutcome::Continue { .. }
    ));
    let html = respond(runtime.runtime.handle(request("GET", "/auth/", None)).await);
    assert_eq!(html.status, 200);
    assert!(
        html.headers
            .iter()
            .any(|(k, v)| k == "content-security-policy" && v.contains("frame-ancestors 'none'"))
    );
    let head = respond(
        runtime
            .runtime
            .handle(request("HEAD", "/auth/", None))
            .await,
    );
    assert!(head.body.is_empty());
    let (_, key) = runtime
        .runtime
        .auth
        .create_api_key(
            &runtime
                .runtime
                .auth
                .authenticate(Some(&token), None)
                .await
                .unwrap()
                .unwrap()
                .identity,
            "test".into(),
            vec![],
            None,
        )
        .await
        .unwrap();
    let sessions = runtime
        .runtime
        .auth
        .store
        .list_sessions(&user.id)
        .await
        .unwrap();
    assert_ne!(sessions[0].token_hash, token);
    assert_eq!(sessions[0].token_hash, hash(&token));
    for entry in std::fs::read_dir(dir.path()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains(&token));
        assert!(!text.contains(key.expose()));
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hundred_concurrent_sessions_keys_and_revocations() {
    let (_dir, runtime, user, token) = setup().await;
    let identity = runtime
        .runtime
        .auth
        .authenticate(Some(&token), None)
        .await
        .unwrap()
        .unwrap()
        .identity;
    let mut tasks = tokio::task::JoinSet::new();
    for i in 0..100 {
        let auth = runtime.runtime.auth.clone();
        let user = user.id.clone();
        let identity = identity.clone();
        tasks.spawn(async move {
            auth.create_session(&user).await.unwrap();
            auth.create_api_key(&identity, format!("key-{i}"), vec![], None)
                .await
                .unwrap();
        });
    }
    while let Some(r) = tasks.join_next().await {
        r.unwrap();
    }
    assert_eq!(
        runtime
            .runtime
            .auth
            .store
            .list_sessions(&user.id)
            .await
            .unwrap()
            .len(),
        101
    );
    let keys = runtime
        .runtime
        .auth
        .store
        .list_api_keys(&user.id)
        .await
        .unwrap();
    assert_eq!(keys.len(), 100);
    for key in keys {
        let auth = runtime.runtime.auth.clone();
        let user = user.id.clone();
        tasks.spawn(async move {
            auth.revoke_api_key(&user, &key.id).await.unwrap();
        });
    }
    while let Some(r) = tasks.join_next().await {
        r.unwrap();
    }
    assert!(
        runtime
            .runtime
            .auth
            .store
            .list_api_keys(&user.id)
            .await
            .unwrap()
            .iter()
            .all(|k| k.revoked_at.is_some())
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transaction_is_bound_single_use_and_identity_resolution_is_atomic() {
    let (_dir, runtime, user, _) = setup().await;
    let transaction = OidcTransaction {
        state_hash: hash("state"),
        browser_hash: hash("browser"),
        provider: "default".into(),
        nonce: "nonce".into(),
        pkce_verifier: "verifier".into(),
        return_to: "/".into(),
        expires_at: now() + 60,
    };
    runtime
        .runtime
        .auth
        .store
        .create_oidc_transaction(&transaction)
        .await
        .unwrap();
    assert!(
        runtime
            .runtime
            .auth
            .store
            .consume_oidc_transaction(&hash("state"), &hash("other-browser"), now())
            .await
            .unwrap()
            .is_none()
    );
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let store = runtime.runtime.auth.store.clone();
        tasks.spawn(async move {
            store
                .consume_oidc_transaction(&hash("state"), &hash("browser"), now())
                .await
                .unwrap()
                .is_some()
        });
    }
    let mut count = 0;
    while let Some(r) = tasks.join_next().await {
        count += u32::from(r.unwrap());
    }
    assert_eq!(count, 1);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..50 {
        let store = runtime.runtime.auth.store.clone();
        let new_user = User {
            id: random_token(),
            email: user.email.clone(),
            created_at: now(),
        };
        tasks.spawn(async move {
            store
                .resolve_identity(
                    &ExternalIdentity {
                        issuer: "https://other-issuer.example".into(),
                        subject: "same-subject".into(),
                        email: new_user.email.clone(),
                        metadata: json!({}),
                    },
                    &new_user,
                )
                .await
                .unwrap()
                .id
        });
    }
    let mut ids = std::collections::HashSet::new();
    while let Some(r) = tasks.join_next().await {
        ids.insert(r.unwrap());
    }
    assert_eq!(ids.len(), 1);
    assert!(!ids.contains(&user.id));
}
#[tokio::test]
async fn migrations_reopen_and_expiration() {
    let (dir, runtime, user, token) = setup().await;
    let mut expired = storage::Session {
        id: random_token(),
        user_id: user.id.clone(),
        token_hash: hash("expired"),
        created_at: now() - 10,
        expires_at: now(),
        revoked_at: None,
        last_used_at: None,
    };
    runtime
        .runtime
        .auth
        .store
        .create_session(&expired)
        .await
        .unwrap();
    assert!(
        runtime
            .runtime
            .auth
            .store
            .find_session(&expired.token_hash, now())
            .await
            .unwrap()
            .is_none()
    );
    expired.id = random_token();
    expired.token_hash = hash("revoked");
    expired.expires_at = now() + 60;
    runtime
        .runtime
        .auth
        .store
        .create_session(&expired)
        .await
        .unwrap();
    runtime
        .runtime
        .auth
        .store
        .revoke_session(&user.id, Some(&expired.id), now())
        .await
        .unwrap();
    assert!(
        runtime
            .runtime
            .auth
            .store
            .find_session(&expired.token_hash, now())
            .await
            .unwrap()
            .is_none()
    );
    drop(runtime);
    let reopened = TursoStore::open(
        dir.path().join("stargate.db").to_str().unwrap(),
        16,
        32,
        None,
    )
    .await
    .unwrap();
    assert!(
        reopened
            .find_session(&hash(&token), now())
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn proxies_and_noncanonical_owned_routes() {
    let (_dir, runtime, _, _) = setup().await;
    let mut req = request("GET", "/", None);
    req.peer_ip = Some("192.0.2.10".parse().unwrap());
    req.headers = vec![("x-forwarded-for".into(), "198.51.100.5".into())];
    assert_eq!(
        req.client_ip(&runtime.runtime.auth).unwrap().to_string(),
        "192.0.2.10"
    );
    let mut config = runtime.runtime.auth.config.clone();
    config.trusted_proxies = vec!["192.0.2.0/24".parse().unwrap()];
    let auth = runtime_core::Auth::new(config, runtime.runtime.auth.store.clone(), None)
        .await
        .unwrap();
    assert_eq!(req.client_ip(&auth).unwrap().to_string(), "198.51.100.5");
    req.headers.clear();
    assert_eq!(req.client_ip(&auth), req.peer_ip);
    for path in ["/%61uth/api/keys", "/auth%2fapi/keys"] {
        assert_eq!(
            respond(runtime.runtime.handle(request("GET", path, None)).await).status,
            400
        );
    }
}

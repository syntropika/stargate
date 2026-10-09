use runtime_core::{now, random_token};
use runtime_http::SESSION_COOKIE;
use rust::*;
use serde_json::{Value, json};
use storage::{LocalCredential, User};

fn request(method: &str, path: &str, cookies: &str, csrf: &str, body: Value) -> AuthRequest {
    AuthRequest {
        method: method.into(),
        path: path.into(),
        query: None,
        headers: vec![
            ("origin".into(), "https://app.example.com".into()),
            ("content-type".into(), "application/json".into()),
            ("cookie".into(), cookies.into()),
            ("x-stargate-csrf".into(), csrf.into()),
        ],
        body: if body.is_null() {
            vec![]
        } else {
            serde_json::to_vec(&body).unwrap()
        },
        peer_ip: Some("127.0.0.1".parse().unwrap()),
    }
}
async fn send(app: &Stargate, request: AuthRequest, status: u16) -> AuthResponse {
    let AuthOutcome::Respond { response } = app.runtime.handle(request).await else {
        panic!("expected owned route");
    };
    assert_eq!(
        response.status,
        status,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    response
}
fn body(response: &AuthResponse) -> Value {
    serde_json::from_slice(&response.body).unwrap()
}
fn cookie(response: &AuthResponse, name: &str) -> String {
    response
        .headers
        .iter()
        .find(|(k, v)| k == "set-cookie" && v.starts_with(name))
        .unwrap()
        .1
        .split(';')
        .next()
        .unwrap()
        .into()
}
async fn local(attempts: u32) -> (tempfile::TempDir, Stargate) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new(
            dir.path().join("local.db").to_str().unwrap(),
        )),
    );
    config.local = Some(LocalConfig {
        login_attempts: attempts,
        ..LocalConfig::default()
    });
    config.session.scopes = vec!["read".into()];
    (dir, Stargate::new(config).await.unwrap())
}
async fn bootstrap(app: &Stargate) -> (String, String, Value) {
    let init = send(
        app,
        request("GET", "/auth/api/local", "", "", Value::Null),
        200,
    )
    .await;
    assert_eq!(body(&init)["setup_available"], true);
    let cookie = cookie(&init, "__Host-stargate-local-csrf");
    let csrf = body(&init)["csrf_token"].as_str().unwrap().to_owned();
    let registered = send(
        app,
        request(
            "POST",
            "/auth/api/local/setup",
            &cookie,
            &csrf,
            json!({"email":" ADMIN@Example.com ","password":"initial password 123"}),
        ),
        201,
    )
    .await;
    let session = self::cookie(&registered, SESSION_COOKIE);
    let me = body(
        &send(
            app,
            request("GET", "/auth/api/me", &session, "", Value::Null),
            200,
        )
        .await,
    );
    (session, me["csrf_token"].as_str().unwrap().into(), me)
}
#[tokio::test]
async fn setup_closes_persists_and_uses_csrf_and_hashed_credentials() {
    let (dir, app) = local(10).await;
    send(
        &app,
        request(
            "POST",
            "/auth/api/local/setup",
            "",
            "",
            json!({"email":"admin@example.com","password":"initial password 123"}),
        ),
        403,
    )
    .await;
    let (_, _, me) = bootstrap(&app).await;
    assert_eq!(
        me["identity"]["claims"]["stargate"]["role"],
        "administrator"
    );
    assert_eq!(me["identity"]["email"], "admin@example.com");
    let credential = app
        .runtime
        .auth
        .store
        .find_local_credential("admin@example.com")
        .await
        .unwrap()
        .unwrap();
    assert!(credential.password_hash.starts_with("$argon2id$"));
    assert!(!credential.password_hash.contains("initial password"));
    let init = send(
        &app,
        request("GET", "/auth/api/local", "", "", Value::Null),
        200,
    )
    .await;
    assert_eq!(body(&init)["setup_available"], false);
    let cookies = cookie(&init, "__Host-stargate-local-csrf");
    let csrf = body(&init)["csrf_token"].as_str().unwrap().to_owned();
    let mut cross_origin = request(
        "POST",
        "/auth/api/local/login",
        &cookies,
        &csrf,
        json!({"email":"admin@example.com","password":"initial password 123"}),
    );
    cross_origin.headers[0].1 = "https://other.example.com".into();
    send(&app, cross_origin, 403).await;
    send(
        &app,
        request(
            "POST",
            "/auth/api/local/setup",
            &cookies,
            &csrf,
            json!({"email":"other@example.com","password":"initial password 123"}),
        ),
        409,
    )
    .await;
    drop(app);
    let mut config = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new(
            dir.path().join("local.db").to_str().unwrap(),
        )),
    );
    config.local = Some(LocalConfig::default());
    let reopened = Stargate::new(config).await.unwrap();
    assert!(!reopened.runtime.auth.local_setup_available().await.unwrap());
}
#[tokio::test]
async fn administration_protects_last_admin_and_disables_all_credentials() {
    let (_dir, app) = local(20).await;
    let (admin, csrf, me) = bootstrap(&app).await;
    let admin_id = me["identity"]["user_id"].as_str().unwrap();
    let key = body(
        &send(
            &app,
            request(
                "POST",
                "/auth/api/keys",
                &admin,
                &csrf,
                json!({"name":"admin key"}),
            ),
            201,
        )
        .await,
    )["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut bearer = request("GET", "/auth/api/users", "", "", Value::Null);
    bearer
        .headers
        .push(("authorization".into(), format!("Bearer {key}")));
    send(&app, bearer, 403).await;
    let listed = body(
        &send(
            &app,
            request("GET", "/auth/api/users", &admin, "", Value::Null),
            200,
        )
        .await,
    );
    assert!(!listed.to_string().contains("password"));
    send(
        &app,
        request(
            "PATCH",
            &format!("/auth/api/users/{admin_id}"),
            &admin,
            &csrf,
            json!({"role":"user","disabled":false}),
        ),
        409,
    )
    .await;
    send(
        &app,
        request(
            "PATCH",
            &format!("/auth/api/users/{admin_id}"),
            &admin,
            &csrf,
            json!({"role":"administrator","disabled":true}),
        ),
        409,
    )
    .await;
    let created = send(
        &app,
        request(
            "POST",
            "/auth/api/users",
            &admin,
            &csrf,
            json!({"email":"member@example.com","password":"member password 123"}),
        ),
        201,
    )
    .await;
    let id = body(&created)["id"].as_str().unwrap().to_owned();
    send(
        &app,
        request(
            "POST",
            "/auth/api/users",
            &admin,
            &csrf,
            json!({"email":"MEMBER@EXAMPLE.COM","password":"member password 123"}),
        ),
        409,
    )
    .await;
    let init = send(
        &app,
        request("GET", "/auth/api/local", "", "", Value::Null),
        200,
    )
    .await;
    let cookies = cookie(&init, "__Host-stargate-local-csrf");
    let anon = body(&init)["csrf_token"].as_str().unwrap().to_owned();
    let logged = send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"member@example.com","password":"member password 123"}),
        ),
        200,
    )
    .await;
    let member = cookie(&logged, SESSION_COOKIE);
    let member_me = body(
        &send(
            &app,
            request("GET", "/auth/api/me", &member, "", Value::Null),
            200,
        )
        .await,
    );
    let member_csrf = member_me["csrf_token"].as_str().unwrap();
    send(
        &app,
        request("GET", "/auth/api/users", &member, "", Value::Null),
        403,
    )
    .await;
    send(&app,request("POST","/auth/api/users",&member,member_csrf,json!({"email":"intruder@example.com","password":"member password 123","role":"administrator"})),403).await;
    let key = body(
        &send(
            &app,
            request(
                "POST",
                "/auth/api/keys",
                &member,
                member_csrf,
                json!({"name":"test","scopes":["read"]}),
            ),
            201,
        )
        .await,
    )["secret"]
        .as_str()
        .unwrap()
        .to_owned();
    send(
        &app,
        request(
            "PATCH",
            &format!("/auth/api/users/{id}"),
            &admin,
            &csrf,
            json!({"role":"user","disabled":true}),
        ),
        200,
    )
    .await;
    send(
        &app,
        request("GET", "/auth/api/me", &member, "", Value::Null),
        401,
    )
    .await;
    let mut bearer = request("GET", "/auth/api/users", "", "", Value::Null);
    bearer
        .headers
        .push(("authorization".into(), format!("Bearer {key}")));
    send(&app, bearer, 401).await;
    send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"member@example.com","password":"member password 123"}),
        ),
        401,
    )
    .await;
    send(
        &app,
        request(
            "PATCH",
            &format!("/auth/api/users/{id}"),
            &admin,
            &csrf,
            json!({"role":"user","disabled":false}),
        ),
        200,
    )
    .await;
    send(
        &app,
        request("GET", "/auth/api/me", &member, "", Value::Null),
        401,
    )
    .await;
    let logged = send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"member@example.com","password":"member password 123"}),
        ),
        200,
    )
    .await;
    let member = cookie(&logged, SESSION_COOKIE);
    send(
        &app,
        request(
            "DELETE",
            &format!("/auth/api/users/{id}/access"),
            &admin,
            &csrf,
            Value::Null,
        ),
        204,
    )
    .await;
    send(
        &app,
        request("GET", "/auth/api/me", &member, "", Value::Null),
        401,
    )
    .await;
}
#[tokio::test]
async fn password_change_rotates_sessions_and_rate_limits_login() {
    let (_dir, app) = local(3).await;
    let (session, csrf, _) = bootstrap(&app).await;
    let rotated=send(&app,request("POST","/auth/api/local/password",&session,&csrf,json!({"current_password":"initial password 123","new_password":"replacement password 456"})),200).await;
    let replacement = cookie(&rotated, SESSION_COOKIE);
    send(
        &app,
        request("GET", "/auth/api/me", &session, "", Value::Null),
        401,
    )
    .await;
    send(
        &app,
        request("GET", "/auth/api/me", &replacement, "", Value::Null),
        200,
    )
    .await;
    let init = send(
        &app,
        request("GET", "/auth/api/local", "", "", Value::Null),
        200,
    )
    .await;
    let cookies = cookie(&init, "__Host-stargate-local-csrf");
    let anon = body(&init)["csrf_token"].as_str().unwrap().to_owned();
    send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"admin@example.com","password":"initial password 123"}),
        ),
        401,
    )
    .await;
    send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"admin@example.com","password":"replacement password 456"}),
        ),
        200,
    )
    .await;
    let limited = send(
        &app,
        request(
            "POST",
            "/auth/api/local/login",
            &cookies,
            &anon,
            json!({"email":"admin@example.com","password":"replacement password 456"}),
        ),
        429,
    )
    .await;
    assert!(limited.headers.iter().any(|(key, _)| key == "retry-after"));
}
#[tokio::test]
async fn simultaneous_setup_and_admin_changes_are_serialized() {
    let (_dir, app) = local(100).await;
    let store = app.runtime.auth.store.clone();
    let mut tasks = vec![];
    for i in 0..24 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            let user = User {
                id: random_token(),
                email: Some(format!("admin{i}@example.com")),
                created_at: now(),
                role: UserRole::Administrator,
                disabled_at: None,
            };
            let credential = LocalCredential {
                user_id: user.id.clone(),
                email: user.email.clone().unwrap(),
                password_hash: "storage-only concurrency fixture".into(),
            };
            store
                .create_local_user(&user, &credential, None, None)
                .await
                .unwrap()
        }));
    }
    let mut winners = vec![];
    for task in tasks {
        if let Some(user) = task.await.unwrap() {
            winners.push(user);
        }
    }
    assert_eq!(winners.len(), 1);
    let first = &winners[0];
    let second = User {
        id: random_token(),
        email: Some("second@example.com".into()),
        created_at: now(),
        role: UserRole::Administrator,
        disabled_at: None,
    };
    let credential = LocalCredential {
        user_id: second.id.clone(),
        email: second.email.clone().unwrap(),
        password_hash: "fixture".into(),
    };
    store
        .create_local_user(&second, &credential, None, Some(&first.id))
        .await
        .unwrap()
        .unwrap();
    let (a, b) = tokio::join!(
        store.update_user(&first.id, &first.id, UserRole::User, None),
        store.update_user(&second.id, &second.id, UserRole::User, None)
    );
    assert_eq!(
        usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
        1
    );
    assert_eq!(
        store
            .list_users(None)
            .await
            .unwrap()
            .iter()
            .filter(|u| u.role == UserRole::Administrator && u.disabled_at.is_none())
            .count(),
        1
    );
}

#[tokio::test]
async fn password_updates_reject_stale_logins_and_only_one_concurrent_change_commits() {
    let (_dir, app) = local(10).await;
    let (_, _, me) = bootstrap(&app).await;
    let store = &app.runtime.auth.store;
    let user = me["identity"]["user_id"].as_str().unwrap();
    let original = store.get_local_credential(user).await.unwrap().unwrap();
    let make_session = || storage::Session {
        id: random_token(),
        user_id: user.into(),
        token_hash: random_token(),
        created_at: now(),
        expires_at: now() + 60,
        revoked_at: None,
        last_used_at: None,
    };
    let first = make_session();
    let second = make_session();
    let (a, b) = tokio::join!(
        store.change_local_password(
            user,
            &original.password_hash,
            "first replacement",
            &first,
            now()
        ),
        store.change_local_password(
            user,
            &original.password_hash,
            "second replacement",
            &second,
            now()
        ),
    );
    assert_eq!(usize::from(a.unwrap()) + usize::from(b.unwrap()), 1);
    assert!(
        !store
            .create_local_session(&original.password_hash, &make_session())
            .await
            .unwrap()
    );
    let active = store
        .list_sessions(user)
        .await
        .unwrap()
        .into_iter()
        .filter(|s| s.revoked_at.is_none())
        .count();
    assert_eq!(active, 1);
    let current = store.get_local_credential(user).await.unwrap().unwrap();
    assert!(
        !store
            .change_local_password(
                user,
                "stale hash",
                &current.password_hash,
                &make_session(),
                now()
            )
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .list_sessions(user)
            .await
            .unwrap()
            .into_iter()
            .filter(|s| s.revoked_at.is_none())
            .count(),
        1
    );
    let external = User {
        id: random_token(),
        email: Some("external@example.com".into()),
        created_at: now(),
        role: UserRole::User,
        disabled_at: None,
    };
    let external = store
        .resolve_identity(
            &storage::ExternalIdentity {
                issuer: "https://identity.example.com".into(),
                subject: "external".into(),
                email: external.email.clone(),
                metadata: json!({}),
            },
            &external,
        )
        .await
        .unwrap();
    let credential = LocalCredential {
        user_id: external.id.clone(),
        email: external.email.clone().unwrap(),
        password_hash: "unauthorized fixture".into(),
    };
    assert!(
        store
            .create_local_user(&external, &credential, None, Some("missing administrator"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .get_local_credential(&external.id)
            .await
            .unwrap()
            .is_none()
    );
}

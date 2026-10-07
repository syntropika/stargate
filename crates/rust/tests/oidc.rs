use axum::{
    Json, Router,
    extract::{Form, Query, State},
    http::HeaderMap,
    response::Redirect,
    routing::{get, post},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use rsa::{
    RsaPrivateKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    traits::PublicKeyParts,
};
use runtime_core::{hash, now, random_token};
use runtime_http::{OIDC_COOKIE, SESSION_COOKIE};
use rust::*;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone)]
struct Idp {
    issuer: String,
    key: Arc<jsonwebtoken::EncodingKey>,
    jwk: Value,
    codes: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    mode: Arc<AtomicUsize>,
}
async fn discovery(State(idp): State<Idp>) -> Json<Value> {
    Json(
        json!({"issuer":idp.issuer,"authorization_endpoint":format!("{}/authorize",idp.issuer),"token_endpoint":format!("{}/token",idp.issuer),"jwks_uri":format!("{}/jwks",idp.issuer),"response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"],"token_endpoint_auth_methods_supported":["client_secret_basic"]}),
    )
}
async fn jwks(State(idp): State<Idp>) -> Json<Value> {
    Json(json!({"keys":[idp.jwk]}))
}
async fn authorize(
    State(idp): State<Idp>,
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    assert_eq!(params["code_challenge_method"], "S256");
    assert_eq!(params["response_type"], "code");
    assert!(params["scope"].split(' ').any(|s| s == "openid"));
    let code = random_token();
    let url = format!(
        "{}?code={}&state={}",
        params["redirect_uri"], code, params["state"]
    );
    idp.codes.lock().unwrap().insert(code, params);
    Redirect::to(&url)
}
async fn token(
    State(idp): State<Idp>,
    headers: HeaderMap,
    Form(params): Form<HashMap<String, String>>,
) -> Json<Value> {
    assert_eq!(
        headers["authorization"],
        format!("Basic {}", STANDARD.encode("client:secret"))
    );
    assert_eq!(params["grant_type"], "authorization_code");
    let flow = idp.codes.lock().unwrap().remove(&params["code"]).unwrap();
    assert_eq!(hash(&params["code_verifier"]), flow["code_challenge"]);
    assert_eq!(params["redirect_uri"], flow["redirect_uri"]);
    let mode = idp.mode.load(Ordering::Relaxed);
    let claims = json!({"iss":if mode==1 {"https://wrong-issuer.example"} else {&idp.issuer},"sub":"subject-123","aud":if mode==2 {"wrong-audience"} else {"client"},"exp":if mode==3 {now()-60} else {now()+600},"iat":now(),"nonce":if mode==4 {"wrong-nonce"} else {&flow["nonce"]},"email":"person@example.com","email_verified":true,"updated_at":if mode==6 {json!(now())} else if mode==7 {json!("not-a-timestamp")} else {json!("2026-10-07T12:00:00.000Z")}});
    let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
    header.kid = Some("test-key".into());
    let key = if mode == 5 {
        let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
        Arc::new(
            jsonwebtoken::EncodingKey::from_rsa_pem(
                key.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes(),
            )
            .unwrap(),
        )
    } else {
        idp.key
    };
    let jwt = jsonwebtoken::encode(&header, &claims, &key).unwrap();
    Json(json!({"access_token":"mock-access-token","token_type":"Bearer","id_token":jwt}))
}
fn req(path: &str) -> AuthRequest {
    AuthRequest {
        method: "GET".into(),
        path: path.into(),
        query: None,
        headers: vec![],
        body: vec![],
        peer_ip: None,
    }
}
fn response(out: AuthOutcome) -> AuthResponse {
    match out {
        AuthOutcome::Respond { response } => response,
        _ => panic!(),
    }
}
fn location(r: &AuthResponse) -> String {
    r.headers
        .iter()
        .find(|(k, _)| k == "location")
        .unwrap()
        .1
        .clone()
}
fn cookie(r: &AuthResponse, name: &str) -> String {
    r.headers
        .iter()
        .find(|(k, v)| k == "set-cookie" && v.starts_with(name))
        .unwrap()
        .1
        .split(';')
        .next()
        .unwrap()
        .into()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn oidc_pkce_browser_binding_replay_and_token_validation() {
    let key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
    let public = key.to_public_key();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let idp = Idp {
        issuer: issuer.clone(),
        key: Arc::new(
            jsonwebtoken::EncodingKey::from_rsa_pem(
                key.to_pkcs8_pem(LineEnding::LF).unwrap().as_bytes(),
            )
            .unwrap(),
        ),
        jwk: json!({"kty":"RSA","use":"sig","alg":"RS256","kid":"test-key","n":URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(public.e().to_bytes_be())}),
        codes: Arc::new(Mutex::new(HashMap::new())),
        mode: Arc::new(AtomicUsize::new(0)),
    };
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .with_state(idp.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut config = AuthConfig::new(
        "https://app.example.com".parse().unwrap(),
        StorageConfig::Turso(TursoConfig::new(
            dir.path().join("stargate.db").to_str().unwrap(),
        )),
    );
    config.allow_insecure_loopback = true;
    config.oidc = vec![OidcProviderConfig {
        name: "test".into(),
        issuer: IssuerUrl::new(issuer).unwrap(),
        client_id: "client".into(),
        client_secret: Secret::new("secret"),
    }];
    let auth = Stargate::new(config).await.unwrap();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    for mode in 0..=7 {
        idp.mode.store(mode, Ordering::Relaxed);
        let mut start = req("/auth/login");
        start.query = Some("return_to=/private".into());
        let login = response(auth.runtime.handle(start).await);
        assert_eq!(login.status, 303);
        let browser = cookie(&login, OIDC_COOKIE);
        assert!(login.headers.iter().any(
            |(k, v)| k == "set-cookie" && v.contains("HttpOnly; Secure; SameSite=Lax; Path=/")
        ));
        let provider = http.get(location(&login)).send().await.unwrap();
        let redirect = provider.headers()["location"].to_str().unwrap();
        let target = url::Url::parse(redirect).unwrap();
        let mut callback = req("/auth/callback");
        callback.query = target.query().map(str::to_owned);
        assert_eq!(
            response(auth.runtime.handle(callback.clone()).await).status,
            401
        );
        callback
            .headers
            .push(("cookie".into(), format!("{OIDC_COOKIE}={}", random_token())));
        assert_eq!(
            response(auth.runtime.handle(callback.clone()).await).status,
            401
        );
        callback.headers = vec![("cookie".into(), browser)];
        let completed = response(auth.runtime.handle(callback.clone()).await);
        if mode == 0 || mode == 6 {
            assert_eq!(completed.status, 303);
            assert_eq!(location(&completed), "/private");
            let session = cookie(&completed, SESSION_COOKIE);
            let mut private = req("/private");
            private.headers.push(("cookie".into(), session));
            assert!(matches!(
                auth.runtime.handle(private).await,
                AuthOutcome::Continue {
                    identity: Some(_),
                    ..
                }
            ));
        } else {
            assert_eq!(completed.status, 502, "mode {mode}");
            assert!(!completed.headers.iter().any(|(k, _)| k == "set-cookie"));
        }
        assert_eq!(response(auth.runtime.handle(callback).await).status, 401);
    }
    let mut bad = req("/auth/login");
    bad.query = Some("return_to=//evil.example".into());
    assert_eq!(response(auth.runtime.handle(bad).await).status, 400);
    task.abort();
}

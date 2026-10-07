//! Framework-neutral HTTP boundary and owned routing.
use runtime_core::{Auth, AuthType, Authentication, Error, Identity, Policy, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, net::IpAddr, sync::Arc};
use subtle::ConstantTimeEq;

pub const SESSION_COOKIE: &str = "__Host-stargate-session";
pub const OIDC_COOKIE: &str = "__Host-stargate-oidc";
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthRequest {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub body: Vec<u8>,
    #[serde(default)]
    pub peer_ip: Option<IpAddr>,
}
impl AuthRequest {
    pub fn header(&self, name: &str) -> Result<Option<&str>> {
        let mut values = self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str());
        let first = values.next();
        if values.next().is_some() {
            return Err(Error::BadRequest);
        }
        Ok(first)
    }
    pub fn cookie(&self, name: &str) -> Result<Option<&str>> {
        let mut found = None;
        for (_, value) in self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case("cookie"))
        {
            for part in value.split(';') {
                if let Some((key, value)) = part.trim().split_once('=')
                    && key == name
                {
                    if found.is_some() {
                        return Err(Error::BadRequest);
                    }
                    found = Some(value);
                }
            }
        }
        Ok(found)
    }
    pub fn params(&self) -> Result<HashMap<String, String>> {
        let mut out = HashMap::new();
        for (key, value) in
            url::form_urlencoded::parse(self.query.as_deref().unwrap_or("").as_bytes())
        {
            if out.insert(key.into_owned(), value.into_owned()).is_some() {
                return Err(Error::BadRequest);
            }
        }
        Ok(out)
    }
    pub fn client_ip(&self, auth: &Auth) -> Option<IpAddr> {
        let peer = self.peer_ip?;
        if !auth
            .config
            .trusted_proxies
            .iter()
            .any(|p| p.contains(&peer))
        {
            return Some(peer);
        }
        let Some(forwarded) = self.header("x-forwarded-for").ok().flatten() else {
            return Some(peer);
        };
        let mut current = peer;
        for part in forwarded.rsplit(',') {
            if !auth
                .config
                .trusted_proxies
                .iter()
                .any(|p| p.contains(&current))
            {
                break;
            }
            current = part.trim().parse().ok()?;
        }
        Some(current)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthOutcome {
    Respond {
        response: AuthResponse,
    },
    Continue {
        identity: Option<Identity>,
        response_headers: Vec<(String, String)>,
    },
}
#[derive(Clone)]
pub struct Runtime {
    pub auth: Arc<Auth>,
}
impl Runtime {
    pub fn new(auth: Arc<Auth>) -> Self {
        Self { auth }
    }
    pub fn owns(&self, path: &str) -> bool {
        path == self.auth.config.path_prefix
            || path.starts_with(&format!("{}/", self.auth.config.path_prefix))
    }
    fn validate(&self, req: &AuthRequest) -> Result<()> {
        let header_bytes = req
            .headers
            .iter()
            .try_fold(0usize, |total, (k, v)| total.checked_add(k.len() + v.len()))
            .ok_or(Error::BadRequest)?;
        if req.body.len() > self.auth.config.max_body_bytes
            || header_bytes > self.auth.config.max_header_bytes
            || req.path.len() + req.query.as_ref().map_or(0, String::len) > 8192
        {
            return Err(Error::TooLarge);
        }
        http_types::Method::from_bytes(req.method.as_bytes()).map_err(|_| Error::BadRequest)?;
        if !req.path.starts_with('/')
            || req.path.starts_with("//")
            || req
                .path
                .bytes()
                .any(|b| b <= 32 || b >= 127 || matches!(b, b'?' | b'#' | b'\\'))
        {
            return Err(Error::BadRequest);
        }
        let decoded = percent_encoding::percent_decode_str(&req.path)
            .decode_utf8()
            .map_err(|_| Error::BadRequest)?;
        if decoded != req.path && self.owns(&decoded) {
            return Err(Error::BadRequest);
        }
        for (key, value) in &req.headers {
            if !value.is_ascii() {
                return Err(Error::BadRequest);
            }
            http_types::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| Error::BadRequest)?;
            http_types::header::HeaderValue::from_str(value).map_err(|_| Error::BadRequest)?;
        }
        Ok(())
    }
    pub async fn handle(&self, req: AuthRequest) -> AuthOutcome {
        let is_head = req.method == "HEAD";
        let result = self.dispatch(&req).await;
        let mut outcome = result.unwrap_or_else(|error| {
            let status = match error {
                Error::Config | Error::BadRequest => 400,
                Error::TooLarge => 413,
                Error::Unauthorized => 401,
                Error::Forbidden => 403,
                Error::Oidc => 502,
                Error::Storage(_) => 503,
            };
            AuthOutcome::Respond {
                response: response(
                    status,
                    "application/json",
                    serde_json::to_vec(&json!({"error":error.to_string()})).unwrap_or_default(),
                ),
            }
        });
        if let AuthOutcome::Respond { response } = &mut outcome
            && is_head
        {
            response.body.clear();
        }
        outcome
    }
    async fn dispatch(&self, req: &AuthRequest) -> Result<AuthOutcome> {
        self.validate(req)?;
        if req.body.len() > self.auth.config.max_body_bytes {
            return Err(Error::TooLarge);
        }
        let owned = self.owns(&req.path);
        let route = if owned {
            &req.path[self.auth.config.path_prefix.len()..]
        } else {
            ""
        };
        if owned && (req.method == "GET" || req.method == "HEAD") {
            if route == "/assets/app.js" {
                return Ok(respond(response(
                    200,
                    "text/javascript; charset=utf-8",
                    ui::JAVASCRIPT.as_bytes().to_vec(),
                )));
            }
            if route == "/assets/app.css" {
                return Ok(respond(response(
                    200,
                    "text/css; charset=utf-8",
                    ui::STYLESHEET
                        .replace("#315cfd", &self.auth.config.branding.accent)
                        .into_bytes(),
                )));
            }
            if ["", "/", "/profile", "/keys", "/sessions", "/logout"].contains(&route) {
                return Ok(respond(response(
                    200,
                    "text/html; charset=utf-8",
                    ui::page(
                        &self.auth.config.path_prefix,
                        &self.auth.config.branding.app_name,
                        self.auth.config.branding.logo.as_ref().map(|u| u.as_str()),
                    )
                    .into_bytes(),
                )));
            }
            if route == "/api/config" {
                return Ok(json_response(
                    200,
                    json!({"app_name":self.auth.config.branding.app_name,"providers":self.auth.config.oidc.iter().map(|p|&p.name).collect::<Vec<_>>()}),
                ));
            }
            if route == "/login" {
                let p = req.params()?;
                let (location, browser) = self
                    .auth
                    .login(
                        p.get("provider").map(String::as_str),
                        p.get("return_to").map_or("/", String::as_str),
                    )
                    .await?;
                let mut r = redirect(&location);
                r.headers.push((
                    "set-cookie".into(),
                    cookie(OIDC_COOKIE, browser.expose(), 600),
                ));
                return Ok(respond(r));
            }
            if route == "/callback" {
                let p = req.params()?;
                let state = p.get("state").ok_or(Error::BadRequest)?;
                let browser = req.cookie(OIDC_COOKIE)?.ok_or(Error::Unauthorized)?;
                let (session, return_to) = self
                    .auth
                    .callback(state, p.get("code").map_or("", String::as_str), browser)
                    .await?;
                let mut r = redirect(&return_to);
                r.headers.push((
                    "set-cookie".into(),
                    cookie(
                        SESSION_COOKIE,
                        session.expose(),
                        self.auth.config.session.ttl_seconds,
                    ),
                ));
                r.headers
                    .push(("set-cookie".into(), cookie(OIDC_COOKIE, "", 0)));
                return Ok(respond(r));
            }
        }
        let bearer = match req.header("authorization")? {
            None => None,
            Some(value) => {
                let (scheme, token) = value.split_once(' ').ok_or(Error::Unauthorized)?;
                if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() || token.contains(' ')
                {
                    return Err(Error::Unauthorized);
                }
                Some(token)
            }
        };
        let authentication = self
            .auth
            .authenticate(req.cookie(SESSION_COOKIE)?, bearer)
            .await?;
        if !owned {
            return Ok(AuthOutcome::Continue {
                identity: authentication.map(|a| a.identity),
                response_headers: Vec::new(),
            });
        }
        let context = authentication.ok_or(Error::Unauthorized)?;
        if context.identity.auth_type != AuthType::Session {
            return Err(Error::Forbidden);
        }
        let user = context
            .identity
            .user_id
            .as_deref()
            .ok_or(Error::Forbidden)?;
        if !matches!(req.method.as_str(), "GET" | "HEAD" | "OPTIONS") {
            self.csrf(req, &context)?;
        }
        match (req.method.as_str(), route) {
            ("GET" | "HEAD", "/api/me") => Ok(json_response(
                200,
                json!({"identity":context.identity,"csrf_token":context.csrf_token.as_ref().map(|s|s.expose()),"session_id":context.session_id}),
            )),
            ("GET" | "HEAD", "/api/sessions") => {
                let data=self.auth.store.list_sessions(user).await?.iter().map(|s|json!({"id":s.id,"created_at":s.created_at,"expires_at":s.expires_at,"last_used_at":s.last_used_at,"revoked_at":s.revoked_at})).collect::<Vec<_>>();
                Ok(json_response(200, json!(data)))
            }
            ("GET" | "HEAD", "/api/keys") => {
                let data = self
                    .auth
                    .store
                    .list_api_keys(user)
                    .await?
                    .iter()
                    .map(public_key)
                    .collect::<Vec<_>>();
                Ok(json_response(200, json!(data)))
            }
            ("POST", "/api/keys") => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct CreateKey {
                    name: String,
                    #[serde(default)]
                    scopes: Vec<String>,
                    #[serde(default)]
                    expires_at: Option<i64>,
                }
                if req
                    .header("content-type")?
                    .and_then(|c| c.split(';').next())
                    .map(str::trim)
                    != Some("application/json")
                {
                    return Err(Error::BadRequest);
                }
                let input: CreateKey =
                    serde_json::from_slice(&req.body).map_err(|_| Error::BadRequest)?;
                let (key, secret) = self
                    .auth
                    .create_api_key(
                        &context.identity,
                        input.name,
                        input.scopes,
                        input.expires_at,
                    )
                    .await?;
                Ok(json_response(
                    201,
                    json!({"key":public_key(&key),"secret":secret.expose()}),
                ))
            }
            ("DELETE", "/api/sessions") => {
                self.auth.revoke_session(user, None).await?;
                let mut r = response(204, "application/json", Vec::new());
                r.headers
                    .push(("set-cookie".into(), cookie(SESSION_COOKIE, "", 0)));
                Ok(respond(r))
            }
            ("POST", "/logout") => {
                self.auth
                    .revoke_session(user, context.session_id.as_deref())
                    .await?;
                self.auth
                    .audit("auth.logout", Some(user), json!({}))
                    .await?;
                let mut r = response(204, "application/json", Vec::new());
                r.headers
                    .push(("set-cookie".into(), cookie(SESSION_COOKIE, "", 0)));
                Ok(respond(r))
            }
            ("DELETE", path) if path.starts_with("/api/keys/") => {
                let id = path.trim_start_matches("/api/keys/");
                validate_id(id)?;
                self.auth.revoke_api_key(user, id).await?;
                Ok(respond(response(204, "application/json", Vec::new())))
            }
            ("DELETE", path) if path.starts_with("/api/sessions/") => {
                let id = path.trim_start_matches("/api/sessions/");
                validate_id(id)?;
                self.auth.revoke_session(user, Some(id)).await?;
                let mut r = response(204, "application/json", Vec::new());
                if context.session_id.as_deref() == Some(id) {
                    r.headers
                        .push(("set-cookie".into(), cookie(SESSION_COOKIE, "", 0)));
                }
                Ok(respond(r))
            }
            _ => Ok(json_response(404, json!({"error":"route not found"}))),
        }
    }
    fn csrf(&self, req: &AuthRequest, context: &Authentication) -> Result<()> {
        if req.header("origin")?
            != Some(
                self.auth
                    .config
                    .base_url
                    .origin()
                    .ascii_serialization()
                    .as_str(),
            )
        {
            return Err(Error::Forbidden);
        }
        let supplied = req.header("x-stargate-csrf")?.ok_or(Error::Forbidden)?;
        let expected = context
            .csrf_token
            .as_ref()
            .ok_or(Error::Forbidden)?
            .expose();
        if !bool::from(supplied.as_bytes().ct_eq(expected.as_bytes())) {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
    pub async fn authorize(&self, identity: Option<&Identity>, policy: &Policy) -> Result<()> {
        self.auth.authorize(identity, policy).await
    }
}
fn validate_id(id: &str) -> Result<()> {
    if id.len() != 43
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::BadRequest);
    }
    Ok(())
}
fn public_key(k: &storage::ApiKey) -> Value {
    json!({"id":k.id,"name":k.name,"prefix":k.prefix,"scopes":k.scopes,"created_at":k.created_at,"expires_at":k.expires_at,"last_used_at":k.last_used_at,"revoked_at":k.revoked_at})
}
fn cookie(name: &str, value: &str, max_age: u32) -> String {
    format!("{name}={value}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age}")
}
fn respond(response: AuthResponse) -> AuthOutcome {
    AuthOutcome::Respond { response }
}
pub fn response(status: u16, content_type: &str, body: Vec<u8>) -> AuthResponse {
    AuthResponse {status,body,headers:vec![("content-type".into(),content_type.into()),("cache-control".into(),"no-store".into()),("x-content-type-options".into(),"nosniff".into()),("referrer-policy".into(),"no-referrer".into()),("content-security-policy".into(),"default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' https:; base-uri 'none'; frame-ancestors 'none'; form-action 'self'".into()),("x-frame-options".into(),"DENY".into())] }
}
fn json_response(status: u16, value: Value) -> AuthOutcome {
    respond(response(
        status,
        "application/json",
        serde_json::to_vec(&value).unwrap_or_default(),
    ))
}
fn redirect(location: &str) -> AuthResponse {
    let mut r = response(303, "text/plain", Vec::new());
    r.headers.push(("location".into(), location.into()));
    r
}

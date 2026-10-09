use super::*;
use runtime_core::Secret;
use serde::de::DeserializeOwned;
use storage::UserRole;

const LOCAL_CSRF_COOKIE: &str = "__Host-stargate-local-csrf";
fn input<T: DeserializeOwned>(req: &AuthRequest) -> Result<T> {
    if req
        .header("content-type")?
        .and_then(|c| c.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err(Error::BadRequest);
    }
    serde_json::from_slice(&req.body).map_err(|_| Error::BadRequest)
}
impl Runtime {
    pub(super) async fn local_public(
        &self,
        req: &AuthRequest,
        route: &str,
    ) -> Result<Option<AuthOutcome>> {
        if route == "/api/local" && matches!(req.method.as_str(), "GET" | "HEAD") {
            let enabled = self.auth.config.local.is_some();
            let setup = self.auth.local_setup_available().await?;
            if !enabled {
                return Ok(Some(json_response(
                    200,
                    json!({"enabled":false,"setup_available":false}),
                )));
            }
            let token = match req.cookie(LOCAL_CSRF_COOKIE)? {
                Some(token) => {
                    validate_id(token)?;
                    token.to_owned()
                }
                None => runtime_core::random_token(),
            };
            let mut r = response(200, "application/json", serde_json::to_vec(&json!({"enabled":true,"setup_available":setup,"csrf_token":runtime_core::csrf(&token),"minimum_password_length":12})).map_err(|_| Error::BadRequest)?);
            r.headers
                .push(("set-cookie".into(), cookie(LOCAL_CSRF_COOKIE, &token, 600)));
            return Ok(Some(respond(r)));
        }
        if req.method != "POST" || !matches!(route, "/api/local/setup" | "/api/local/login") {
            return Ok(None);
        }
        if self.auth.config.local.is_none() {
            return Ok(Some(json_response(404, json!({"error":"route not found"}))));
        }
        if req.header("authorization")?.is_some() {
            return Err(Error::Forbidden);
        }
        self.local_csrf(req)?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Credentials {
            email: String,
            password: Secret,
        }
        let value: Credentials = input(req)?;
        let peer = req
            .client_ip(&self.auth)
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "unknown".into());
        let token = if route == "/api/local/setup" {
            self.auth
                .setup_local(&value.email, value.password, &peer)
                .await?
        } else {
            self.auth
                .login_local(&value.email, value.password, &peer)
                .await?
        };
        Ok(Some(self.local_session(
            token,
            if route.ends_with("setup") { 201 } else { 200 },
        )))
    }
    fn local_session(&self, token: Secret, status: u16) -> AuthOutcome {
        let mut r = response(status, "application/json", b"{\"success\":true}".to_vec());
        r.headers.push((
            "set-cookie".into(),
            cookie(
                SESSION_COOKIE,
                token.expose(),
                self.auth.config.session.ttl_seconds,
            ),
        ));
        respond(r)
    }
    fn local_csrf(&self, req: &AuthRequest) -> Result<()> {
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
        let token = req.cookie(LOCAL_CSRF_COOKIE)?.ok_or(Error::Forbidden)?;
        validate_id(token)?;
        let expected = runtime_core::csrf(token);
        let supplied = req.header("x-stargate-csrf")?.ok_or(Error::Forbidden)?;
        if !bool::from(supplied.as_bytes().ct_eq(expected.as_bytes())) {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
    pub(super) async fn local_account(
        &self,
        req: &AuthRequest,
        route: &str,
        context: &Authentication,
    ) -> Result<Option<AuthOutcome>> {
        if route == "/api/local/password" && req.method == "POST" {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct PasswordChange {
                current_password: Secret,
                new_password: Secret,
            }
            let value: PasswordChange = input(req)?;
            let peer = req
                .client_ip(&self.auth)
                .map(|ip| ip.to_string())
                .unwrap_or_else(|| "unknown".into());
            let token = self
                .auth
                .change_password(
                    &context.identity,
                    value.current_password,
                    value.new_password,
                    &peer,
                )
                .await?;
            return Ok(Some(self.local_session(token, 200)));
        }
        if route != "/api/users" && !route.starts_with("/api/users/") {
            return Ok(None);
        }
        let actor = self.auth.require_administrator(&context.identity).await?;
        let outcome = match (req.method.as_str(), route) {
            ("GET" | "HEAD", "/api/users") => {
                let params = req.params()?;
                let after = params.get("after").map(String::as_str);
                if let Some(id) = after {
                    validate_id(id)?;
                }
                let users = self.auth.store.list_users(after).await?;
                let cursor = if users.len() == 100 {
                    users.last().map(|u| &u.id)
                } else {
                    None
                };
                json_response(200, json!({"users":users,"next_cursor":cursor}))
            }
            ("POST", "/api/users") => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct NewUser {
                    email: String,
                    password: Secret,
                    #[serde(default)]
                    role: UserRole,
                }
                let value: NewUser = input(req)?;
                let user = self
                    .auth
                    .create_local_user(&context.identity, &value.email, value.password, value.role)
                    .await?;
                json_response(201, json!(user))
            }
            ("PATCH", path) if path.starts_with("/api/users/") => {
                let id = path.trim_start_matches("/api/users/");
                validate_id(id)?;
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct UpdateUser {
                    role: UserRole,
                    disabled: bool,
                }
                let value: UpdateUser = input(req)?;
                let user = self
                    .auth
                    .update_user(&context.identity, id, value.role, value.disabled)
                    .await?;
                json_response(200, json!(user))
            }
            ("DELETE", path) if path.ends_with("/access") => {
                let id = path
                    .trim_start_matches("/api/users/")
                    .trim_end_matches("/access");
                validate_id(id)?;
                self.auth.revoke_user_access(&context.identity, id).await?;
                let mut r = response(204, "application/json", vec![]);
                if id == actor.id {
                    r.headers
                        .push(("set-cookie".into(), cookie(SESSION_COOKIE, "", 0)));
                }
                respond(r)
            }
            _ => json_response(404, json!({"error":"route not found"})),
        };
        Ok(Some(outcome))
    }
}

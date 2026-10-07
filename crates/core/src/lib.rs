//! Authentication and authorization logic, independent of HTTP frameworks and storage engines.
pub mod config;
mod local;
mod oidc;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
pub use config::*;
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use storage::*;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid configuration")]
    Config,
    #[error("invalid request")]
    BadRequest,
    #[error("request too large")]
    TooLarge,
    #[error("authentication required")]
    Unauthorized,
    #[error("permission denied")]
    Forbidden,
    #[error("account operation conflicts with the current state")]
    Conflict,
    #[error("too many attempts; try again later")]
    TooManyRequests,
    #[error("identity provider unavailable or invalid response")]
    Oidc,
    #[error(transparent)]
    Storage(#[from] StoreError),
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    Session,
    ApiKey,
    BearerJwt,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub subject: String,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub auth_type: AuthType,
    pub scopes: Vec<String>,
    #[serde(default)]
    pub claims: BTreeMap<String, Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Policy {
    Anonymous,
    Authenticated,
    Scopes { scopes: Vec<String> },
}
pub fn authorize(identity: Option<&Identity>, policy: &Policy) -> Result<()> {
    match policy {
        Policy::Anonymous => Ok(()),
        Policy::Authenticated => identity.map(|_| ()).ok_or(Error::Unauthorized),
        Policy::Scopes { scopes } => {
            validate_scopes(scopes)?;
            let id = identity.ok_or(Error::Unauthorized)?;
            if scopes.iter().all(|s| id.scopes.contains(s)) {
                Ok(())
            } else {
                Err(Error::Forbidden)
            }
        }
    }
}
/// Internal context. Never serialized into a host identity or logs.
pub struct Authentication {
    pub identity: Identity,
    pub session_id: Option<String>,
    pub csrf_token: Option<Secret>,
}
pub type AuditSink = Arc<dyn Fn(AuditEvent) + Send + Sync>;
pub struct Auth {
    pub config: AuthConfig,
    pub store: Arc<dyn AuthStore>,
    providers: Vec<oidc::Provider>,
    sink: Option<AuditSink>,
    local: local::LocalRuntime,
}
impl Auth {
    pub async fn new(
        config: AuthConfig,
        store: Arc<dyn AuthStore>,
        sink: Option<AuditSink>,
    ) -> Result<Self> {
        config.validate()?;
        let providers = oidc::discover(&config).await?;
        if let Some(local) = &config.local {
            store
                .prepare_local_accounts(local.initial_admin_user_id.as_deref())
                .await?;
        }
        Ok(Self {
            config,
            store,
            providers,
            sink,
            local: local::LocalRuntime::default(),
        })
    }
    pub async fn audit(&self, event: &str, actor: Option<&str>, metadata: Value) -> Result<()> {
        let record = AuditEvent {
            id: random_token(),
            event: event.into(),
            actor: actor.map(str::to_owned),
            metadata,
            created_at: now(),
        };
        self.store.write_audit_event(&record).await?;
        if let Some(sink) = &self.sink {
            sink(record);
        }
        Ok(())
    }
    pub async fn authorize(&self, identity: Option<&Identity>, policy: &Policy) -> Result<()> {
        let result = authorize(identity, policy);
        if result.is_err() {
            self.audit(
                "auth.authorization.denied",
                identity.and_then(|i| i.user_id.as_deref()),
                json!({}),
            )
            .await?;
        }
        result
    }
    pub async fn authenticate(
        &self,
        session: Option<&str>,
        bearer: Option<&str>,
    ) -> Result<Option<Authentication>> {
        // An explicitly supplied bearer credential takes precedence. Invalid credentials never fall back to cookies.
        let (user_id, auth_type, scopes, session_id, csrf_token) = if let Some(key) = bearer {
            if !key.starts_with("ak_live_") || key.len() != 51 {
                return Err(Error::Unauthorized);
            }
            let record = self
                .store
                .find_api_key(&hash(key), now())
                .await?
                .ok_or(Error::Unauthorized)?;
            (record.user_id, AuthType::ApiKey, record.scopes, None, None)
        } else if let Some(token) = session {
            if token.len() != 43 {
                return Err(Error::Unauthorized);
            }
            let record = self
                .store
                .find_session(&hash(token), now())
                .await?
                .ok_or(Error::Unauthorized)?;
            (
                record.user_id,
                AuthType::Session,
                self.config.session.scopes.clone(),
                Some(record.id),
                Some(Secret::new(csrf(token))),
            )
        } else {
            return Ok(None);
        };
        let user = self
            .store
            .get_user(&user_id)
            .await?
            .ok_or(Error::Unauthorized)?;
        if user.disabled_at.is_some() {
            return Err(Error::Unauthorized);
        }
        let mut claims = BTreeMap::new();
        claims.insert("stargate".into(), json!({"role":user.role}));
        Ok(Some(Authentication {
            identity: Identity {
                subject: user.id.clone(),
                user_id: Some(user.id),
                email: user.email,
                auth_type,
                scopes,
                claims,
            },
            session_id,
            csrf_token,
        }))
    }
    pub async fn create_session(&self, user_id: &str) -> Result<(Session, Secret)> {
        let (record, token) = self.session_record(user_id);
        self.store.create_session(&record).await?;
        self.audit(
            "auth.session.created",
            Some(user_id),
            json!({"session_id":record.id}),
        )
        .await?;
        Ok((record, token))
    }
    fn session_record(&self, user_id: &str) -> (Session, Secret) {
        let token = Secret::new(random_token());
        let record = Session {
            id: random_token(),
            user_id: user_id.into(),
            token_hash: hash(token.expose()),
            created_at: now(),
            expires_at: now() + i64::from(self.config.session.ttl_seconds),
            revoked_at: None,
            last_used_at: None,
        };
        (record, token)
    }
    pub async fn create_api_key(
        &self,
        identity: &Identity,
        name: String,
        scopes: Vec<String>,
        expires_at: Option<i64>,
    ) -> Result<(ApiKey, Secret)> {
        if identity.auth_type != AuthType::Session {
            return Err(Error::Forbidden);
        }
        if name.trim().is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
            || expires_at.is_some_and(|e| e <= now())
        {
            return Err(Error::BadRequest);
        }
        validate_scopes(&scopes)?;
        authorize(
            Some(identity),
            &Policy::Scopes {
                scopes: scopes.clone(),
            },
        )?;
        let owner = identity.user_id.as_deref().ok_or(Error::Forbidden)?;
        let secret = Secret::new(format!("ak_live_{}", random_token()));
        let record = ApiKey {
            id: random_token(),
            user_id: owner.into(),
            name,
            prefix: secret.expose()[..16].into(),
            secret_hash: hash(secret.expose()),
            scopes,
            created_at: now(),
            expires_at,
            revoked_at: None,
            last_used_at: None,
        };
        self.store.create_api_key(&record).await?;
        self.audit(
            "auth.api_key.created",
            Some(owner),
            json!({"key_id":record.id}),
        )
        .await?;
        Ok((record, secret))
    }
    pub async fn revoke_session(&self, user: &str, id: Option<&str>) -> Result<()> {
        self.store.revoke_session(user, id, now()).await?;
        self.audit("auth.session.revoked", Some(user), json!({"session_id":id}))
            .await
    }
    pub async fn revoke_api_key(&self, user: &str, id: &str) -> Result<()> {
        self.store.revoke_api_key(user, id, now()).await?;
        self.audit("auth.api_key.revoked", Some(user), json!({"key_id":id}))
            .await
    }
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn hash(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(value.as_bytes()))
}
pub fn csrf(token: &str) -> String {
    hash(&format!("stargate-csrf:{token}"))
}

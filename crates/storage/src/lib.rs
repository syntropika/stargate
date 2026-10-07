//! Persistence boundary. Implementations must make identity resolution and transaction consumption atomic.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Result<T> = std::result::Result<T, StoreError>;
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage is unavailable")]
    Unavailable,
    #[error("storage conflict retry limit exceeded")]
    Conflict,
    #[error("storage data is invalid")]
    InvalidData,
    #[error("unsupported schema version")]
    SchemaVersion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub created_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub metadata: Value,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub last_used_at: Option<i64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub prefix: String,
    pub secret_hash: String,
    pub scopes: Vec<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub last_used_at: Option<i64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct OidcTransaction {
    pub state_hash: String,
    pub browser_hash: String,
    pub provider: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub return_to: String,
    pub expires_at: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub event: String,
    pub actor: Option<String>,
    pub metadata: Value,
    pub created_at: i64,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    /// Resolve by issuer + subject, creating a user only when that pair is new. Never link by email.
    async fn resolve_identity(&self, identity: &ExternalIdentity, new_user: &User) -> Result<User>;
    async fn get_user(&self, id: &str) -> Result<Option<User>>;
    async fn create_session(&self, session: &Session) -> Result<()>;
    /// Return only active sessions and atomically update last_used_at.
    async fn find_session(&self, token_hash: &str, now: i64) -> Result<Option<Session>>;
    async fn list_sessions(&self, user_id: &str) -> Result<Vec<Session>>;
    async fn revoke_session(&self, user_id: &str, id: Option<&str>, now: i64) -> Result<()>;
    async fn create_api_key(&self, key: &ApiKey) -> Result<()>;
    async fn find_api_key(&self, secret_hash: &str, now: i64) -> Result<Option<ApiKey>>;
    async fn list_api_keys(&self, user_id: &str) -> Result<Vec<ApiKey>>;
    async fn revoke_api_key(&self, user_id: &str, id: &str, now: i64) -> Result<()>;
    async fn create_oidc_transaction(&self, transaction: &OidcTransaction) -> Result<()>;
    /// Atomically delete and return once, only with the matching browser binding and before expiry.
    async fn consume_oidc_transaction(
        &self,
        state_hash: &str,
        browser_hash: &str,
        now: i64,
    ) -> Result<Option<OidcTransaction>>;
    async fn write_audit_event(&self, event: &AuditEvent) -> Result<()>;
}

/// Storage engine adapters, enabled through Cargo features.
pub mod adapters;

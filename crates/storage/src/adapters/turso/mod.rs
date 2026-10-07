//! Embedded Turso adapter. Every operation uses an independent connection and a bounded MVCC retry.
use crate::*;
use ::turso::{Database, Value};
use async_trait::async_trait;
use rand::Rng;
use serde::de::DeserializeOwned;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub struct TursoStore {
    database: Database,
    permits: Semaphore,
    retry_limit: u32,
    observer: Option<Arc<dyn Fn(AuditEvent) + Send + Sync>>,
}
fn text(v: impl Into<String>) -> Value {
    Value::Text(v.into())
}
fn opt(v: Option<i64>) -> Value {
    v.map(Value::Integer).unwrap_or(Value::Null)
}
fn encode<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| StoreError::InvalidData)
}
fn decode<T: DeserializeOwned>(v: String) -> Result<T> {
    serde_json::from_str(&v).map_err(|_| StoreError::InvalidData)
}
fn retryable(e: &turso::Error) -> bool {
    // Turso 0.8 exposes these MVCC errors as generic Error values. Match only documented conflict messages.
    matches!(e, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
        || matches!(e,turso::Error::Error(message) if matches!(message.as_str(),"Write-write conflict" | "Database schema conflict" | "Database schema changed"))
}
fn error(e: turso::Error) -> StoreError {
    if retryable(&e) {
        StoreError::Conflict
    } else {
        StoreError::Unavailable
    }
}

impl TursoStore {
    pub async fn open(
        path: &str,
        connections: usize,
        retry_limit: u32,
        observer: Option<Arc<dyn Fn(AuditEvent) + Send + Sync>>,
    ) -> Result<Self> {
        if path.is_empty() || connections == 0 || connections > 128 || retry_limit > 64 {
            return Err(StoreError::InvalidData);
        }
        let database = turso::Builder::new_local(path)
            .build()
            .await
            .map_err(error)?;
        let conn = database.connect().map_err(error)?;
        conn.pragma_update("journal_mode", "'mvcc'")
            .await
            .map_err(error)?;
        conn.execute("PRAGMA foreign_keys = ON", ())
            .await
            .map_err(error)?;
        conn.execute("BEGIN EXCLUSIVE", ()).await.map_err(error)?;
        let migration = async {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY)",
                (),
            )
            .await?;
            let mut rows = conn
                .query("SELECT COALESCE(MAX(version),0) FROM schema_migrations", ())
                .await?;
            let version = rows
                .next()
                .await?
                .ok_or(turso::Error::QueryReturnedNoRows)?
                .get::<i64>(0)?;
            drop(rows);
            if version > 1 {
                return Ok(false);
            }
            if version == 0 {
                conn.execute_batch(include_str!("migrations/0001.sql"))
                    .await?;
                conn.execute("INSERT INTO schema_migrations VALUES (1)", ())
                    .await?;
            }
            Ok::<_, turso::Error>(true)
        }
        .await;
        match migration {
            Ok(true) => {
                conn.execute("COMMIT", ()).await.map_err(error)?;
            }
            Ok(false) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                return Err(StoreError::SchemaVersion);
            }
            Err(e) => {
                let _ = conn.execute("ROLLBACK", ()).await;
                return Err(error(e));
            }
        }
        Ok(Self {
            database,
            permits: Semaphore::new(connections),
            retry_limit,
            observer,
        })
    }

    async fn transaction(&self, statements: Vec<(String, Vec<Value>)>) -> Result<Vec<String>> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| StoreError::Unavailable)?;
        for attempt in 0..=self.retry_limit {
            let conn = self.database.connect().map_err(error)?;
            conn.execute("PRAGMA foreign_keys = ON", ())
                .await
                .map_err(error)?;
            let result = async {
                conn.execute("BEGIN CONCURRENT", ()).await?;
                let mut out = Vec::new();
                for (sql, values) in &statements {
                    let mut rows = conn.query(sql, values.clone()).await?;
                    while let Some(row) = rows.next().await? {
                        out.push(row.get::<String>(0)?);
                    }
                }
                conn.execute("COMMIT", ()).await?;
                Ok::<_, turso::Error>(out)
            }
            .await;
            match result {
                Ok(out) => return Ok(out),
                Err(e) => {
                    let _ = conn.execute("ROLLBACK", ()).await;
                    if !retryable(&e) {
                        return Err(error(e));
                    }
                    if attempt == self.retry_limit {
                        return Err(StoreError::Conflict);
                    }
                    if let Some(observer) = &self.observer {
                        observer(AuditEvent {
                            id: format!("retry-{}", rand::random::<u64>()),
                            event: "storage.retry".into(),
                            actor: None,
                            metadata: serde_json::json!({"attempt":attempt+1}),
                            created_at: now(),
                        });
                    }
                    let jitter = rand::thread_rng().gen_range(0..8);
                    tokio::time::sleep(Duration::from_millis((1u64 << attempt.min(7)) + jitter))
                        .await;
                }
            }
        }
        Err(StoreError::Conflict)
    }
    async fn query<T: DeserializeOwned>(&self, sql: &str, values: Vec<Value>) -> Result<Vec<T>> {
        self.transaction(vec![(sql.into(), values)])
            .await?
            .into_iter()
            .map(decode)
            .collect()
    }
    async fn execute(&self, sql: &str, values: Vec<Value>) -> Result<()> {
        self.transaction(vec![(sql.into(), values)]).await?;
        Ok(())
    }
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[async_trait]
impl AuthStore for TursoStore {
    async fn resolve_identity(&self, identity: &ExternalIdentity, user: &User) -> Result<User> {
        let rows = self.transaction(vec![
            ("INSERT INTO users(id,email,created_at,data) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS(SELECT 1 FROM identities WHERE issuer=?5 AND subject=?6)".into(), vec![text(&user.id), identity.email.as_ref().map(text).unwrap_or(Value::Null),Value::Integer(user.created_at),text(encode(user)?),text(&identity.issuer),text(&identity.subject)]),
            ("INSERT INTO identities(id,user_id,issuer,subject,email,metadata,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(issuer,subject) DO UPDATE SET email=excluded.email,metadata=excluded.metadata".into(),vec![text(&user.id),text(&user.id),text(&identity.issuer),text(&identity.subject),identity.email.as_ref().map(text).unwrap_or(Value::Null),text(encode(&identity.metadata)?),Value::Integer(user.created_at)]),
            ("UPDATE users SET email=?1,data=json_set(data,'$.email',?1) WHERE id=(SELECT user_id FROM identities WHERE issuer=?2 AND subject=?3)".into(),vec![identity.email.as_ref().map(text).unwrap_or(Value::Null),text(&identity.issuer),text(&identity.subject)]),
            ("SELECT users.data FROM users JOIN identities ON identities.user_id=users.id WHERE issuer=?1 AND subject=?2".into(),vec![text(&identity.issuer),text(&identity.subject)]),
        ]).await?;
        rows.into_iter()
            .next()
            .ok_or(StoreError::InvalidData)
            .and_then(decode)
    }
    async fn get_user(&self, id: &str) -> Result<Option<User>> {
        Ok(self
            .query("SELECT data FROM users WHERE id=?1", vec![text(id)])
            .await?
            .pop())
    }
    async fn create_session(&self, s: &Session) -> Result<()> {
        self.execute(
            "INSERT INTO sessions(id,user_id,token_hash,expires_at,data) VALUES(?1,?2,?3,?4,?5)",
            vec![
                text(&s.id),
                text(&s.user_id),
                text(&s.token_hash),
                Value::Integer(s.expires_at),
                text(encode(s)?),
            ],
        )
        .await
    }
    async fn find_session(&self, hash: &str, now: i64) -> Result<Option<Session>> {
        Ok(self.query("UPDATE sessions SET last_used_at=?2,data=json_set(data,'$.last_used_at',?2) WHERE token_hash=?1 AND expires_at>?2 AND revoked_at IS NULL RETURNING data",vec![text(hash),Value::Integer(now)]).await?.pop())
    }
    async fn list_sessions(&self, user: &str) -> Result<Vec<Session>> {
        self.query(
            "SELECT data FROM sessions WHERE user_id=?1 ORDER BY id",
            vec![text(user)],
        )
        .await
    }
    async fn revoke_session(&self, user: &str, id: Option<&str>, now: i64) -> Result<()> {
        self.execute("UPDATE sessions SET revoked_at=?3,data=json_set(data,'$.revoked_at',?3) WHERE user_id=?1 AND (?2 IS NULL OR id=?2) AND revoked_at IS NULL",vec![text(user),id.map(text).unwrap_or(Value::Null),Value::Integer(now)]).await
    }
    async fn create_api_key(&self, k: &ApiKey) -> Result<()> {
        self.execute(
            "INSERT INTO api_keys(id,user_id,secret_hash,expires_at,data) VALUES(?1,?2,?3,?4,?5)",
            vec![
                text(&k.id),
                text(&k.user_id),
                text(&k.secret_hash),
                opt(k.expires_at),
                text(encode(k)?),
            ],
        )
        .await
    }
    async fn find_api_key(&self, hash: &str, now: i64) -> Result<Option<ApiKey>> {
        Ok(self.query("UPDATE api_keys SET last_used_at=?2,data=json_set(data,'$.last_used_at',?2) WHERE secret_hash=?1 AND (expires_at IS NULL OR expires_at>?2) AND revoked_at IS NULL RETURNING data",vec![text(hash),Value::Integer(now)]).await?.pop())
    }
    async fn list_api_keys(&self, user: &str) -> Result<Vec<ApiKey>> {
        self.query(
            "SELECT data FROM api_keys WHERE user_id=?1 ORDER BY id",
            vec![text(user)],
        )
        .await
    }
    async fn revoke_api_key(&self, user: &str, id: &str, now: i64) -> Result<()> {
        self.execute("UPDATE api_keys SET revoked_at=?3,data=json_set(data,'$.revoked_at',?3) WHERE user_id=?1 AND id=?2 AND revoked_at IS NULL",vec![text(user),text(id),Value::Integer(now)]).await
    }
    async fn create_oidc_transaction(&self, t: &OidcTransaction) -> Result<()> {
        self.transaction(vec![("DELETE FROM oidc_transactions WHERE expires_at<=?1".into(),vec![Value::Integer(now())]),("INSERT INTO oidc_transactions(state_hash,browser_hash,expires_at,data) VALUES(?1,?2,?3,?4)".into(),vec![text(&t.state_hash),text(&t.browser_hash),Value::Integer(t.expires_at),text(encode(t)?)])]).await?;
        Ok(())
    }
    async fn consume_oidc_transaction(
        &self,
        state: &str,
        browser: &str,
        now: i64,
    ) -> Result<Option<OidcTransaction>> {
        Ok(self.query("DELETE FROM oidc_transactions WHERE state_hash=?1 AND browser_hash=?2 AND expires_at>?3 RETURNING data",vec![text(state),text(browser),Value::Integer(now)]).await?.pop())
    }
    async fn write_audit_event(&self, e: &AuditEvent) -> Result<()> {
        self.execute(
            "INSERT INTO audit_events(id,event,actor,created_at,data) VALUES(?1,?2,?3,?4,?5)",
            vec![
                text(&e.id),
                text(&e.event),
                e.actor.as_ref().map(text).unwrap_or(Value::Null),
                Value::Integer(e.created_at),
                text(encode(e)?),
            ],
        )
        .await
    }
}

use super::*;

// A single persisted row serializes bootstrap, role changes and credential writes under MVCC.
pub(super) fn account_lock() -> (String, Vec<Value>) {
    (
        "UPDATE account_state SET serial=serial+1 WHERE id=1".into(),
        vec![],
    )
}
fn active_admin() -> &'static str {
    "EXISTS(SELECT 1 FROM users WHERE id=?1 AND role='administrator' AND disabled_at IS NULL)"
}
impl TursoStore {
    pub(super) async fn prepare_accounts(&self, admin: Option<&str>) -> Result<()> {
        let mut statements = vec![account_lock()];
        if let Some(admin) = admin {
            statements.push(("UPDATE users SET role='administrator',data=json_set(data,'$.role','administrator') WHERE id=?1 AND disabled_at IS NULL AND (SELECT initialized FROM account_state WHERE id=1)=0".into(), vec![text(admin)]));
            statements.push(("UPDATE account_state SET initialized=1 WHERE id=1 AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND role='administrator' AND disabled_at IS NULL)".into(),vec![text(admin)]));
        }
        statements.push(("SELECT 'ready' FROM account_state WHERE initialized=1 OR NOT EXISTS(SELECT 1 FROM users)".into(), vec![]));
        if self.transaction(statements).await?.is_empty() {
            return Err(StoreError::InvalidData);
        }
        Ok(())
    }
    pub(super) async fn setup_available(&self) -> Result<bool> {
        Ok(!self.transaction(vec![("SELECT 'ready' FROM account_state WHERE initialized=0 AND NOT EXISTS(SELECT 1 FROM users)".into(),vec![])]).await?.is_empty())
    }
    pub(super) async fn credential(
        &self,
        field: &str,
        value: &str,
    ) -> Result<Option<LocalCredential>> {
        Ok(self.query(&format!("SELECT json_object('user_id',user_id,'email',email,'password_hash',password_hash) FROM local_credentials WHERE {field}=?1"),vec![text(value)]).await?.pop())
    }
    pub(super) async fn insert_local_user(
        &self,
        user: &User,
        credential: &LocalCredential,
        session: Option<&Session>,
        actor: Option<&str>,
    ) -> Result<Option<User>> {
        if credential.user_id != user.id
            || user.disabled_at.is_some()
            || (actor.is_none() && user.role != UserRole::Administrator)
        {
            return Err(StoreError::InvalidData);
        }
        let permission = if actor.is_some() {
            "EXISTS(SELECT 1 FROM users WHERE id=?7 AND role='administrator' AND disabled_at IS NULL)"
        } else {
            "?7 IS NULL AND (SELECT initialized FROM account_state WHERE id=1)=0 AND NOT EXISTS(SELECT 1 FROM users)"
        };
        let mut statements = vec![account_lock(), (
            format!("INSERT INTO users(id,email,created_at,data,role) SELECT ?1,?2,?3,?4,?5 WHERE {permission} AND NOT EXISTS(SELECT 1 FROM local_credentials WHERE email=?6) RETURNING data"),
            vec![text(&user.id), user.email.as_ref().map(text).unwrap_or(Value::Null), Value::Integer(user.created_at),text(encode(user)?),text(user.role.as_str()),text(&credential.email),actor.map(text).unwrap_or(Value::Null)],
        ), (
            "INSERT INTO local_credentials(user_id,email,password_hash) SELECT ?1,?2,?3 WHERE EXISTS(SELECT 1 FROM users WHERE id=?1)".into(),
            vec![text(&credential.user_id),text(&credential.email),text(&credential.password_hash)],
        )];
        if actor.is_none() {
            statements.push(("UPDATE account_state SET initialized=1 WHERE id=1 AND EXISTS(SELECT 1 FROM users WHERE id=?1)".into(),vec![text(&user.id)]));
        }
        if let Some(session) = session {
            if session.user_id != user.id {
                return Err(StoreError::InvalidData);
            }
            statements.push(("INSERT INTO sessions(id,user_id,token_hash,expires_at,data) SELECT ?1,?2,?3,?4,?5 WHERE EXISTS(SELECT 1 FROM users WHERE id=?2)".into(), vec![text(&session.id),text(&session.user_id),text(&session.token_hash),Value::Integer(session.expires_at),text(encode(session)?)]));
        }
        statements.push((
            "SELECT data FROM users WHERE id=?1".into(),
            vec![text(&user.id)],
        ));
        self.transaction_guarded(statements, Some(1))
            .await?
            .unwrap_or_default()
            .pop()
            .map(decode)
            .transpose()
    }
    pub(super) async fn edit_user(
        &self,
        actor: &str,
        id: &str,
        role: UserRole,
        disabled: Option<i64>,
    ) -> Result<Option<User>> {
        let statements = vec![account_lock(), (
            format!("UPDATE users SET role=?3,disabled_at=?4,data=json_set(data,'$.role',?3,'$.disabled_at',?4) WHERE id=?2 AND {} AND NOT (role='administrator' AND disabled_at IS NULL AND (?3!='administrator' OR ?4 IS NOT NULL) AND NOT EXISTS(SELECT 1 FROM users WHERE id!=?2 AND role='administrator' AND disabled_at IS NULL)) RETURNING data",active_admin()),
            vec![text(actor),text(id),text(role.as_str()),opt(disabled)],
        ), (
            "UPDATE sessions SET revoked_at=?2,data=json_set(data,'$.revoked_at',?2) WHERE user_id=?1 AND revoked_at IS NULL AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NOT NULL)".into(), vec![text(id),opt(disabled)],
        ), (
            "UPDATE api_keys SET revoked_at=?2,data=json_set(data,'$.revoked_at',?2) WHERE user_id=?1 AND revoked_at IS NULL AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NOT NULL)".into(),vec![text(id),opt(disabled)],
        )];
        self.transaction_guarded(statements, Some(1))
            .await?
            .unwrap_or_default()
            .pop()
            .map(decode)
            .transpose()
    }
    pub(super) async fn revoke_access(&self, actor: &str, user: &str, now: i64) -> Result<bool> {
        let values = vec![text(actor), text(user), Value::Integer(now)];
        let mut statements = vec![account_lock()];
        for table in ["sessions", "api_keys"] {
            statements.push((format!("UPDATE {table} SET revoked_at=?3,data=json_set(data,'$.revoked_at',?3) WHERE user_id=?2 AND revoked_at IS NULL AND {}",active_admin()),values.clone()));
        }
        statements.push((
            format!("SELECT id FROM users WHERE id=?2 AND {}", active_admin()),
            vec![text(actor), text(user)],
        ));
        Ok(!self.transaction(statements).await?.is_empty())
    }
    pub(super) async fn replace_password(
        &self,
        user: &str,
        expected: &str,
        replacement: &str,
        session: &Session,
        now: i64,
    ) -> Result<bool> {
        if session.user_id != user {
            return Err(StoreError::InvalidData);
        }
        let rows = self.transaction_guarded(vec![account_lock(), (
            "UPDATE local_credentials SET password_hash=?3 WHERE user_id=?1 AND password_hash=?2 AND EXISTS(SELECT 1 FROM users WHERE id=?1 AND disabled_at IS NULL) RETURNING user_id".into(),vec![text(user),text(expected),text(replacement)],
        ), (
            "UPDATE sessions SET revoked_at=?3,data=json_set(data,'$.revoked_at',?3) WHERE user_id=?1 AND revoked_at IS NULL AND EXISTS(SELECT 1 FROM local_credentials WHERE user_id=?1 AND password_hash=?2)".into(),vec![text(user),text(replacement),Value::Integer(now)],
        ), (
            "INSERT INTO sessions(id,user_id,token_hash,expires_at,data) SELECT ?1,?2,?3,?4,?5 WHERE EXISTS(SELECT 1 FROM local_credentials WHERE user_id=?2 AND password_hash=?6)".into(), vec![text(&session.id),text(user),text(&session.token_hash),Value::Integer(session.expires_at),text(encode(session)?),text(replacement)],
        )], Some(1)).await?;
        Ok(rows.is_some())
    }
}

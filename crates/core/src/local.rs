use super::*;
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use std::{collections::HashMap, sync::Mutex};
use tokio::sync::Semaphore;

pub(super) struct LocalRuntime {
    work: Arc<Semaphore>,
    attempts: Mutex<HashMap<String, (i64, u32)>>,
}
impl Default for LocalRuntime {
    fn default() -> Self {
        Self {
            work: Arc::new(Semaphore::new(4)),
            attempts: Mutex::new(HashMap::new()),
        }
    }
}
fn password_engine() -> Argon2<'static> {
    Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(19 * 1024, 2, 1, None).expect("fixed Argon2 parameters"),
    )
}
fn email(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    let Some((name, domain)) = value.split_once('@') else {
        return Err(Error::BadRequest);
    };
    if value.len() > 254
        || name.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || !value.is_ascii()
        || value.bytes().any(|b| b <= 32 || b >= 127)
    {
        return Err(Error::BadRequest);
    }
    Ok(value)
}
fn validate_password(password: &Secret) -> Result<()> {
    if password.expose().chars().count() < 12 || password.expose().len() > 1024 {
        return Err(Error::BadRequest);
    }
    Ok(())
}
impl Auth {
    fn local_config(&self) -> Result<&LocalConfig> {
        self.config.local.as_ref().ok_or(Error::Forbidden)
    }
    fn reserve_attempt(&self, email: &str, peer: &str) -> Result<()> {
        let config = self.local_config()?;
        let now = now();
        let mut attempts = self
            .local
            .attempts
            .lock()
            .map_err(|_| Error::TooManyRequests)?;
        attempts.retain(|_, (expires, _)| *expires > now);
        let keys = [
            (format!("email:{}", hash(email)), config.login_attempts),
            (format!("peer:{}", hash(peer)), config.login_attempts * 10),
        ];
        for (key, limit) in &keys {
            if attempts.get(key).is_some_and(|(_, count)| count >= limit)
                || (!attempts.contains_key(key) && attempts.len() >= 4096)
            {
                return Err(Error::TooManyRequests);
            }
        }
        for (key, _) in keys {
            let record = attempts
                .entry(key)
                .or_insert((now + i64::from(config.login_window_seconds), 0));
            record.1 += 1;
        }
        Ok(())
    }
    async fn hash_password(&self, password: Secret) -> Result<String> {
        validate_password(&password)?;
        let permit = self
            .local
            .work
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::TooManyRequests)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let salt = SaltString::generate(&mut OsRng);
            password_engine()
                .hash_password(password.expose().as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .map_err(|_| Error::BadRequest)
        })
        .await
        .map_err(|_| Error::BadRequest)?
    }
    async fn verify_password(&self, password: Secret, stored: Option<String>) -> Result<bool> {
        if password.expose().len() > 1024 {
            return Err(Error::Unauthorized);
        }
        let permit = self
            .local
            .work
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::TooManyRequests)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            match stored {
                Some(hash) => PasswordHash::new(&hash).is_ok_and(|hash| {
                    password_engine()
                        .verify_password(password.expose().as_bytes(), &hash)
                        .is_ok()
                }),
                None => {
                    let salt = SaltString::encode_b64(b"stargate-unknown-account")
                        .expect("fixed dummy salt");
                    let _ = password_engine().hash_password(password.expose().as_bytes(), &salt);
                    false
                }
            }
        })
        .await
        .map_err(|_| Error::Unauthorized)
    }
    pub async fn local_setup_available(&self) -> Result<bool> {
        if self.config.local.is_none() {
            return Ok(false);
        }
        Ok(self.store.local_setup_available().await?)
    }
    pub async fn setup_local(
        &self,
        raw_email: &str,
        password: Secret,
        peer: &str,
    ) -> Result<Secret> {
        self.local_config()?;
        if !self.store.local_setup_available().await? {
            return Err(Error::Conflict);
        }
        let email = email(raw_email)?;
        self.reserve_attempt(&email, peer)?;
        let password_hash = self.hash_password(password).await?;
        let user = User {
            id: random_token(),
            email: Some(email.clone()),
            created_at: now(),
            role: UserRole::Administrator,
            disabled_at: None,
        };
        let credential = LocalCredential {
            user_id: user.id.clone(),
            email,
            password_hash,
        };
        let (session, token) = self.session_record(&user.id);
        self.store
            .create_local_user(&user, &credential, Some(&session), None)
            .await?
            .ok_or(Error::Conflict)?;
        self.audit("auth.local.setup", Some(&user.id), json!({}))
            .await?;
        Ok(token)
    }
    pub async fn login_local(
        &self,
        raw_email: &str,
        password: Secret,
        peer: &str,
    ) -> Result<Secret> {
        self.local_config()?;
        let email = email(raw_email)?;
        self.reserve_attempt(&email, peer)?;
        let credential = self.store.find_local_credential(&email).await?;
        let verified = self
            .verify_password(
                password,
                credential.as_ref().map(|c| c.password_hash.clone()),
            )
            .await?;
        let Some(credential) = credential.filter(|_| verified) else {
            self.audit("auth.local.login.denied", None, json!({}))
                .await?;
            return Err(Error::Unauthorized);
        };
        let (session, token) = self.session_record(&credential.user_id);
        if !self
            .store
            .create_local_session(&credential.password_hash, &session)
            .await?
        {
            return Err(Error::Unauthorized);
        }
        self.audit("auth.local.login", Some(&credential.user_id), json!({}))
            .await?;
        Ok(token)
    }
    pub async fn require_administrator(&self, identity: &Identity) -> Result<User> {
        self.local_config()?;
        if identity.auth_type != AuthType::Session {
            return Err(Error::Forbidden);
        }
        let user = self
            .store
            .get_user(identity.user_id.as_deref().ok_or(Error::Forbidden)?)
            .await?
            .ok_or(Error::Forbidden)?;
        if user.role != UserRole::Administrator || user.disabled_at.is_some() {
            return Err(Error::Forbidden);
        }
        Ok(user)
    }
    pub async fn create_local_user(
        &self,
        identity: &Identity,
        raw_email: &str,
        password: Secret,
        role: UserRole,
    ) -> Result<User> {
        let actor = self.require_administrator(identity).await?;
        let email = email(raw_email)?;
        let password_hash = self.hash_password(password).await?;
        let user = User {
            id: random_token(),
            email: Some(email.clone()),
            created_at: now(),
            role,
            disabled_at: None,
        };
        let credential = LocalCredential {
            user_id: user.id.clone(),
            email,
            password_hash,
        };
        let user = self
            .store
            .create_local_user(&user, &credential, None, Some(&actor.id))
            .await?
            .ok_or(Error::Conflict)?;
        self.audit(
            "auth.user.created",
            Some(&actor.id),
            json!({"user_id":user.id,"role":user.role}),
        )
        .await?;
        Ok(user)
    }
    pub async fn update_user(
        &self,
        identity: &Identity,
        id: &str,
        role: UserRole,
        disabled: bool,
    ) -> Result<User> {
        let actor = self.require_administrator(identity).await?;
        let user = self
            .store
            .update_user(&actor.id, id, role, disabled.then(now))
            .await?
            .ok_or(Error::Conflict)?;
        self.audit(
            "auth.user.updated",
            Some(&actor.id),
            json!({"user_id":id,"role":role,"disabled":disabled}),
        )
        .await?;
        Ok(user)
    }
    pub async fn revoke_user_access(&self, identity: &Identity, id: &str) -> Result<()> {
        let actor = self.require_administrator(identity).await?;
        if !self.store.revoke_user_access(&actor.id, id, now()).await? {
            return Err(Error::Conflict);
        }
        self.audit(
            "auth.user.access.revoked",
            Some(&actor.id),
            json!({"user_id":id}),
        )
        .await
    }
    pub async fn change_password(
        &self,
        identity: &Identity,
        current: Secret,
        replacement: Secret,
        peer: &str,
    ) -> Result<Secret> {
        self.local_config()?;
        if identity.auth_type != AuthType::Session {
            return Err(Error::Forbidden);
        }
        let id = identity.user_id.as_deref().ok_or(Error::Forbidden)?;
        self.reserve_attempt(id, peer)?;
        let credential = self.store.get_local_credential(id).await?;
        if !self
            .verify_password(
                current,
                credential.as_ref().map(|c| c.password_hash.clone()),
            )
            .await?
        {
            return Err(Error::Unauthorized);
        }
        let credential = credential.ok_or(Error::Forbidden)?;
        let hash = self.hash_password(replacement).await?;
        let (session, token) = self.session_record(id);
        if !self
            .store
            .change_local_password(id, &credential.password_hash, &hash, &session, now())
            .await?
        {
            return Err(Error::Conflict);
        }
        self.audit("auth.local.password.changed", Some(id), json!({}))
            .await?;
        Ok(token)
    }
}

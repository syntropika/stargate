use crate::*;
use openidconnect::{
    AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret, CsrfToken, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreClient, CoreProviderMetadata, CoreResponseType},
    reqwest,
};
use serde_json::json;
use storage::{ExternalIdentity, OidcTransaction, User};

pub(super) struct Provider {
    config: OidcProviderConfig,
    metadata: CoreProviderMetadata,
}
fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| Error::Oidc)
}
pub(super) async fn discover(config: &AuthConfig) -> Result<Vec<Provider>> {
    let client = http_client()?;
    let mut providers = Vec::new();
    for p in &config.oidc {
        let metadata = CoreProviderMetadata::discover_async(p.issuer.clone(), &client)
            .await
            .map_err(|_| Error::Oidc)?;
        config.validate_url(metadata.authorization_endpoint().url())?;
        config.validate_url(metadata.jwks_uri().url())?;
        config.validate_url(metadata.token_endpoint().ok_or(Error::Oidc)?.url())?;
        providers.push(Provider {
            config: p.clone(),
            metadata,
        });
    }
    Ok(providers)
}
impl Auth {
    pub async fn login(&self, name: Option<&str>, return_to: &str) -> Result<(String, Secret)> {
        self.config.validate_return_to(return_to)?;
        let provider = if let Some(name) = name {
            self.providers.iter().find(|p| p.config.name == name)
        } else if self.providers.len() == 1 {
            self.providers.first()
        } else {
            None
        }
        .ok_or(Error::BadRequest)?;
        let client = CoreClient::from_provider_metadata(
            provider.metadata.clone(),
            ClientId::new(provider.config.client_id.clone()),
            Some(ClientSecret::new(
                provider.config.client_secret.expose().into(),
            )),
        )
        .set_redirect_uri(RedirectUrl::new(self.config.callback_url()).map_err(|_| Error::Config)?);
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = client
            .authorize_url(
                AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("email".into()))
            .set_pkce_challenge(challenge)
            .url();
        let browser = Secret::new(random_token());
        self.store
            .create_oidc_transaction(&OidcTransaction {
                state_hash: hash(state.secret()),
                browser_hash: hash(browser.expose()),
                provider: provider.config.name.clone(),
                nonce: nonce.secret().clone(),
                pkce_verifier: verifier.secret().clone(),
                return_to: return_to.into(),
                expires_at: now() + 600,
            })
            .await?;
        Ok((url.into(), browser))
    }
    pub async fn callback(
        &self,
        state: &str,
        code: &str,
        browser: &str,
    ) -> Result<(Secret, String)> {
        let result = self.finish_callback(state, code, browser).await;
        if result.is_err() {
            self.audit("auth.login.failed", None, json!({})).await?;
        }
        result
    }
    async fn finish_callback(
        &self,
        state: &str,
        code: &str,
        browser: &str,
    ) -> Result<(Secret, String)> {
        if state.is_empty() || browser.len() != 43 {
            return Err(Error::Unauthorized);
        }
        let transaction = self
            .store
            .consume_oidc_transaction(&hash(state), &hash(browser), now())
            .await?
            .ok_or(Error::Unauthorized)?;
        if code.is_empty() {
            return Err(Error::Unauthorized);
        }
        let provider = self
            .providers
            .iter()
            .find(|p| p.config.name == transaction.provider)
            .ok_or(Error::Oidc)?;
        // Rediscover on callback so key rotation is honored; the issuer and endpoint schemes are revalidated.
        let metadata =
            CoreProviderMetadata::discover_async(provider.config.issuer.clone(), &http_client()?)
                .await
                .map_err(|_| Error::Oidc)?;
        self.config
            .validate_url(metadata.authorization_endpoint().url())?;
        self.config.validate_url(metadata.jwks_uri().url())?;
        self.config
            .validate_url(metadata.token_endpoint().ok_or(Error::Oidc)?.url())?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(provider.config.client_id.clone()),
            Some(ClientSecret::new(
                provider.config.client_secret.expose().into(),
            )),
        )
        .set_redirect_uri(RedirectUrl::new(self.config.callback_url()).map_err(|_| Error::Config)?);
        let token = client
            .exchange_code(AuthorizationCode::new(code.into()))
            .map_err(|_| Error::Oidc)?
            .set_pkce_verifier(PkceCodeVerifier::new(transaction.pkce_verifier))
            .request_async(&http_client()?)
            .await
            .map_err(|_| Error::Oidc)?;
        let id_token = token.id_token().ok_or(Error::Oidc)?;
        let claims = id_token
            .claims(&client.id_token_verifier(), &Nonce::new(transaction.nonce))
            .map_err(|_| Error::Oidc)?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = openidconnect::AccessTokenHash::from_token(
                token.access_token(),
                id_token.signing_alg().map_err(|_| Error::Oidc)?,
                id_token
                    .signing_key(&client.id_token_verifier())
                    .map_err(|_| Error::Oidc)?,
            )
            .map_err(|_| Error::Oidc)?;
            if &actual != expected {
                return Err(Error::Oidc);
            }
        }
        let identity = ExternalIdentity {
            issuer: provider.config.issuer.to_string(),
            subject: claims.subject().to_string(),
            email: claims.email().map(|e| e.to_string()),
            metadata: json!({"email_verified":claims.email_verified()}),
        };
        let new_user = User {
            id: random_token(),
            email: identity.email.clone(),
            created_at: now(),
        };
        let user = self.store.resolve_identity(&identity, &new_user).await?;
        let (_, session) = self.create_session(&user.id).await?;
        self.audit(
            "auth.login.success",
            Some(&user.id),
            json!({"provider":transaction.provider}),
        )
        .await?;
        self.config.validate_return_to(&transaction.return_to)?;
        Ok((session, transaction.return_to))
    }
}

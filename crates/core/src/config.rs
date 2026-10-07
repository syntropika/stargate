use crate::{Error, Result};
use ipnet::IpNet;
pub use openidconnect::IssuerUrl;
use serde::Deserialize;
use std::net::IpAddr;
use url::Url;
use zeroize::Zeroizing;

#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct Secret(#[serde(deserialize_with = "deserialize_secret")] Zeroizing<String>);
fn deserialize_secret<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Zeroizing<String>, D::Error> {
    Ok(Zeroizing::new(String::deserialize(d)?))
}
impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcProviderConfig {
    pub name: String,
    pub issuer: IssuerUrl,
    pub client_id: String,
    pub client_secret: Secret,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TursoConfig {
    pub path: String,
    #[serde(default = "connections")]
    pub connections: usize,
    #[serde(default = "retries")]
    pub retry_limit: u32,
}
fn connections() -> usize {
    16
}
fn retries() -> u32 {
    32
}
impl TursoConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            connections: connections(),
            retry_limit: retries(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StorageConfig {
    Turso(TursoConfig),
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SessionConfig {
    pub ttl_seconds: u32,
    pub scopes: Vec<String>,
}
impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            ttl_seconds: 86400,
            scopes: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BrandingConfig {
    pub app_name: String,
    pub logo: Option<Url>,
    pub accent: String,
    pub stylesheet: Option<Url>,
}
impl Default for BrandingConfig {
    fn default() -> Self {
        Self {
            app_name: "Stargate".into(),
            logo: None,
            accent: "#315cfd".into(),
            stylesheet: None,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthConfig {
    pub base_url: Url,
    pub storage: StorageConfig,
    #[serde(default = "prefix")]
    pub path_prefix: String,
    #[serde(default)]
    pub oidc: Vec<OidcProviderConfig>,
    #[serde(default)]
    pub session: SessionConfig,
    #[serde(default)]
    pub branding: BrandingConfig,
    #[serde(default = "body_limit")]
    pub max_body_bytes: usize,
    #[serde(default = "header_limit")]
    pub max_header_bytes: usize,
    #[serde(default)]
    pub trusted_proxies: Vec<IpNet>,
    /// Explicit test/development opt-in. Cookies remain Secure even with this option.
    #[serde(default)]
    pub allow_insecure_loopback: bool,
}
fn prefix() -> String {
    "/auth".into()
}
fn body_limit() -> usize {
    65536
}
fn header_limit() -> usize {
    16384
}
impl AuthConfig {
    pub fn new(base_url: Url, storage: StorageConfig) -> Self {
        Self {
            base_url,
            storage,
            path_prefix: prefix(),
            oidc: Vec::new(),
            session: SessionConfig::default(),
            branding: BrandingConfig::default(),
            max_body_bytes: body_limit(),
            max_header_bytes: header_limit(),
            trusted_proxies: Vec::new(),
            allow_insecure_loopback: false,
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.validate_url(&self.base_url)?;
        if self.base_url.path() != "/"
            || self.base_url.query().is_some()
            || self.base_url.fragment().is_some()
        {
            return Err(Error::Config);
        }
        if !self.path_prefix.starts_with('/')
            || self.path_prefix == "/"
            || self.path_prefix.ends_with('/')
            || self.path_prefix.split('/').skip(1).any(|p| {
                p.is_empty()
                    || !p
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
        {
            return Err(Error::Config);
        }
        let StorageConfig::Turso(t) = &self.storage;
        if t.path.trim().is_empty()
            || t.connections == 0
            || t.connections > 128
            || t.retry_limit > 64
            || self.session.ttl_seconds == 0
            || self.session.ttl_seconds > 31536000
            || self.max_body_bytes == 0
            || self.max_body_bytes > 16 * 1024 * 1024
            || self.max_header_bytes == 0
            || self.max_header_bytes > 1024 * 1024
        {
            return Err(Error::Config);
        }
        if self.branding.app_name.is_empty()
            || self.branding.app_name.len() > 128
            || self.branding.app_name.chars().any(char::is_control)
            || self.branding.accent.len() != 7
            || !self.branding.accent.starts_with('#')
            || !self.branding.accent[1..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::Config);
        }
        if let Some(logo) = &self.branding.logo {
            self.validate_url(logo)?;
        }
        if let Some(stylesheet) = &self.branding.stylesheet {
            self.validate_url(stylesheet)?;
            if stylesheet.origin() != self.base_url.origin() {
                return Err(Error::Config);
            }
        }
        validate_scopes(&self.session.scopes)?;
        let mut names = std::collections::HashSet::new();
        for provider in &self.oidc {
            if provider.name.is_empty()
                || provider.name.len() > 64
                || !provider
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !names.insert(&provider.name)
                || provider.client_id.trim().is_empty()
                || provider.client_secret.expose().is_empty()
            {
                return Err(Error::Config);
            }
            self.validate_url(provider.issuer.url())?;
            if provider.issuer.url().query().is_some() || provider.issuer.url().fragment().is_some()
            {
                return Err(Error::Config);
            }
        }
        Ok(())
    }
    pub fn validate_url(&self, url: &Url) -> Result<()> {
        let loopback = url.host_str().is_some_and(|h| {
            h == "localhost"
                || h.trim_matches(['[', ']'])
                    .parse::<IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || (url.scheme() != "https"
                && !(self.allow_insecure_loopback && url.scheme() == "http" && loopback))
        {
            return Err(Error::Config);
        }
        Ok(())
    }
    pub fn callback_url(&self) -> String {
        format!(
            "{}{}{}",
            self.base_url.as_str().trim_end_matches('/'),
            self.path_prefix,
            "/callback"
        )
    }
    pub fn validate_return_to(&self, target: &str) -> Result<()> {
        if !target.starts_with('/')
            || target.starts_with("//")
            || target.len() > 2048
            || target
                .bytes()
                .any(|b| b <= 32 || b >= 127 || matches!(b, b'\\' | b'%' | b'#'))
        {
            return Err(Error::BadRequest);
        }
        let url = self.base_url.join(target).map_err(|_| Error::BadRequest)?;
        if url.origin() != self.base_url.origin() {
            return Err(Error::BadRequest);
        }
        Ok(())
    }
}
pub fn validate_scopes(scopes: &[String]) -> Result<()> {
    if scopes.len() > 64
        || scopes.iter().any(|s| {
            s.is_empty()
                || s.len() > 128
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b":._-".contains(&b))
        })
    {
        return Err(Error::BadRequest);
    }
    Ok(())
}

#[cfg(test)]
mod branding_tests {
    use super::*;

    fn config() -> AuthConfig {
        AuthConfig::new(
            "https://app.example.com".parse().unwrap(),
            StorageConfig::Turso(TursoConfig::new("test.db")),
        )
    }

    #[test]
    fn custom_stylesheet_must_share_the_host_origin() {
        let mut config = config();
        assert!(config.validate().is_ok());
        config.branding.stylesheet =
            Some("https://app.example.com/brand/theme.css".parse().unwrap());
        assert!(config.validate().is_ok());
        for url in [
            "https://other.example.com/theme.css",
            "https://app.example.com:8443/theme.css",
            "http://app.example.com/theme.css",
            "https://user:password@app.example.com/theme.css",
        ] {
            config.branding.stylesheet = Some(url.parse().unwrap());
            assert!(config.validate().is_err(), "accepted {url}");
        }
    }
}

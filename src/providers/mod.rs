mod feeds;
mod knaben;

use crate::model::Torrent;
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, RequestBuilder};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};
use url::Url;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
/// Built-in HTTP provider protocols.
pub enum Kind {
    /// Nyaa-compatible RSS search.
    Nyaa,
    /// Sukebei's Nyaa-compatible RSS search.
    Sukebei,
    /// Knaben JSON API.
    Knaben,
    /// Torznab RSS API.
    Torznab,
    /// A fixed RSS or Atom feed filtered locally.
    Rss,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
/// Configuration for one built-in HTTP provider.
pub struct ProviderConfig {
    /// Unique CLI-visible provider name.
    pub name: String,
    /// Wire protocol used by the provider.
    pub kind: Kind,
    /// Feed URL or API endpoint.
    pub url: String,
    /// Environment variable containing a Torznab API key.
    pub api_key_env: Option<String>,
    #[serde(default = "default_search")]
    /// Whether this provider participates when `--source` is omitted.
    pub default_search: bool,
}
fn default_search() -> bool {
    true
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    providers: Vec<ProviderConfig>,
}

/// Loads and validates provider configuration, or returns the built-in defaults.
pub fn configuration(path: Option<&Path>) -> Result<Vec<ProviderConfig>> {
    let configs = if let Some(path) = path {
        let contents = std::fs::read_to_string(path).context("cannot read config")?;
        toml::from_str::<Config>(&contents)
            .map_err(|_| anyhow::anyhow!("invalid provider TOML configuration"))?
            .providers
    } else {
        vec![
            ProviderConfig {
                name: "nyaa".into(),
                kind: Kind::Nyaa,
                url: "https://nyaa.si/".into(),
                api_key_env: None,
                default_search: true,
            },
            ProviderConfig {
                name: "knaben".into(),
                kind: Kind::Knaben,
                url: "https://api.knaben.org/v1".into(),
                api_key_env: None,
                default_search: true,
            },
            ProviderConfig {
                name: "sukebei".into(),
                kind: Kind::Sukebei,
                url: "https://sukebei.nyaa.si/".into(),
                api_key_env: None,
                default_search: true,
            },
        ]
    };
    let mut names = HashSet::new();
    for config in &configs {
        if config.name.is_empty()
            || !config
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || !names.insert(&config.name)
        {
            bail!(
                "provider names must be unique and contain only letters, digits, hyphens or underscores"
            );
        }
        let url = Url::parse(&config.url).map_err(|_| anyhow::anyhow!("invalid provider URL"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("provider URLs must use HTTP(S)");
        }
        if config.api_key_env.is_some() && !matches!(config.kind, Kind::Torznab) {
            bail!("api_key_env is only valid for Torznab providers");
        }
        if config.api_key_env.as_ref().is_some_and(|name| {
            name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || name.as_bytes()[0].is_ascii_digit()
        }) {
            bail!("api_key_env must be a valid environment variable name");
        }
    }
    if configs.is_empty() {
        bail!("configure at least one provider");
    }
    Ok(configs)
}

#[derive(Debug)]
/// A sanitized provider failure suitable for CLI diagnostics.
pub struct ProviderError {
    /// Error text that does not expose request URLs or credentials.
    pub message: String,
    /// Whether this was an HTTP, transport, or timeout-class failure.
    pub network: bool,
}
impl ProviderError {
    fn data(message: &str) -> Self {
        Self {
            message: message.into(),
            network: false,
        }
    }
    fn http(error: reqwest::Error) -> Self {
        // Never print request URLs: Torznab URLs may contain API keys.
        let message = if let Some(status) = error.status() {
            format!("HTTP {status}")
        } else if error.is_timeout() {
            "request timed out".into()
        } else {
            "HTTP transport error".into()
        };
        Self {
            message,
            network: true,
        }
    }
}

#[async_trait]
/// Asynchronous source of torrent search results.
pub trait Provider: Send + Sync {
    /// Returns the stable provider name used in diagnostics.
    fn name(&self) -> &str;
    /// Executes one logical search.
    async fn search(&self, query: &str) -> Result<Vec<Torrent>, ProviderError>;
    /// Executes a paginated search; basic providers may keep the default behavior.
    async fn search_pages(&self, query: &str, _pages: u16) -> Result<Vec<Torrent>, ProviderError> {
        self.search(query).await
    }
}

/// Built-in provider backed by a shared HTTP client.
pub struct HttpProvider {
    /// Validated provider configuration.
    pub config: ProviderConfig,
    /// HTTP client used for requests.
    pub client: Client,
}
#[async_trait]
impl Provider for HttpProvider {
    fn name(&self) -> &str {
        &self.config.name
    }
    async fn search(&self, query: &str) -> Result<Vec<Torrent>, ProviderError> {
        self.search_pages(query, 1).await
    }
    async fn search_pages(&self, query: &str, pages: u16) -> Result<Vec<Torrent>, ProviderError> {
        match self.config.kind {
            Kind::Knaben => knaben::search(self, query, pages).await,
            _ => feeds::search(self, query, pages).await,
        }
    }
}

async fn body(request: RequestBuilder) -> Result<Vec<u8>, ProviderError> {
    const MAX: usize = 8 * 1024 * 1024;
    let response = request
        .send()
        .await
        .map_err(ProviderError::http)?
        .error_for_status()
        .map_err(ProviderError::http)?;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(ProviderError::http)?;
        if bytes.len() + chunk.len() > MAX {
            return Err(ProviderError::data("response exceeds 8 MiB"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(contents: &str) -> anyhow::Result<Vec<ProviderConfig>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, contents)?;
        configuration(Some(&path))
    }

    #[test]
    fn validates_provider_specific_api_key_fields() {
        let rss = "[[providers]]\nname='feed'\nkind='rss'\nurl='https://example.org/feed'\napi_key_env='TOKEN'";
        assert!(
            config(rss)
                .err()
                .unwrap()
                .to_string()
                .contains("only valid")
        );

        let invalid_name = "[[providers]]\nname='indexer'\nkind='torznab'\nurl='https://example.org/api'\napi_key_env='1TOKEN'";
        assert!(
            config(invalid_name)
                .err()
                .unwrap()
                .to_string()
                .contains("valid environment variable")
        );
    }
}

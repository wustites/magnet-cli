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
pub enum Kind {
    Nyaa,
    Knaben,
    Torznab,
    Rss,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub name: String,
    pub kind: Kind,
    pub url: String,
    pub api_key_env: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    providers: Vec<ProviderConfig>,
}

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
            },
            ProviderConfig {
                name: "knaben".into(),
                kind: Kind::Knaben,
                url: "https://api.knaben.org/v1".into(),
                api_key_env: None,
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
    }
    if configs.is_empty() {
        bail!("configure at least one provider");
    }
    Ok(configs)
}

#[derive(Debug)]
pub struct ProviderError {
    pub message: String,
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
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, query: &str) -> Result<Vec<Torrent>, ProviderError>;
}

pub struct HttpProvider {
    pub config: ProviderConfig,
    pub client: Client,
}
#[async_trait]
impl Provider for HttpProvider {
    fn name(&self) -> &str {
        &self.config.name
    }
    async fn search(&self, query: &str) -> Result<Vec<Torrent>, ProviderError> {
        match self.config.kind {
            Kind::Knaben => knaben::search(self, query).await,
            _ => feeds::search(self, query).await,
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

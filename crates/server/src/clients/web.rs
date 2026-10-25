//! Téléchargement de pages et d'images de recettes (R-REC-60, R-REC-68), déclenché par une action de
//! l'utilisateur (ADR-0007).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, bail};

/// Taille maximale d'une page ou d'une image téléchargée.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Fetched {
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

pub type FetchFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<Fetched>> + Send + 'a>>;

pub trait Fetcher: Send + Sync {
    fn get<'a>(&'a self, url: &'a str) -> FetchFuture<'a>;
}

/// Seules les adresses `http` et `https` sont acceptées.
pub fn check_url(url: &str) -> anyhow::Result<()> {
    let lower = url.trim().to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) || lower.len() > 2_000 {
        bail!("adresse invalide : seules les pages http et https s'importent");
    }
    Ok(())
}

/// Client HTTP réel : délai court, taille bornée, aucune exécution de script.
pub struct HttpFetcher {
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(concat!("Foodflow/", env!("CARGO_PKG_VERSION"), " (import de recettes)"))
            .build()
            .context("client HTTP")?;
        Ok(Self { client })
    }
}

impl Fetcher for HttpFetcher {
    fn get<'a>(&'a self, url: &'a str) -> FetchFuture<'a> {
        Box::pin(async move {
            check_url(url)?;
            let mut response = self.client.get(url).send().await.context("page injoignable")?;
            if !response.status().is_success() {
                bail!("la page répond {}", response.status());
            }
            let content_type =
                response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_BYTES {
                    bail!("page trop volumineuse");
                }
            }
            Ok(Fetched { body, content_type })
        })
    }
}

/// Doublure des tests : des pages enregistrées, par adresse.
#[cfg(test)]
pub struct StaticFetcher {
    pub pages: std::collections::HashMap<String, Fetched>,
}

#[cfg(test)]
impl Fetcher for StaticFetcher {
    fn get<'a>(&'a self, url: &'a str) -> FetchFuture<'a> {
        Box::pin(async move { self.pages.get(url).cloned().context("page introuvable") })
    }
}

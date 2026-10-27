//! Recherche d'une photo libre pour une recette qui n'en a pas (REC-10, R-REC-73) : l'API de Pixabay, avec la
//! clé enregistrée dans les réglages (ADR-0007). Seul le nom de la recette part ; la clé, passée dans
//! l'adresse, n'apparaît jamais dans une erreur ni dans le journal. Pixabay demande de copier l'image chez
//! soi plutôt que de la lier, comme le fait Foodflow (R-REC-68).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::Value;

/// Taille maximale d'une réponse de recherche.
const MAX_BYTES: usize = 512 * 1024;

pub type PhotoFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'a>>;

pub trait PhotoSearch: Send + Sync {
    /// Cherche une photo pour `query` ; renvoie l'adresse de l'image trouvée, à télécharger ensuite.
    fn search<'a>(&'a self, api_key: &'a str, query: &'a str) -> PhotoFuture<'a>;
}

/// Adresse de l'image du premier résultat d'une réponse de Pixabay (`hits[0].largeImageURL`).
pub fn first_image(response: &Value) -> Option<String> {
    let hit = response.get("hits")?.as_array()?.first()?;
    ["largeImageURL", "webformatURL"].iter().find_map(|size| hit.get(size)?.as_str().map(str::to_owned))
}

pub struct PixabayClient {
    client: reqwest::Client,
}

impl PixabayClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .user_agent(concat!("Foodflow/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("client HTTP")?;
        Ok(Self { client })
    }
}

impl PhotoSearch for PixabayClient {
    fn search<'a>(&'a self, api_key: &'a str, query: &'a str) -> PhotoFuture<'a> {
        Box::pin(async move {
            let url = reqwest::Url::parse_with_params(
                "https://pixabay.com/api/",
                &[
                    ("key", api_key),
                    ("q", query),
                    ("lang", "fr"),
                    ("image_type", "photo"),
                    ("orientation", "horizontal"),
                    ("category", "food"),
                    ("safesearch", "true"),
                    ("per_page", "3"),
                ],
            )?;
            // `without_url` : l'adresse porte la clé.
            let mut response =
                self.client.get(url).send().await.map_err(|e| e.without_url()).context("Pixabay injoignable")?;
            if !response.status().is_success() {
                bail!("Pixabay répond {}", response.status());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.without_url())? {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_BYTES {
                    bail!("réponse de Pixabay trop volumineuse");
                }
            }
            let value: Value = serde_json::from_slice(&body).context("réponse de Pixabay illisible")?;
            Ok(first_image(&value))
        })
    }
}

/// Doublure des tests : une image par recherche, et la liste des recherches reçues.
#[cfg(test)]
#[derive(Default)]
pub struct StaticPhotoSearch {
    pub image: Option<String>,
    pub queries: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl PhotoSearch for StaticPhotoSearch {
    fn search<'a>(&'a self, _api_key: &'a str, query: &'a str) -> PhotoFuture<'a> {
        Box::pin(async move {
            self.queries.lock().map_err(|_| anyhow::anyhow!("verrou"))?.push(query.to_owned());
            Ok(self.image.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_image_of_a_pixabay_response() {
        let response = serde_json::json!({ "hits": [{ "webformatURL": "w.jpg", "largeImageURL": "l.jpg" }] });
        assert_eq!(first_image(&response).as_deref(), Some("l.jpg"));
        assert_eq!(first_image(&serde_json::json!({ "hits": [] })), None);
    }
}

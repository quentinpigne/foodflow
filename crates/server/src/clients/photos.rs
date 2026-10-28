//! Recherche de photos libres pour les recettes (REC-10, R-REC-73) : l'API de Pixabay, avec la clé enregistrée
//! dans les réglages (ADR-0007). Seuls des mots-clés partent ; la clé, passée dans l'adresse, n'apparaît
//! jamais dans une erreur ni dans le journal. Pixabay demande de copier chez soi l'image retenue plutôt que
//! de la lier, comme le fait Foodflow (R-REC-68).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::Value;

/// Taille maximale d'une réponse de recherche.
const MAX_BYTES: usize = 512 * 1024;

/// Une photo trouvée : un aperçu à montrer, et l'image à télécharger si elle est retenue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundPhoto {
    pub preview_url: String,
    pub image_url: String,
}

/// Une recherche : mots-clés, langue des mots-clés (`fr`, `en`), nombre de résultats voulus.
#[derive(Debug, Clone, Copy)]
pub struct PhotoQuery<'a> {
    pub text: &'a str,
    pub lang: &'a str,
    pub count: u32,
}

pub type PhotoFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<Vec<FoundPhoto>>> + Send + 'a>>;

pub trait PhotoSearch: Send + Sync {
    fn search<'a>(&'a self, api_key: &'a str, query: PhotoQuery<'a>) -> PhotoFuture<'a>;
}

/// Les photos d'une réponse de Pixabay (`hits[].webformatURL`, `hits[].largeImageURL`).
pub fn found(response: &Value) -> Vec<FoundPhoto> {
    let hits = response.get("hits").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    hits.iter()
        .filter_map(|hit| {
            let url = |key: &str| hit.get(key).and_then(Value::as_str).map(str::to_owned);
            let preview = url("webformatURL").or_else(|| url("previewURL"))?;
            Some(FoundPhoto {
                image_url: url("largeImageURL").unwrap_or_else(|| preview.clone()),
                preview_url: preview,
            })
        })
        .collect()
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
    fn search<'a>(&'a self, api_key: &'a str, query: PhotoQuery<'a>) -> PhotoFuture<'a> {
        Box::pin(async move {
            // Pixabay demande entre 3 et 200 résultats par page.
            let count = query.count.clamp(3, 200).to_string();
            let url = reqwest::Url::parse_with_params(
                "https://pixabay.com/api/",
                &[
                    ("key", api_key),
                    ("q", query.text),
                    ("lang", query.lang),
                    ("image_type", "photo"),
                    ("orientation", "horizontal"),
                    ("category", "food"),
                    ("safesearch", "true"),
                    ("per_page", &count),
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
            Ok(found(&value))
        })
    }
}

/// Doublure des tests : les mêmes photos à chaque recherche, et la liste des recherches reçues
/// (« mots-clés|langue »).
#[cfg(test)]
#[derive(Default)]
pub struct StaticPhotoSearch {
    pub photos: Vec<FoundPhoto>,
    pub queries: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl StaticPhotoSearch {
    /// Une seule photo, à la même adresse en aperçu et en grand.
    pub fn one(url: &str) -> Self {
        let photo = FoundPhoto { preview_url: url.to_owned(), image_url: url.to_owned() };
        Self { photos: vec![photo], ..Default::default() }
    }
}

#[cfg(test)]
impl PhotoSearch for StaticPhotoSearch {
    fn search<'a>(&'a self, _api_key: &'a str, query: PhotoQuery<'a>) -> PhotoFuture<'a> {
        Box::pin(async move {
            self.queries.lock().map_err(|_| anyhow::anyhow!("verrou"))?.push(format!("{}|{}", query.text, query.lang));
            Ok(self.photos.iter().take(query.count as usize).cloned().collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_of_a_pixabay_response() {
        let response = serde_json::json!({ "hits": [
            { "webformatURL": "https://pixabay.com/get/w.jpg", "largeImageURL": "https://pixabay.com/get/l.jpg" },
            { "previewURL": "https://cdn.pixabay.com/p.jpg" }
        ] });
        let photos = found(&response);
        assert_eq!(photos[0].image_url, "https://pixabay.com/get/l.jpg");
        assert_eq!(photos[0].preview_url, "https://pixabay.com/get/w.jpg");
        assert_eq!(photos[1].image_url, "https://cdn.pixabay.com/p.jpg", "à défaut de grande image, l'aperçu");
        assert!(found(&serde_json::json!({ "hits": [] })).is_empty());
    }
}

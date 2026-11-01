//! Fiches produits d'Open Food Facts (ALI-08, R-ALI-60), à la lecture d'un code-barres inconnu (ADR-0007).
//! Seul le code-barres part ; la réponse est une proposition, soumise à validation (ADR-0005).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, bail};

/// Taille maximale d'une fiche.
const MAX_BYTES: usize = 1024 * 1024;

/// Champs demandés : ceux que lit `readers::off`.
const FIELDS: &str = "code,product_name,product_name_fr,brands,quantity,product_quantity,product_quantity_unit,categories_tags,categories_tags_fr,nutriments";

/// `Ok(None)` : produit absent d'Open Food Facts ; `Ok(Some(json))` : la fiche brute.
pub type FactsFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send + 'a>>;

pub trait ProductFacts: Send + Sync {
    fn fetch<'a>(&'a self, barcode: &'a str) -> FactsFuture<'a>;
}

pub struct OffClient {
    client: reqwest::Client,
}

impl OffClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            // Open Food Facts demande un agent qui nomme l'application.
            .user_agent(concat!("Foodflow/", env!("CARGO_PKG_VERSION"), " (gestion alimentaire du foyer)"))
            .build()
            .context("client HTTP")?;
        Ok(Self { client })
    }
}

impl ProductFacts for OffClient {
    fn fetch<'a>(&'a self, barcode: &'a str) -> FactsFuture<'a> {
        Box::pin(async move {
            let url = reqwest::Url::parse_with_params(
                &format!("https://world.openfoodfacts.org/api/v2/product/{barcode}.json"),
                &[("fields", FIELDS)],
            )?;
            let mut response = self.client.get(url).send().await.context("Open Food Facts injoignable")?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !response.status().is_success() {
                bail!("Open Food Facts répond {}", response.status());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                body.extend_from_slice(&chunk);
                if body.len() > MAX_BYTES {
                    bail!("fiche d'Open Food Facts trop volumineuse");
                }
            }
            Ok(Some(String::from_utf8(body).context("fiche d'Open Food Facts illisible")?))
        })
    }
}

/// Doublure des tests : des fiches par code-barres, et les codes demandés.
#[cfg(test)]
#[derive(Default)]
pub struct StaticFacts {
    pub sheets: std::collections::HashMap<String, String>,
    pub fail: bool,
    pub asked: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl ProductFacts for StaticFacts {
    fn fetch<'a>(&'a self, barcode: &'a str) -> FactsFuture<'a> {
        Box::pin(async move {
            self.asked.lock().unwrap().push(barcode.to_owned());
            if self.fail {
                bail!("Open Food Facts injoignable");
            }
            Ok(self.sheets.get(barcode).cloned())
        })
    }
}

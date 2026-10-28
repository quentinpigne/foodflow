//! Photo libre d'une recette qui n'en a pas (REC-10, R-REC-73) : réglages et recherche, à la création d'un
//! brouillon (ADR-0007). Seul le nom de la recette part ; l'image trouvée est copiée dans le volume
//! (R-REC-68).

use foodflow_domain::shared::ids::{RecipeDraftId, RecipeId};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::images;
use crate::clients::photos::{PhotoQuery, PhotoSearch};
use crate::clients::web::{Fetched, Fetcher, check_url};
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const ENABLED: &str = "photos.enabled";
const API_KEY: &str = "photos.api_key";

/// Mots vides retirés de la recherche de repli.
const STOP_WORDS: [&str; 12] = ["de", "du", "des", "d", "la", "le", "les", "l", "à", "au", "aux", "et"];

/// Réglages des photos, tels que l'interface les voit : la clé n'est jamais renvoyée.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PhotoSettings {
    pub enabled: bool,
    /// Une clé est enregistrée.
    pub has_key: bool,
    /// Activée, avec une clé : utilisable.
    pub ready: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PhotoSettingsInput {
    pub enabled: bool,
    /// Absente : la clé enregistrée est gardée ; vide : elle est effacée.
    pub api_key: Option<String>,
}

async fn key(conn: &mut SqliteConnection) -> anyhow::Result<Option<String>> {
    let key: Option<String> = db::settings::get(&mut *conn, API_KEY).await?;
    Ok(key.filter(|k| !k.is_empty()))
}

pub async fn get_settings(db: &Db) -> AppResult<PhotoSettings> {
    let mut conn = db.read.acquire().await?;
    let enabled: bool = db::settings::get(&mut *conn, ENABLED).await?.unwrap_or(false);
    let has_key = key(&mut conn).await?.is_some();
    Ok(PhotoSettings { enabled, has_key, ready: enabled && has_key })
}

pub async fn update_settings(db: &Db, input: &PhotoSettingsInput) -> AppResult<()> {
    if input.api_key.as_deref().is_some_and(|k| k.chars().count() > 200) {
        return Err(AppError::invalid_field("TOO_LONG", "api_key", "200 caractères au maximum"));
    }
    let mut tx = db.write.begin().await?;
    db::settings::set(&mut *tx, ENABLED, &input.enabled).await?;
    if let Some(key) = &input.api_key {
        db::settings::set(&mut *tx, API_KEY, &key.trim()).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Recherches tentées pour un nom de recette : le nom entier, puis, s'il a plusieurs mots, le nom sans son
/// premier mot ni les mots vides (« Velouté de courgettes » → « courgettes »).
pub fn queries(name: &str) -> Vec<String> {
    let name = name.trim();
    let mut queries = vec![name.to_owned()];
    let words: Vec<&str> = name.split([' ', '\'', '’']).filter(|w| !w.is_empty()).collect();
    let rest: Vec<&str> =
        words.iter().skip(1).copied().filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str())).collect();
    if !rest.is_empty() {
        queries.push(rest.join(" "));
    }
    queries.retain(|q| !q.is_empty());
    queries.dedup();
    queries
}

/// Clé de Pixabay, si le service est activé.
async fn ready_key(db: &Db) -> anyhow::Result<Option<String>> {
    let mut conn = db.read.acquire().await?;
    let enabled: bool = db::settings::get(&mut *conn, ENABLED).await?.unwrap_or(false);
    Ok(if enabled { key(&mut conn).await? } else { None })
}

/// Cherche une photo pour une recette et la copie dans le volume sous `stem` ; renvoie son nom de fichier.
/// Les mots-clés de l'IA (en anglais) passent d'abord, puis le nom de la recette. La photo est un plus : sans
/// réglage, sans résultat ou en cas d'échec, la recette reste sans photo.
pub async fn find(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    stem: &str,
    name: &str,
    ai_query: Option<&str>,
) -> Option<String> {
    let key = ready_key(db).await.ok()??;
    let searches =
        ai_query.map(|q| (q.to_owned(), "en")).into_iter().chain(queries(name).into_iter().map(|q| (q, "fr")));
    for (text, lang) in searches {
        match photos.search(&key, PhotoQuery { text: &text, lang, count: 3 }).await {
            Ok(found) => {
                let Some(photo) = found.into_iter().find(|p| check_url(&p.image_url).is_ok()) else { continue };
                let fetched = fetcher.get(&photo.image_url).await.ok()?;
                return images::store(data, stem, fetched.content_type.as_deref(), &fetched.body).ok();
            }
            Err(error) => {
                tracing::warn!(error = format!("{error:#}"), "recherche de photo impossible");
                return None;
            }
        }
    }
    None
}

/// Nombre de photos proposées au choix.
pub const CHOICES: u32 = 9;

/// Une photo proposée au choix : l'aperçu montré, et l'adresse de l'image à retenir.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PhotoCandidate {
    pub preview_url: String,
    pub image_url: String,
}

/// Adresse d'une image à retenir (une photo proposée, ou une adresse saisie).
#[derive(Debug, Deserialize, ToSchema)]
pub struct ImageUrlInput {
    pub url: String,
}

/// Photos proposées pour des mots-clés en français (R-REC-73).
pub async fn search(db: &Db, photos: &dyn PhotoSearch, text: &str) -> AppResult<Vec<PhotoCandidate>> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 100 {
        return Err(AppError::invalid_field("INVALID_QUERY", "q", "de 1 à 100 caractères"));
    }
    let key = ready_key(db).await?.ok_or_else(|| {
        AppError::invalid("PHOTOS_DISABLED", "photos Pixabay désactivées : les activer avec leur clé dans les réglages")
    })?;
    let found = photos
        .search(&key, PhotoQuery { text, lang: "fr", count: CHOICES })
        .await
        .map_err(|e| AppError::invalid("PHOTOS_UNAVAILABLE", format!("{e:#}")))?;
    Ok(found
        .into_iter()
        .filter(|p| check_url(&p.image_url).is_ok())
        .map(|p| PhotoCandidate { preview_url: p.preview_url, image_url: p.image_url })
        .collect())
}

/// Télécharge l'image d'une adresse http(s), à la demande de l'utilisateur.
async fn download(fetcher: &dyn Fetcher, url: &str) -> AppResult<Fetched> {
    let url = url.trim();
    check_url(url).map_err(|e| AppError::invalid_field("INVALID_URL", "url", e.to_string()))?;
    fetcher
        .get(url)
        .await
        .map_err(|e| AppError::invalid_field("IMAGE_UNREACHABLE", "url", format!("image inaccessible : {e}")))
}

/// Retient pour une recette l'image d'une adresse (photo proposée ou adresse saisie).
pub async fn choose_for_recipe(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    id: RecipeId,
    input: &ImageUrlInput,
) -> AppResult<()> {
    let fetched = download(fetcher, &input.url).await?;
    images::upload(db, data, id, fetched.content_type.as_deref(), &fetched.body).await
}

/// Retient pour un brouillon l'image d'une adresse (photo proposée ou adresse saisie).
pub async fn choose_for_draft(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    id: RecipeDraftId,
    input: &ImageUrlInput,
) -> AppResult<()> {
    let fetched = download(fetcher, &input.url).await?;
    images::upload_draft(db, data, id, fetched.content_type.as_deref(), &fetched.body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_rec_73_queries_fall_back_to_the_main_words() {
        assert_eq!(queries("Velouté de courgettes"), ["Velouté de courgettes", "courgettes"]);
        assert_eq!(queries("Salade d'endives aux noix"), ["Salade d'endives aux noix", "endives noix"]);
        assert_eq!(queries("Ratatouille"), ["Ratatouille"]);
    }
}

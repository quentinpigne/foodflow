//! Photo libre d'une recette qui n'en a pas (REC-10, R-REC-73) : réglages et recherche, à la création d'un
//! brouillon (ADR-0007). Seul le nom de la recette part ; l'image trouvée est copiée dans le volume
//! (R-REC-68).

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::images;
use crate::clients::photos::PhotoSearch;
use crate::clients::web::{Fetcher, check_url};
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

/// Cherche une photo pour une recette et la copie dans le volume sous `stem` ; renvoie son nom de fichier.
/// La photo est un plus : sans réglage, sans résultat ou en cas d'échec, la recette reste sans photo.
pub async fn find(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    stem: &str,
    name: &str,
) -> Option<String> {
    let key = {
        let mut conn = db.read.acquire().await.ok()?;
        let enabled: bool = db::settings::get(&mut *conn, ENABLED).await.ok()?.unwrap_or(false);
        if !enabled {
            return None;
        }
        key(&mut conn).await.ok()??
    };
    for query in queries(name) {
        match photos.search(&key, &query).await {
            Ok(Some(url)) if check_url(&url).is_ok() => {
                let fetched = fetcher.get(&url).await.ok()?;
                return images::store(data, stem, fetched.content_type.as_deref(), &fetched.body).ok();
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(error = format!("{error:#}"), "recherche de photo impossible");
                return None;
            }
        }
    }
    None
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

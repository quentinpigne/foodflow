//! Open Food Facts (ALI-08) : brouillon de produit à la lecture d'un code inconnu (R-ALI-60), fiche gardée
//! (R-ALI-61), rattachement mémorisé par catégorie (R-ALI-62). Activé par défaut, désactivable (ADR-0007) ;
//! seul le code-barres part.

use std::collections::HashMap;

use foodflow_domain::foods::NutrientBasis;
use foodflow_domain::recipes::Target;
use foodflow_domain::recipes::matching::{self, Candidate, Confidence};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, ProductId};
use foodflow_domain::{Amount, Measure};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::catalog::Catalog;
use crate::clients::off::ProductFacts;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::readers::off::{self, OffProduct};

const ENABLED: &str = "off.enabled";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OffSettings {
    pub enabled: bool,
}

pub async fn get_settings(db: impl sqlx::SqliteExecutor<'_>) -> anyhow::Result<OffSettings> {
    Ok(OffSettings { enabled: db::settings::get(db, ENABLED).await?.unwrap_or(true) })
}

pub async fn update_settings(db: &Db, input: &OffSettings) -> AppResult<()> {
    db::settings::set(&db.write, ENABLED, &input.enabled).await?;
    Ok(())
}

/// Ce qu'a donné Open Food Facts pour un code inconnu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OffStatus {
    /// Service désactivé dans les réglages.
    Disabled,
    /// Fiche trouvée : un brouillon est proposé.
    Found,
    /// Produit absent d'Open Food Facts.
    NotFound,
    /// Service injoignable ou réponse illisible.
    Unavailable,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DraftNutrient {
    pub code: String,
    pub value: i64,
}

/// Brouillon de produit (R-ALI-60) : rien n'est créé avant validation (ADR-0005).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProductDraft {
    pub name: Option<String>,
    pub brand: Option<String>,
    /// Mesure du conditionnement lu ; il ne vaut que si elle est celle de l'aliment choisi.
    pub package_measure: Option<Measure>,
    pub package_amount: Option<Amount>,
    pub package_units: Option<i64>,
    pub nutrient_basis: NutrientBasis,
    pub nutrients: Vec<DraftNutrient>,
    /// Catégorie la plus précise (`en:spaghetti`).
    pub category: Option<String>,
    /// Rattachement proposé : catégorie mémorisée (R-ALI-62), sinon d'après le nom.
    pub proposed_food_id: Option<FoodId>,
    pub proposed_group_id: Option<FoodGroupId>,
    /// Nom de l'aliment ou chemin de la famille proposés.
    pub proposed_name: Option<String>,
    /// Fiche brute, renvoyée à la création du produit pour être gardée (R-ALI-61).
    pub off_payload: String,
}

/// Interroge Open Food Facts pour un code inconnu, si le service est activé (R-ALI-60).
///
/// Aucune connexion à la base n'est gardée pendant l'appel au service.
pub(crate) async fn draft(
    db: &Db,
    catalog: &Catalog,
    facts: &dyn ProductFacts,
    barcode: &str,
) -> AppResult<(OffStatus, Option<ProductDraft>)> {
    let (settings, mappings) = {
        let mut conn = db.read.acquire().await?;
        (get_settings(&mut *conn).await?, db::products::off_mappings(&mut *conn).await?)
    };
    if !settings.enabled {
        return Ok((OffStatus::Disabled, None));
    }
    let payload = match facts.fetch(barcode).await {
        Ok(Some(payload)) => payload,
        Ok(None) => return Ok((OffStatus::NotFound, None)),
        Err(error) => {
            tracing::warn!("Open Food Facts : {error:#}");
            return Ok((OffStatus::Unavailable, None));
        }
    };
    let product = match off::parse(&payload) {
        Ok(Some(product)) => product,
        Ok(None) => return Ok((OffStatus::NotFound, None)),
        Err(error) => {
            tracing::warn!("Open Food Facts : {error:#}");
            return Ok((OffStatus::Unavailable, None));
        }
    };
    let target = propose(catalog, &mappings, &product);
    let (proposed_food_id, proposed_group_id, proposed_name) = match target {
        Some(Target::Food(food)) => (Some(food), None, catalog.foods.get(&food).map(|f| f.name.clone())),
        Some(Target::Group(group)) => (None, Some(group), Some(catalog.group_path(group))),
        _ => (None, None, None),
    };
    Ok((
        OffStatus::Found,
        Some(ProductDraft {
            name: product.name,
            brand: product.brand,
            package_measure: product.package.map(|(m, _)| m),
            package_amount: product.package.map(|(_, a)| a),
            package_units: product.package_units,
            nutrient_basis: product.nutrient_basis,
            nutrients: product
                .nutrients
                .iter()
                .map(|(code, value)| DraftNutrient { code: (*code).to_owned(), value: *value })
                .collect(),
            category: product.categories.last().cloned(),
            proposed_food_id,
            proposed_group_id,
            proposed_name,
            off_payload: payload,
        }),
    ))
}

/// Rattachement proposé : la catégorie mémorisée la plus précise (R-ALI-62), sinon un aliment ou une
/// famille dont le nom correspond bien à celui du produit, puis à l'une de ses catégories en français, de
/// la plus précise à la plus générale.
fn propose(
    catalog: &Catalog,
    mappings: &[(String, Option<FoodId>, Option<FoodGroupId>)],
    product: &OffProduct,
) -> Option<Target> {
    let memory: HashMap<&str, Target> = mappings
        .iter()
        .filter_map(|(category, food, group)| {
            let target = match (food, group) {
                (Some(f), _) if catalog.foods.contains_key(f) => Target::Food(*f),
                (_, Some(g)) if catalog.groups.contains_key(g) => Target::Group(*g),
                _ => return None,
            };
            Some((category.as_str(), target))
        })
        .collect();
    if let Some(target) = product.categories.iter().rev().find_map(|c| memory.get(c.as_str())) {
        return Some(*target);
    }
    let mut candidates: Vec<Candidate> = catalog
        .foods
        .values()
        .map(|f| Candidate { target: Target::Food(f.id), name: f.name.clone() })
        .chain(catalog.groups.values().map(|g| Candidate { target: Target::Group(g.id), name: g.name.clone() }))
        .collect();
    // Ordre stable, pour une proposition déterministe.
    candidates.sort_by(|a, b| a.name.cmp(&b.name));
    product.name.iter().chain(product.category_labels.iter().rev()).find_map(|name| {
        let proposal = matching::propose(name, &HashMap::new(), &candidates);
        proposal.target.filter(|_| proposal.confidence >= Confidence::Medium)
    })
}

/// Produit créé depuis un brouillon : garde la fiche, ses valeurs nutritionnelles (R-ALI-61), et mémorise
/// le rattachement de sa catégorie la plus précise (R-ALI-62).
pub(crate) async fn apply(
    conn: &mut SqliteConnection,
    id: ProductId,
    payload: &str,
    food: Option<FoodId>,
    group: Option<FoodGroupId>,
) -> AppResult<()> {
    let product = off::parse(payload)
        .ok()
        .flatten()
        .ok_or_else(|| AppError::invalid_field("OFF_PAYLOAD", "off_payload", "fiche d'Open Food Facts illisible"))?;
    db::products::set_off_payload(&mut *conn, id, payload).await?;
    for (code, value) in &product.nutrients {
        db::products::set_nutrient(&mut *conn, id, code, *value, "OFF").await?;
    }
    if let Some(category) = product.categories.last() {
        db::products::set_off_mapping(&mut *conn, category, food, group).await?;
    }
    Ok(())
}

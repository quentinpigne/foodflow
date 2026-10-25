//! Aliments : recherche, fiche, saisie, valeurs nutritionnelles, unités propres (spec 01, ALI-02 à ALI-06).

use std::collections::HashMap;

use foodflow_domain::foods::nutrients::NutrientValue;
use foodflow_domain::foods::{NutrientBasis, ValueSource};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, ProductId, UnitId};
use foodflow_domain::text::{matches, normalize};
use foodflow_domain::{Amount, DomainError};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::catalog::{Catalog, EffectiveSettings};
use super::products::{Product, to_product};
use super::units::{Unit, UnitInput, to_unit};
use super::{clean, required};
use crate::db::foods::FoodFields;
use crate::db::groups::Settings;
use crate::db::units::UnitFields;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Nombre maximal de résultats d'une recherche.
const SEARCH_LIMIT: usize = 50;

#[derive(Debug, Serialize, ToSchema)]
pub struct FoodSummary {
    pub id: FoodId,
    pub name: String,
    pub group_id: FoodGroupId,
    pub group_path: String,
    pub ciqual_code: Option<String>,
    pub hidden: bool,
    pub effective: EffectiveSettings,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FoodNutrient {
    pub code: String,
    pub value: i64,
    pub estimated: bool,
    pub source: ValueSource,
    /// Valeur Ciqual remplacée par la saisie, qu'on peut rétablir (R-ALI-11).
    pub reference_value: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FoodDetail {
    #[serde(flatten)]
    pub summary: FoodSummary,
    pub aliases: Vec<String>,
    pub nutrient_basis: NutrientBasis,
    pub density_mg_per_ml: Option<i64>,
    pub piece_weight_mg: Option<i64>,
    /// Réglages propres ; `null` = hérité de la famille.
    pub settings: Settings,
    pub minimum_stock: Option<Amount>,
    pub preferred_product_id: Option<ProductId>,
    /// Valeurs connues ; un nutriment absent est inconnu (R-ALI-20).
    pub nutrients: Vec<FoodNutrient>,
    pub units: Vec<Unit>,
    pub products: Vec<Product>,
}

#[derive(Default, Debug, Deserialize, IntoParams)]
pub struct FoodQuery {
    /// Mots recherchés dans le nom et les alias, sans accents ni casse.
    pub q: Option<String>,
    pub group_id: Option<FoodGroupId>,
    /// Inclure les aliments Ciqual masqués (R-ALI-52).
    #[serde(default)]
    pub include_hidden: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FoodInput {
    pub group_id: FoodGroupId,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub nutrient_basis: Option<NutrientBasis>,
    pub density_mg_per_ml: Option<i64>,
    pub piece_weight_mg: Option<i64>,
    #[serde(default)]
    pub settings: Settings,
    pub minimum_stock: Option<Amount>,
    pub preferred_product_id: Option<ProductId>,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NutrientValueInput {
    pub code: String,
    /// `null` : valeur inconnue (R-ALI-20).
    pub value: Option<i64>,
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NutrientsInput {
    pub values: Vec<NutrientValueInput>,
}

fn summary(catalog: &Catalog, food: &db::foods::FoodRow) -> FoodSummary {
    FoodSummary {
        id: food.id,
        name: food.name.clone(),
        group_id: food.group_id,
        group_path: catalog.group_path(food.group_id),
        ciqual_code: food.ciqual_code.clone(),
        hidden: food.hidden,
        effective: catalog.food_settings(food),
    }
}

/// Recherche dans les noms et les alias, sans accents ni casse (ALI-06).
pub async fn search(db: &Db, query: &FoodQuery) -> AppResult<Vec<FoodSummary>> {
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    let mut aliases: HashMap<FoodId, Vec<String>> = HashMap::new();
    for (food, key) in db::foods::all_alias_keys(&mut *conn).await? {
        aliases.entry(food).or_default().push(key);
    }
    let text = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    let in_group = |food: &db::foods::FoodRow| match query.group_id {
        None => true,
        Some(group) => {
            food.group_id == group
                || foodflow_domain::foods::groups::ancestors(&catalog.parents, food.group_id).contains(&group)
        }
    };
    let mut found: Vec<(bool, &db::foods::FoodRow)> = catalog
        .foods
        .values()
        .filter(|f| query.include_hidden || !f.hidden || text.is_some())
        .filter(|f| in_group(f))
        .filter_map(|f| match text {
            None => Some((true, f)),
            Some(q) => {
                let by_name = matches(&f.name_key, q);
                let by_alias = aliases.get(&f.id).is_some_and(|keys| keys.iter().any(|k| matches(k, q)));
                (by_name || by_alias).then_some((f.name_key.starts_with(&normalize(q)), f))
            }
        })
        .collect();
    // Les aliments visibles et ceux dont le nom commence par la recherche d'abord.
    found.sort_by(|(a_prefix, a), (b_prefix, b)| {
        a.hidden.cmp(&b.hidden).then(b_prefix.cmp(a_prefix)).then(a.name_key.len().cmp(&b.name_key.len()))
    });
    Ok(found.into_iter().take(SEARCH_LIMIT).map(|(_, f)| summary(&catalog, f)).collect())
}

pub async fn get(db: &Db, id: FoodId) -> AppResult<FoodDetail> {
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    let food = db::foods::find(&mut *conn, id).await?.ok_or(AppError::NotFound("aliment"))?;
    let nutrients = db::nutrients::food_values(&mut *conn, id)
        .await?
        .into_iter()
        .map(|n| FoodNutrient {
            code: n.code,
            value: n.value.value,
            estimated: n.value.estimated,
            source: n.value.source,
            reference_value: n.reference_value,
        })
        .collect();
    let units = db::units::list(&mut *conn, Some(id)).await?.into_iter().filter(|u| u.food_id == Some(id)).map(to_unit);
    let products = db::products::list_for_food(&mut *conn, id).await?;
    Ok(FoodDetail {
        summary: summary(&catalog, &food),
        aliases: db::foods::aliases(&mut *conn, id).await?,
        nutrient_basis: food.nutrient_basis,
        density_mg_per_ml: food.density_mg_per_ml,
        piece_weight_mg: food.piece_weight_mg,
        settings: food.settings,
        minimum_stock: food.minimum_stock,
        preferred_product_id: food.preferred_product_id,
        nutrients,
        units: units.collect(),
        products: products.iter().map(|p| to_product(&catalog, p)).collect(),
    })
}

fn positive(value: Option<i64>, field: &'static str) -> AppResult<Option<i64>> {
    match value {
        Some(v) if v <= 0 => Err(AppError::invalid_field("NOT_POSITIVE", field, "une valeur positive est attendue")),
        other => Ok(other),
    }
}

/// Alias nettoyés, sans doublon ni répétition du nom (R-ALI-10).
fn clean_aliases(name_key: &str, aliases: &[String]) -> AppResult<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for alias in aliases {
        let Some(alias) = clean(Some(alias)) else { continue };
        let key = normalize(&alias);
        if key != name_key && !out.iter().any(|(_, k)| *k == key) {
            out.push((required(&alias, "aliases", 200)?, key));
        }
    }
    Ok(out)
}

async fn check_names(
    conn: &mut sqlx::SqliteConnection,
    key: &str,
    aliases: &[(String, String)],
    except: Option<FoodId>,
) -> AppResult<()> {
    if db::foods::name_taken(&mut *conn, key, except).await? {
        return Err(DomainError::FoodNameTaken.into());
    }
    for (_, alias) in aliases {
        if db::foods::name_taken(&mut *conn, alias, except).await? {
            return Err(DomainError::AliasTaken.into());
        }
    }
    Ok(())
}

pub async fn create(db: &Db, input: &FoodInput) -> AppResult<FoodId> {
    let name = required(&input.name, "name", 200)?;
    let key = normalize(&name);
    let aliases = clean_aliases(&key, &input.aliases)?;
    let mut tx = db.write.begin().await?;
    let catalog = Catalog::load(&mut tx).await?;
    if !catalog.groups.contains_key(&input.group_id) {
        return Err(AppError::NotFound("famille"));
    }
    check_names(&mut tx, &key, &aliases, None).await?;
    let id = FoodId::new();
    let fields = FoodFields {
        group_id: input.group_id,
        name: &name,
        name_key: &key,
        ciqual_code: None,
        hidden: input.hidden,
        nutrient_basis: input.nutrient_basis.unwrap_or(NutrientBasis::Per100G),
        density_mg_per_ml: positive(input.density_mg_per_ml, "density_mg_per_ml")?,
        piece_weight_mg: positive(input.piece_weight_mg, "piece_weight_mg")?,
        settings: input.settings,
        minimum_stock: input.minimum_stock,
        preferred_product_id: None,
        user_modified: true,
    };
    db::foods::insert(&mut *tx, id, &fields).await?;
    db::foods::replace_aliases(&mut tx, id, &aliases).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: FoodId, input: &FoodInput) -> AppResult<()> {
    let name = required(&input.name, "name", 200)?;
    let key = normalize(&name);
    let aliases = clean_aliases(&key, &input.aliases)?;
    let mut tx = db.write.begin().await?;
    let catalog = Catalog::load(&mut tx).await?;
    let current = catalog.foods.get(&id).ok_or(AppError::NotFound("aliment"))?;
    if !catalog.groups.contains_key(&input.group_id) {
        return Err(AppError::NotFound("famille"));
    }
    if let Some(product) = input.preferred_product_id
        && catalog.products.get(&product).and_then(|p| p.food_id) != Some(id)
    {
        return Err(AppError::invalid_field(
            "PREFERRED_PRODUCT_OF_ANOTHER_FOOD",
            "preferred_product_id",
            "le produit habituel doit être un produit de cet aliment (R-ALI-43)",
        ));
    }
    // Le mode de suivi change par un service dédié, qui convertit les lots (R-STK-21).
    if input.settings.tracking_mode != current.settings.tracking_mode {
        let before = catalog.food_settings(current).tracking_mode;
        let mut after_row = current.clone();
        after_row.settings.tracking_mode = input.settings.tracking_mode;
        if catalog.food_settings(&after_row).tracking_mode != before
            && super::stock::has_active_lots(&mut tx, id).await?
        {
            return Err(AppError::invalid_field(
                "TRACKING_MODE_HAS_LOTS",
                "settings.tracking_mode",
                "l'aliment a des lots en stock : changer le mode de suivi depuis le stock (R-STK-21)",
            ));
        }
    }
    check_names(&mut tx, &key, &aliases, Some(id)).await?;
    let user_modified = current.user_modified || current.name != name || current.group_id != input.group_id;
    let fields = FoodFields {
        group_id: input.group_id,
        name: &name,
        name_key: &key,
        ciqual_code: current.ciqual_code.as_deref(),
        hidden: input.hidden,
        nutrient_basis: input.nutrient_basis.unwrap_or(current.nutrient_basis),
        density_mg_per_ml: positive(input.density_mg_per_ml, "density_mg_per_ml")?,
        piece_weight_mg: positive(input.piece_weight_mg, "piece_weight_mg")?,
        settings: input.settings,
        minimum_stock: input.minimum_stock,
        preferred_product_id: input.preferred_product_id,
        user_modified,
    };
    db::foods::update(&mut *tx, id, &fields).await?;
    db::foods::replace_aliases(&mut tx, id, &aliases).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete(db: &Db, id: FoodId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::foods::find(&mut *tx, id).await?.ok_or(AppError::NotFound("aliment"))?;
    if db::foods::usage(&mut *tx, id).await? > 0 {
        return Err(DomainError::FoodInUse.into());
    }
    db::foods::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Saisit des valeurs nutritionnelles. Une valeur Ciqual remplacée est conservée pour être rétablie
/// (R-ALI-11) ; `null` rend la valeur inconnue (R-ALI-20).
pub async fn set_nutrients(db: &Db, id: FoodId, input: &NutrientsInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::foods::find(&mut *tx, id).await?.ok_or(AppError::NotFound("aliment"))?;
    let known: Vec<String> = db::nutrients::list(&mut *tx).await?.into_iter().map(|n| n.code).collect();
    let current: HashMap<String, db::nutrients::FoodNutrientRow> =
        db::nutrients::food_values(&mut *tx, id).await?.into_iter().map(|n| (n.code.clone(), n)).collect();
    for entry in &input.values {
        if !known.contains(&entry.code) {
            return Err(AppError::invalid_field(
                "UNKNOWN_NUTRIENT",
                "values",
                format!("nutriment inconnu : {}", entry.code),
            ));
        }
        let before = current.get(&entry.code);
        // Valeur Ciqual d'origine : celle en place si elle vient de Ciqual, ou celle déjà mémorisée.
        let reference = before.and_then(|b| match b.value.source {
            ValueSource::Ciqual => Some(b.value.value),
            _ => b.reference_value,
        });
        match entry.value {
            Some(value) if value < 0 => {
                return Err(AppError::invalid_field("NOT_POSITIVE", "values", "une valeur positive est attendue"));
            }
            Some(value) => {
                let value = NutrientValue { value, estimated: entry.estimated, source: ValueSource::Manual };
                db::nutrients::set_food_value(&mut *tx, id, &entry.code, value, reference).await?;
            }
            None => db::nutrients::delete_food_value(&mut *tx, id, &entry.code).await?,
        }
    }
    tx.commit().await?;
    Ok(())
}

/// Rétablit la valeur Ciqual d'un nutriment remplacée par une saisie (R-ALI-11).
pub async fn restore_ciqual(db: &Db, id: FoodId, code: &str) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let current = db::nutrients::food_values(&mut *tx, id).await?.into_iter().find(|n| n.code == code);
    let reference = current.and_then(|n| n.reference_value).ok_or(AppError::NotFound("valeur Ciqual"))?;
    let value = NutrientValue { value: reference, estimated: false, source: ValueSource::Ciqual };
    db::nutrients::set_food_value(&mut *tx, id, code, value, None).await?;
    tx.commit().await?;
    Ok(())
}

fn unit_fields<'a>(food: FoodId, input: &'a UnitInput, name: &'a str) -> AppResult<UnitFields<'a>> {
    if !input.amount.is_positive() {
        return Err(DomainError::NotPositive.into());
    }
    Ok(UnitFields {
        food_id: Some(food),
        name,
        plural: input.plural.as_deref(),
        abbreviation: input.abbreviation.as_deref(),
        measure: input.measure,
        amount: input.amount,
        position: input.position.unwrap_or(0),
    })
}

/// Ajoute une unité propre à l'aliment (gousse, sachet, tranche…).
pub async fn add_unit(db: &Db, food: FoodId, input: &UnitInput) -> AppResult<UnitId> {
    let name = required(&input.name, "name", 60)?;
    let mut tx = db.write.begin().await?;
    db::foods::find(&mut *tx, food).await?.ok_or(AppError::NotFound("aliment"))?;
    let id = UnitId::new();
    db::units::insert(&mut *tx, id, &unit_fields(food, input, &name)?).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update_unit(db: &Db, food: FoodId, unit: UnitId, input: &UnitInput) -> AppResult<()> {
    let name = required(&input.name, "name", 60)?;
    let mut tx = db.write.begin().await?;
    let exists = db::units::list(&mut *tx, Some(food)).await?.iter().any(|u| u.id == unit && u.food_id == Some(food));
    if !exists {
        return Err(AppError::NotFound("unité"));
    }
    db::units::update(&mut *tx, unit, &unit_fields(food, input, &name)?).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_unit(db: &Db, food: FoodId, unit: UnitId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let exists = db::units::list(&mut *tx, Some(food)).await?.iter().any(|u| u.id == unit && u.food_id == Some(food));
    if !exists {
        return Err(AppError::NotFound("unité"));
    }
    db::units::delete(&mut *tx, unit).await?;
    tx.commit().await?;
    Ok(())
}

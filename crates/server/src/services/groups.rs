//! Familles d'aliments (spec 01, ALI-01).

use foodflow_domain::DomainError;
use foodflow_domain::foods::groups::check_parent;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId};
use foodflow_domain::text::normalize;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::catalog::{Catalog, EffectiveSettings};
use super::required;
use crate::db::groups::{GroupFields, Settings};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct FoodGroup {
    pub id: FoodGroupId,
    pub parent_id: Option<FoodGroupId>,
    pub name: String,
    /// Chemin depuis la racine (« Produits laitiers › Laits »).
    pub path: String,
    pub ciqual_code: Option<String>,
    /// Réglages propres ; `null` = hérité.
    pub settings: Settings,
    pub effective: EffectiveSettings,
    pub reference_food_id: Option<FoodId>,
    pub food_count: i64,
    pub product_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FoodGroupInput {
    pub parent_id: Option<FoodGroupId>,
    pub name: String,
    #[serde(default)]
    pub settings: Settings,
    pub reference_food_id: Option<FoodId>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeGroupInput {
    pub into: FoodGroupId,
}

pub async fn list(db: &Db) -> AppResult<Vec<FoodGroup>> {
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    let mut groups: Vec<FoodGroup> = catalog
        .groups
        .values()
        .map(|g| FoodGroup {
            id: g.id,
            parent_id: g.parent_id,
            name: g.name.clone(),
            path: catalog.group_path(g.id),
            ciqual_code: g.ciqual_code.clone(),
            settings: g.settings,
            effective: catalog.group_settings(g.id),
            reference_food_id: g.reference_food_id,
            food_count: g.food_count,
            product_count: g.product_count,
        })
        .collect();
    groups.sort_by_key(|g| normalize(&g.path));
    Ok(groups)
}

/// Vérifie l'aliment de référence : un aliment de la famille ou d'une sous-famille (R-ALI-4).
fn check_reference(catalog: &Catalog, group: FoodGroupId, food: Option<FoodId>) -> AppResult<()> {
    let Some(food) = food else { return Ok(()) };
    let food = catalog.foods.get(&food).ok_or(AppError::NotFound("aliment de référence"))?;
    let mut current = Some(food.group_id);
    while let Some(id) = current {
        if id == group {
            return Ok(());
        }
        current = catalog.parents.get(&id).copied().flatten();
    }
    Err(AppError::invalid_field(
        "REFERENCE_FOOD_OUTSIDE_GROUP",
        "reference_food_id",
        "l'aliment de référence doit appartenir à la famille ou à une sous-famille (R-ALI-4)",
    ))
}

pub async fn create(db: &Db, input: &FoodGroupInput) -> AppResult<FoodGroupId> {
    let name = required(&input.name, "name", 120)?;
    let key = normalize(&name);
    let mut tx = db.write.begin().await?;
    if let Some(parent) = input.parent_id
        && !Catalog::load(&mut tx).await?.groups.contains_key(&parent)
    {
        return Err(AppError::NotFound("famille parente"));
    }
    if db::groups::name_taken(&mut *tx, input.parent_id, &key, None).await? {
        return Err(DomainError::FoodGroupNameTaken.into());
    }
    let id = FoodGroupId::new();
    let fields = GroupFields {
        parent_id: input.parent_id,
        name: &name,
        name_key: &key,
        ciqual_code: None,
        settings: input.settings,
        reference_food_id: None,
        user_modified: true,
    };
    db::groups::insert(&mut *tx, id, &fields).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: FoodGroupId, input: &FoodGroupInput) -> AppResult<()> {
    let name = required(&input.name, "name", 120)?;
    let key = normalize(&name);
    let mut tx = db.write.begin().await?;
    let catalog = Catalog::load(&mut tx).await?;
    let current = catalog.groups.get(&id).ok_or(AppError::NotFound("famille"))?;
    check_parent(&catalog.parents, id, input.parent_id)?;
    if let Some(parent) = input.parent_id
        && !catalog.groups.contains_key(&parent)
    {
        return Err(AppError::NotFound("famille parente"));
    }
    if db::groups::name_taken(&mut *tx, input.parent_id, &key, Some(id)).await? {
        return Err(DomainError::FoodGroupNameTaken.into());
    }
    check_reference(&catalog, id, input.reference_food_id)?;
    // Un nom ou un parent changés à la main sont conservés par les imports Ciqual suivants (R-ALI-51).
    let user_modified = current.user_modified || current.name != name || current.parent_id != input.parent_id;
    let fields = GroupFields {
        parent_id: input.parent_id,
        name: &name,
        name_key: &key,
        ciqual_code: current.ciqual_code.as_deref(),
        settings: input.settings,
        reference_food_id: input.reference_food_id,
        user_modified,
    };
    db::groups::update(&mut *tx, id, &fields).await?;
    tx.commit().await?;
    Ok(())
}

/// Supprime une famille vide (R-ALI-3).
pub async fn delete(db: &Db, id: FoodGroupId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let catalog = Catalog::load(&mut tx).await?;
    let group = catalog.groups.get(&id).ok_or(AppError::NotFound("famille"))?;
    let has_children = catalog.groups.values().any(|g| g.parent_id == Some(id));
    let has_foods = catalog.foods.values().any(|f| f.group_id == id);
    if has_children || has_foods || group.product_count > 0 {
        return Err(DomainError::FoodGroupInUse.into());
    }
    db::groups::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Fusionne une famille dans une autre : tout ce qui la vise est déplacé, puis elle est supprimée (R-ALI-3).
pub async fn merge(db: &Db, id: FoodGroupId, into: FoodGroupId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let catalog = Catalog::load(&mut tx).await?;
    if !catalog.groups.contains_key(&id) || !catalog.groups.contains_key(&into) {
        return Err(AppError::NotFound("famille"));
    }
    if id == into || foodflow_domain::foods::groups::ancestors(&catalog.parents, into).contains(&id) {
        return Err(DomainError::FoodGroupCycle.into());
    }
    db::groups::move_contents(&mut tx, id, into).await?;
    db::groups::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

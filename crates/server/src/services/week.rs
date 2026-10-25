//! Semaine type (PLA-02, PLA-07, PLA-08) : horaires et temps disponible de chaque créneau, composition type,
//! repas habituels.

use chrono::NaiveTime;
use foodflow_domain::foods::NutritionGroup;
use foodflow_domain::planning::meals::window_minutes;
use foodflow_domain::recipes::RecipeKind;
use foodflow_domain::shared::ids::{FoodId, HabitualMealId, MealSlotId, ProductId, RecipeId, SlotRoleId};
use foodflow_domain::{Amount, DomainError, Measure};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::catalog::Catalog;
use super::required;
use crate::db::planning::{HabitualItemRow, HabitualRow, RoleRow, ScheduleRow, Stocked};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Horaires d'un créneau un jour de la semaine (R-PLA-1, R-PLA-3, R-PLA-21).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DaySchedule {
    /// 1 = lundi … 7 = dimanche.
    pub weekday: u32,
    pub available_active_minutes: u32,
    /// « HH:MM ».
    #[schema(value_type = String, example = "18:30")]
    pub earliest_start: NaiveTime,
    #[schema(value_type = String, example = "19:30")]
    pub ready_by: NaiveTime,
    #[serde(default)]
    pub out_of_home: bool,
    pub habitual_meal_id: Option<HabitualMealId>,
}

/// Un rôle de la composition type (R-PLA-14).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SlotRole {
    /// Vide pour un nouveau rôle.
    pub id: Option<SlotRoleId>,
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub recipe_kinds: Vec<RecipeKind>,
    #[serde(default)]
    pub nutrition_groups: Vec<NutritionGroup>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SlotTemplate {
    pub slot_id: MealSlotId,
    pub name: String,
    pub days: Vec<DaySchedule>,
    pub roles: Vec<SlotRole>,
}

/// Semaine type des créneaux actifs.
pub async fn template(db: &Db) -> AppResult<Vec<SlotTemplate>> {
    let mut conn = db.read.acquire().await?;
    let slots = db::goals::slots(&mut *conn).await?;
    let schedules = db::planning::schedules(&mut *conn).await?;
    let roles = db::planning::roles(&mut *conn).await?;
    Ok(slots
        .into_iter()
        .filter(|s| s.active)
        .map(|slot| SlotTemplate {
            days: schedules
                .iter()
                .filter(|d| d.slot_id == slot.id)
                .map(|d| DaySchedule {
                    weekday: d.weekday,
                    available_active_minutes: d.available_active_minutes,
                    earliest_start: d.earliest_start,
                    ready_by: d.ready_by,
                    out_of_home: d.out_of_home,
                    habitual_meal_id: d.habitual_meal_id,
                })
                .collect(),
            roles: roles
                .iter()
                .filter(|r| r.slot_id == slot.id)
                .map(|r| SlotRole {
                    id: Some(r.id),
                    name: r.name.clone(),
                    required: r.required,
                    recipe_kinds: r.recipe_kinds.clone(),
                    nutrition_groups: r.nutrition_groups.clone(),
                })
                .collect(),
            slot_id: slot.id,
            name: slot.name,
        })
        .collect())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SlotTemplateInput {
    pub days: Vec<DaySchedule>,
    pub roles: Vec<SlotRole>,
}

/// Enregistre les horaires et la composition type d'un créneau. Les repas déjà créés gardent leurs valeurs
/// (R-PLA-5).
pub async fn save_template(db: &Db, slot: MealSlotId, input: &SlotTemplateInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    if !db::goals::slots(&mut *tx).await?.iter().any(|s| s.id == slot) {
        return Err(AppError::NotFound("créneau"));
    }
    let habituals = db::planning::habitual_meals(&mut tx).await?;
    for day in &input.days {
        if !(1..=7).contains(&day.weekday) {
            return Err(AppError::invalid_field("INVALID_WEEKDAY", "weekday", "jour de 1 (lundi) à 7 (dimanche)"));
        }
        window_minutes(day.earliest_start, day.ready_by)?;
        if day.habitual_meal_id.is_some_and(|h| !habituals.iter().any(|x| x.id == h)) {
            return Err(AppError::NotFound("repas habituel"));
        }
        let row = ScheduleRow {
            slot_id: slot,
            weekday: day.weekday,
            available_active_minutes: day.available_active_minutes,
            earliest_start: day.earliest_start,
            ready_by: day.ready_by,
            out_of_home: day.out_of_home,
            habitual_meal_id: day.habitual_meal_id,
        };
        db::planning::upsert_schedule(&mut *tx, &row).await?;
    }
    let roles = input
        .roles
        .iter()
        .enumerate()
        .map(|(position, role)| {
            Ok(RoleRow {
                id: role.id.unwrap_or_else(SlotRoleId::new),
                slot_id: slot,
                position: i64::try_from(position).map_err(anyhow::Error::from)?,
                name: required(&role.name, "name", 60)?,
                required: role.required,
                recipe_kinds: role.recipe_kinds.clone(),
                nutrition_groups: role.nutrition_groups.clone(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    db::planning::replace_roles(&mut tx, slot, &roles).await?;
    tx.commit().await?;
    Ok(())
}

// ===================================================================== Repas habituels (R-PLA-20)

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HabitualItem {
    pub food_id: Option<FoodId>,
    pub product_id: Option<ProductId>,
    pub recipe_id: Option<RecipeId>,
    /// Par équivalent adulte : dans la mesure de stock de l'aliment, en portions pour une recette.
    pub amount: Amount,
    /// Nom et mesure (lecture seule).
    #[serde(default)]
    pub name: String,
    pub measure: Option<Measure>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HabitualMeal {
    pub id: HabitualMealId,
    pub name: String,
    pub items: Vec<HabitualItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HabitualMealInput {
    pub name: String,
    pub items: Vec<HabitualItem>,
}

pub async fn habitual_meals(db: &Db) -> AppResult<Vec<HabitualMeal>> {
    let mut conn = db.read.acquire().await?;
    let rows = db::planning::habitual_meals(&mut conn).await?;
    let catalog = Catalog::load(&mut conn).await?;
    let recipes = db::recipes::list(&mut *conn).await?;
    Ok(rows
        .into_iter()
        .map(|h| HabitualMeal {
            id: h.id,
            name: h.name,
            items: h
                .items
                .iter()
                .map(|i| {
                    let (food_id, product_id) = match i.stocked {
                        Some(Stocked::Food(f)) => (Some(f), None),
                        Some(Stocked::Product(p)) => (None, Some(p)),
                        None => (None, None),
                    };
                    let (name, measure) = describe(&catalog, &recipes, i);
                    HabitualItem { food_id, product_id, recipe_id: i.recipe_id, amount: i.amount, name, measure }
                })
                .collect(),
        })
        .collect())
}

/// Nom et mesure d'un élément : celle de stock de l'aliment ou du produit, aucune pour une recette (portions).
pub fn describe(
    catalog: &Catalog,
    recipes: &[db::recipes::RecipeRow],
    item: &HabitualItemRow,
) -> (String, Option<Measure>) {
    match (item.stocked, item.recipe_id) {
        (Some(Stocked::Food(f)), _) => catalog
            .foods
            .get(&f)
            .map(|food| (food.name.clone(), Some(catalog.food_settings(food).stock_measure)))
            .unwrap_or_default(),
        (Some(Stocked::Product(p)), _) => catalog
            .products
            .get(&p)
            .map(|product| (product.name.clone(), Some(catalog.product_settings(product).stock_measure)))
            .unwrap_or_default(),
        (None, Some(r)) => (recipes.iter().find(|x| x.id == r).map(|x| x.name.clone()).unwrap_or_default(), None),
        (None, None) => (String::new(), None),
    }
}

pub async fn save_habitual(
    db: &Db,
    id: Option<HabitualMealId>,
    input: &HabitualMealInput,
) -> AppResult<HabitualMealId> {
    let name = required(&input.name, "name", 100)?;
    let mut tx = db.write.begin().await?;
    if db::planning::habitual_name_taken(&mut *tx, &name, id).await? {
        return Err(DomainError::HabitualMealNameTaken.into());
    }
    let items = input
        .items
        .iter()
        .enumerate()
        .map(|(position, item)| {
            let stocked = match (item.food_id, item.product_id, item.recipe_id) {
                (Some(f), None, None) => Some(Stocked::Food(f)),
                (None, Some(p), None) => Some(Stocked::Product(p)),
                (None, None, Some(_)) => None,
                _ => return Err(DomainError::MealItemContent.into()),
            };
            if !item.amount.is_positive() {
                return Err(DomainError::NotPositive.into());
            }
            Ok(HabitualItemRow {
                position: i64::try_from(position).map_err(anyhow::Error::from)?,
                stocked,
                recipe_id: item.recipe_id,
                amount: item.amount,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    let id = id.unwrap_or_else(HabitualMealId::new);
    db::planning::save_habitual(&mut tx, &HabitualRow { id, name, items }).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn delete_habitual(db: &Db, id: HabitualMealId) -> AppResult<()> {
    db::planning::delete_habitual(&db.write, id).await?;
    Ok(())
}

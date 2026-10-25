//! Nutriments et valeurs nutritionnelles des aliments et des produits (spec 01).

use foodflow_domain::foods::nutrients::{NutrientUnit, NutrientValue};
use foodflow_domain::shared::ids::FoodId;
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db};

#[derive(Debug, Clone)]
pub struct NutrientRow {
    pub code: String,
    pub name: String,
    pub storage_unit: NutrientUnit,
    pub ciqual_code: Option<String>,
    pub position: i64,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<NutrientRow>> {
    let rows = sqlx::query!("SELECT code, name, storage_unit, ciqual_code, position FROM nutrient ORDER BY position")
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(NutrientRow {
                code: r.code,
                name: r.name,
                storage_unit: enum_from_db(&r.storage_unit)?,
                ciqual_code: r.ciqual_code,
                position: r.position,
            })
        })
        .collect()
}

/// Données de référence : insère ou met à jour un nutriment (seed).
pub async fn upsert(db: impl SqliteExecutor<'_>, row: &NutrientRow) -> anyhow::Result<()> {
    let unit = enum_to_db(&row.storage_unit);
    sqlx::query!(
        "INSERT INTO nutrient (code, name, storage_unit, ciqual_code, position) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (code) DO UPDATE SET name = excluded.name, storage_unit = excluded.storage_unit,
             ciqual_code = excluded.ciqual_code, position = excluded.position",
        row.code,
        row.name,
        unit,
        row.ciqual_code,
        row.position
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FoodNutrientRow {
    pub code: String,
    pub value: NutrientValue,
    /// Valeur Ciqual remplacée par une saisie (R-ALI-11).
    pub reference_value: Option<i64>,
}

pub async fn food_values(db: impl SqliteExecutor<'_>, food: FoodId) -> anyhow::Result<Vec<FoodNutrientRow>> {
    let id = food.to_string();
    let rows = sqlx::query!(
        r#"SELECT nutrient_code, value, estimated AS "estimated: bool", source, reference_value
           FROM food_nutrient WHERE food_id = ?"#,
        id
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(FoodNutrientRow {
                code: r.nutrient_code,
                value: NutrientValue { value: r.value, estimated: r.estimated, source: enum_from_db(&r.source)? },
                reference_value: r.reference_value,
            })
        })
        .collect()
}

pub async fn set_food_value(
    db: impl SqliteExecutor<'_>,
    food: FoodId,
    code: &str,
    value: NutrientValue,
    reference_value: Option<i64>,
) -> anyhow::Result<()> {
    let id = food.to_string();
    let source = enum_to_db(&value.source);
    sqlx::query!(
        "INSERT INTO food_nutrient (food_id, nutrient_code, value, estimated, source, reference_value)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT (food_id, nutrient_code) DO UPDATE SET value = excluded.value, estimated = excluded.estimated,
             source = excluded.source, reference_value = excluded.reference_value",
        id,
        code,
        value.value,
        value.estimated,
        source,
        reference_value
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_food_value(db: impl SqliteExecutor<'_>, food: FoodId, code: &str) -> anyhow::Result<()> {
    let id = food.to_string();
    sqlx::query!("DELETE FROM food_nutrient WHERE food_id = ? AND nutrient_code = ?", id, code).execute(db).await?;
    Ok(())
}

/// Toutes les valeurs connues des aliments, pour les calculs de recettes.
/// Valeurs des seuls aliments dont les recettes ont besoin : ceux de leurs ingrédients, et les aliments de
/// référence des familles (ingrédient « au choix »). La table Ciqual compte quelque 25 000 valeurs : les
/// charger toutes à chaque recherche de recette la ralentit.
pub async fn recipe_food_values(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(FoodId, String, NutrientValue)>> {
    let rows = sqlx::query!(
        r#"SELECT food_id, nutrient_code, value, estimated AS "estimated: bool", source FROM food_nutrient
           WHERE food_id IN (SELECT food_id FROM recipe_ingredient WHERE food_id IS NOT NULL)
              OR food_id IN (SELECT reference_food_id FROM food_group WHERE reference_food_id IS NOT NULL)"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            let value = NutrientValue { value: r.value, estimated: r.estimated, source: enum_from_db(&r.source)? };
            Ok((super::codec::id_from_db(&r.food_id)?, r.nutrient_code, value))
        })
        .collect()
}

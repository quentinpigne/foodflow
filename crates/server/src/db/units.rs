//! Unités générales et unités propres à un aliment (spec 01, R-ALI-32).

use foodflow_domain::shared::ids::{FoodId, UnitId};
use foodflow_domain::{Amount, Measure};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct UnitRow {
    pub id: UnitId,
    pub food_id: Option<FoodId>,
    pub name: String,
    pub plural: Option<String>,
    pub abbreviation: Option<String>,
    pub measure: Measure,
    pub amount: Amount,
}

pub struct UnitFields<'a> {
    pub food_id: Option<FoodId>,
    pub name: &'a str,
    pub plural: Option<&'a str>,
    pub abbreviation: Option<&'a str>,
    pub measure: Measure,
    pub amount: Amount,
    pub position: i64,
}

/// Unités générales (sans aliment) et, si `food` est donné, celles de cet aliment.
pub async fn list(db: impl SqliteExecutor<'_>, food: Option<FoodId>) -> anyhow::Result<Vec<UnitRow>> {
    let food = food.map(|f| f.to_string());
    let rows = sqlx::query!(
        "SELECT id, food_id, name, plural, abbreviation, measure, amount, position FROM unit
         WHERE food_id IS NULL OR food_id = ? ORDER BY food_id IS NOT NULL, position, name",
        food
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(UnitRow {
                id: id_from_db(&r.id)?,
                food_id: opt_id_from_db(r.food_id.as_deref())?,
                name: r.name,
                plural: r.plural,
                abbreviation: r.abbreviation,
                measure: enum_from_db(&r.measure)?,
                amount: Amount::from_raw(r.amount),
            })
        })
        .collect()
}

pub async fn find_general(db: impl SqliteExecutor<'_>, name: &str) -> anyhow::Result<Option<UnitId>> {
    let id =
        sqlx::query_scalar!("SELECT id FROM unit WHERE food_id IS NULL AND name = ?", name).fetch_optional(db).await?;
    id.map(|id| id_from_db(&id)).transpose()
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: UnitId, f: &UnitFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let food = f.food_id.map(|x| x.to_string());
    let measure = enum_to_db(&f.measure);
    let amount = f.amount.raw();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO unit (id, food_id, name, plural, abbreviation, measure, amount, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        food,
        f.name,
        f.plural,
        f.abbreviation,
        measure,
        amount,
        f.position,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: UnitId, f: &UnitFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let measure = enum_to_db(&f.measure);
    let amount = f.amount.raw();
    let now = timestamp();
    sqlx::query!(
        "UPDATE unit SET name = ?, plural = ?, abbreviation = ?, measure = ?, amount = ?, position = ?, updated_at = ?
         WHERE id = ?",
        f.name,
        f.plural,
        f.abbreviation,
        measure,
        amount,
        f.position,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: UnitId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM unit WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Toutes les unités : générales et propres aux aliments.
pub async fn all(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<UnitRow>> {
    let rows = sqlx::query!(
        "SELECT id, food_id, name, plural, abbreviation, measure, amount FROM unit ORDER BY food_id IS NOT NULL, position, name"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(UnitRow {
                id: id_from_db(&r.id)?,
                food_id: opt_id_from_db(r.food_id.as_deref())?,
                name: r.name,
                plural: r.plural,
                abbreviation: r.abbreviation,
                measure: enum_from_db(&r.measure)?,
                amount: Amount::from_raw(r.amount),
            })
        })
        .collect()
}

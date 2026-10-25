//! Unités de saisie (spec 01, ALI-04).

use foodflow_domain::shared::ids::{FoodId, UnitId};
use foodflow_domain::{Amount, Measure};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::units::UnitRow;
use crate::db::{self, Db};
use crate::error::AppResult;

#[derive(Debug, Serialize, ToSchema)]
pub struct Unit {
    pub id: UnitId,
    /// `null` pour une unité générale.
    pub food_id: Option<FoodId>,
    pub name: String,
    pub plural: Option<String>,
    pub abbreviation: Option<String>,
    pub measure: Measure,
    /// Valeur d'une unité dans sa mesure, en unités de stockage (c. à soupe : 15 000 µL).
    pub amount: Amount,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UnitInput {
    pub name: String,
    pub plural: Option<String>,
    pub abbreviation: Option<String>,
    pub measure: Measure,
    pub amount: Amount,
    pub position: Option<i64>,
}

pub fn to_unit(row: UnitRow) -> Unit {
    Unit {
        id: row.id,
        food_id: row.food_id,
        name: row.name,
        plural: row.plural,
        abbreviation: row.abbreviation,
        measure: row.measure,
        amount: row.amount,
    }
}

/// Unités générales, et celles d'un aliment s'il est donné.
pub async fn list(db: &Db, food: Option<FoodId>) -> AppResult<Vec<Unit>> {
    Ok(db::units::list(&db.read, food).await?.into_iter().map(to_unit).collect())
}

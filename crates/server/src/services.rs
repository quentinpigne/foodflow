//! Services : un par cas d'usage. Ils ouvrent la transaction SQLite, chargent les données, appellent le
//! domaine et écrivent le résultat (architecture.md). Aucun calcul métier ici.

pub mod ai;
pub mod ai_planning;
pub mod assessment;
pub mod auth;
pub mod backup;
pub mod catalog;
pub mod ciqual;
pub mod dashboard;
pub mod demo;
pub mod export;
pub mod foods;
pub mod goals;
pub mod groups;
pub mod household;
pub mod images;
pub mod locations;
pub mod meal_validation;
pub mod meals;
pub mod off;
pub mod photos;
pub mod products;
pub mod recipe_import;
pub mod recipes;
pub mod seed;
pub mod settings;
pub mod stock;
pub mod units;
pub mod week;

use crate::error::{AppError, AppResult};

/// Texte saisi : espaces retirés, `None` s'il est vide.
pub fn clean(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

/// Texte obligatoire, borné en longueur.
pub fn required(value: &str, field: &'static str, max: usize) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::invalid_field("REQUIRED", field, "ce champ est obligatoire"));
    }
    if value.chars().count() > max {
        return Err(AppError::invalid_field("TOO_LONG", field, format!("{max} caractères au maximum")));
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod ai_tests;
#[cfg(test)]
mod matching_tests;
#[cfg(test)]
mod planning_tests;
#[cfg(test)]
mod recipe_tests;
#[cfg(test)]
mod tests;

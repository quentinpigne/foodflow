//! Services : un par cas d'usage. Ils ouvrent la transaction SQLite, chargent les données, appellent le
//! domaine et écrivent le résultat (architecture.md). Aucun calcul métier ici.

pub mod auth;
pub mod backup;
pub mod catalog;
pub mod ciqual;
pub mod dashboard;
pub mod demo;
pub mod export;
pub mod foods;
pub mod groups;
pub mod locations;
pub mod products;
pub mod seed;
pub mod settings;
pub mod stock;
pub mod units;

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
mod tests;

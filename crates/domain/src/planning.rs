//! Planification (spec 05) : foyer et convives, semaine type, repas et leurs éléments, restes prévus,
//! signalements, sorties de stock de la validation.

pub mod household;
pub mod meals;
pub mod signals;
pub mod validation;

use serde::{Deserialize, Serialize};

/// Statut d'un repas (spec 05, « Statuts d'un repas »).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MealStatus {
    /// Prévu : compté dans les besoins et le bilan prévu.
    Planned,
    /// Cuisiné et mangé, validé : mouvements de stock créés, bilan réel.
    Done,
    /// N'a pas eu lieu.
    Skipped,
    /// Pris hors du foyer : la cible du jour est réduite (R-NUT-31).
    OutOfHome,
}

/// Provenance d'un élément de repas (R-PLA-12). Les préparations d'une session (`PREPARED`) viendront avec
/// les sessions de préparation (PLA-10).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemSource {
    /// Recette cuisinée sur place, pour un nombre de portions.
    Cook,
    /// Part d'un lot de préparation ou de restes, ou d'un lot d'aliment de reste.
    Lot,
    /// Aliment (ou produit), en quantité.
    Food,
}

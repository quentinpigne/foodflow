//! Aliments (spec 01) : familles, aliments, produits, unités, conversions, valeurs nutritionnelles.

pub mod barcode;
pub mod conversion;
pub mod groups;
pub mod nutrients;

use serde::{Deserialize, Serialize};

/// Mode de suivi du stock d'un aliment (spec 02, ADR-0003).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrackingMode {
    /// Quantité exacte, dans la mesure de stock de l'aliment.
    Quantity,
    /// État seulement : disponible, bientôt fini, épuisé.
    Presence,
}

/// Base des valeurs nutritionnelles (glossaire).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NutrientBasis {
    #[serde(rename = "PER_100_G")]
    Per100G,
    #[serde(rename = "PER_100_ML")]
    Per100Ml,
}

/// Groupe des repères de consommation (spec 04), attaché aux familles.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NutritionGroup {
    Vegetables,
    Fruits,
    Legumes,
    WholeGrains,
    RefinedStarches,
    Dairy,
    Poultry,
    RedMeat,
    ProcessedMeat,
    Fish,
    FattyFish,
    Eggs,
    Nuts,
    AddedFats,
    SweetProducts,
    SaltySnacks,
    SugaryDrinks,
    Other,
}

/// Origine d'une valeur ou d'un élément du référentiel.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValueSource {
    Ciqual,
    Off,
    Manual,
    Computed,
}

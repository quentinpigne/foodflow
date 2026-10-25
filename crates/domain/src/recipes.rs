//! Recettes (spec 03) : rendement, ingrédients, sous-recettes, mise à l'échelle, valeurs calculées,
//! analyse des recettes importées.

pub mod durations;
pub mod graph;
pub mod matching;
pub mod parsing;
pub mod scaling;
pub mod values;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::foods::NutrientBasis;
use crate::foods::conversion::FoodConversions;
use crate::foods::nutrients::NutrientValue;
use crate::shared::ids::{FoodGroupId, FoodId, RecipeId};
use crate::shared::{Amount, DomainError, Measure};

/// Catégorie d'une recette.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecipeKind {
    Main,
    Side,
    Starter,
    Dessert,
    Breakfast,
    Snack,
    /// Préparation de base, souvent utilisée comme sous-recette.
    Base,
}

/// Ce que produit une recette (R-REC-10, R-REC-11).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Yield {
    /// Portions adulte, en millièmes.
    Portions { portions: Amount },
    /// Quantité dans une mesure (900 g de sauce).
    Amount { measure: Measure, amount: Amount },
}

impl Yield {
    pub fn check(&self) -> Result<(), DomainError> {
        let amount = match self {
            Yield::Portions { portions } => *portions,
            Yield::Amount { amount, .. } => *amount,
        };
        if amount.is_positive() { Ok(()) } else { Err(DomainError::InvalidYield) }
    }

    /// Quantité de rendement, dans l'unité où une sous-recette se demande (portions ou mesure).
    pub fn amount(&self) -> Amount {
        match self {
            Yield::Portions { portions } => *portions,
            Yield::Amount { amount, .. } => *amount,
        }
    }
}

/// Ce que vise un ingrédient (R-REC-20).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Target {
    Food(FoodId),
    Group(FoodGroupId),
    Recipe(RecipeId),
}

/// Un ingrédient tel que les calculs le voient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingredient {
    pub target: Target,
    /// Quantité dans la mesure de stock de la cible, ou dans l'unité de rendement d'une sous-recette ;
    /// `None` si l'ingrédient est à volonté ou si sa saisie n'est pas convertible (R-REC-21).
    pub amount: Option<Amount>,
    /// Mesure de `amount` : celle de stock de l'aliment ou de la famille, celle du rendement d'une
    /// sous-recette ; `None` pour des portions.
    pub measure: Option<Measure>,
    pub to_taste: bool,
    pub optional: bool,
    pub fixed: bool,
    /// Libellé pour dire ce qui manque (« 2 verres de farine »).
    pub label: String,
}

impl Ingredient {
    /// Quantité saisie mais non convertible : elle rend les totaux incomplets (R-REC-21).
    pub fn unconvertible(&self) -> bool {
        !self.to_taste && self.amount.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub id: RecipeId,
    pub name: String,
    pub yield_: Yield,
    pub ingredients: Vec<Ingredient>,
    pub active_minutes: u32,
    pub passive_minutes: u32,
}

/// Ce qu'il faut savoir d'un aliment pour les calculs : conversions, mesure, valeurs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoodFacts {
    pub name: String,
    pub stock_measure: Measure,
    pub conversions: FoodConversions,
    pub basis: NutrientBasis,
    pub nutrients: HashMap<String, NutrientValue>,
}

/// Recettes, aliments et familles disponibles pour les calculs, chargés par le service.
#[derive(Debug, Clone, Default)]
pub struct RecipeBook {
    pub recipes: HashMap<RecipeId, Recipe>,
    pub foods: HashMap<FoodId, FoodFacts>,
    /// Aliment de référence de chaque famille qui en a un (R-ALI-4).
    pub reference_foods: HashMap<FoodGroupId, FoodId>,
}

/// Vérifie la forme d'une recette (R-REC-1, R-REC-22) ; les cycles se vérifient sur le livre (R-REC-30).
pub fn check(recipe: &Recipe) -> Result<(), DomainError> {
    recipe.yield_.check()?;
    if recipe.ingredients.is_empty() {
        return Err(DomainError::RecipeWithoutIngredient);
    }
    if recipe.ingredients.iter().any(|i| i.to_taste && i.amount.is_some()) {
        return Err(DomainError::ToTasteWithQuantity);
    }
    Ok(())
}

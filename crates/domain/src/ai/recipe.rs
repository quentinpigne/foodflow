//! Recette proposée par l'IA (R-REC-82) : elle devient un brouillon, relu comme un import.

use serde::{Deserialize, Serialize};

use crate::recipes::RecipeKind;
use crate::shared::DomainError;

/// Une recette au format imposé à l'IA.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiRecipe {
    pub name: String,
    pub kind: RecipeKind,
    #[serde(default)]
    pub description: Option<String>,
    /// Nombre de portions adulte.
    pub portions: u32,
    pub active_minutes: u32,
    pub passive_minutes: u32,
    /// Lignes d'ingrédients, comme sur un site de recettes (« 200 g de farine »).
    pub ingredients: Vec<String>,
    pub steps: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn invalid(reason: &'static str) -> DomainError {
    DomainError::AiResponse(reason)
}

impl AiRecipe {
    /// Vérifie une recette reçue (R-REC-82).
    pub fn check(&self) -> Result<(), DomainError> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 200 {
            return Err(invalid("une recette a un nom de 1 à 200 caractères"));
        }
        if !(1..=50).contains(&self.portions) {
            return Err(invalid("le nombre de portions va de 1 à 50"));
        }
        if self.active_minutes > 24 * 60 || self.passive_minutes > 7 * 24 * 60 {
            return Err(invalid("temps de préparation invraisemblable"));
        }
        if !self.ingredients.iter().any(|i| !i.trim().is_empty()) {
            return Err(invalid("une recette a au moins un ingrédient"));
        }
        if !self.steps.iter().any(|s| !s.trim().is_empty()) {
            return Err(invalid("une recette a au moins une étape"));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn soup() -> AiRecipe {
        AiRecipe {
            name: "Soupe de potiron".into(),
            kind: RecipeKind::Main,
            description: None,
            portions: 4,
            active_minutes: 15,
            passive_minutes: 30,
            ingredients: vec!["800 g de potiron".into(), "1 oignon".into()],
            steps: vec!["Tout cuire, mixer.".into()],
            tags: vec![],
        }
    }

    /// Une réponse incomplète est refusée, avec sa raison (R-REC-82).
    #[test]
    fn r_rec_82_recipe_checks() {
        assert!(soup().check().is_ok());
        let nameless = AiRecipe { name: " ".into(), ..soup() };
        assert_eq!(nameless.check(), Err(DomainError::AiResponse("une recette a un nom de 1 à 200 caractères")));
        assert!(AiRecipe { portions: 0, ..soup() }.check().is_err());
        assert!(AiRecipe { ingredients: vec!["".into()], ..soup() }.check().is_err());
        assert!(AiRecipe { steps: vec![], ..soup() }.check().is_err());
    }
}

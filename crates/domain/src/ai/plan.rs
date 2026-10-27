//! Planning proposé par l'IA (ADR-0014, R-PLA-66, R-PLA-67) : des repas qui référencent des recettes et des
//! lots **par identifiant**, ou des recettes nouvelles ; le domaine écarte et signale ce qui ne va pas.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::recipe::AiRecipe;
use crate::shared::Date;
use crate::shared::ids::{LotId, MealSlotId, RecipeId};

/// Un élément de repas proposé : une recette connue, une recette nouvelle (rang dans `new_recipes`), ou une
/// part d'un lot (reste, préparation). Exactement un des trois.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiItem {
    #[serde(default)]
    pub recipe_id: Option<RecipeId>,
    #[serde(default)]
    pub new_recipe: Option<usize>,
    #[serde(default)]
    pub lot_id: Option<LotId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiMeal {
    pub date: Date,
    pub slot_id: MealSlotId,
    pub items: Vec<AiItem>,
    /// Justification courte, montrée avec la proposition.
    #[serde(default)]
    pub reason: String,
}

/// La réponse attendue de l'IA pour un planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiPlan {
    pub meals: Vec<AiMeal>,
    #[serde(default)]
    pub new_recipes: Vec<AiRecipe>,
}

/// Ce que le serveur sait de la période, pour contrôler la réponse.
#[derive(Debug, Clone, Default)]
pub struct PlanContext {
    /// Créneaux à remplir : (date, créneau).
    pub open: HashSet<(Date, MealSlotId)>,
    /// Recettes candidates, envoyées à l'IA.
    pub recipes: HashSet<RecipeId>,
    /// Lots à utiliser en priorité, envoyés à l'IA.
    pub lots: HashSet<LotId>,
    /// Recettes refusées par un convive présent, par créneau (R-PLA-50).
    pub refused: HashMap<(Date, MealSlotId), HashSet<RecipeId>>,
    /// Nombre maximal de recettes nouvelles (R-PLA-66).
    pub max_new_recipes: usize,
}

/// Un élément écarté, avec sa raison (R-PLA-67).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejection {
    pub date: Option<Date>,
    pub slot_id: Option<MealSlotId>,
    pub reason: String,
}

/// La réponse contrôlée : les repas retenus (avec leurs seuls éléments valides), les recettes nouvelles
/// retenues (rang d'origine, recette), ce qui a été écarté.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CheckedPlan {
    pub meals: Vec<AiMeal>,
    pub new_recipes: Vec<(usize, AiRecipe)>,
    pub rejected: Vec<Rejection>,
}

/// Contrôle un planning reçu (R-PLA-67) : créneau à remplir, une seule proposition par créneau, recettes et
/// lots connus, recettes refusées par un convive, recettes nouvelles valides et dans le plafond.
pub fn check_plan(plan: AiPlan, context: &PlanContext) -> CheckedPlan {
    let mut out = CheckedPlan::default();
    let mut reject = |date: Option<Date>, slot: Option<MealSlotId>, reason: String| {
        out.rejected.push(Rejection { date, slot_id: slot, reason });
    };
    // Recettes nouvelles valides, dans l'ordre, jusqu'au plafond : rang d'origine → retenue.
    let mut kept_new: Vec<(usize, AiRecipe)> = Vec::new();
    for (index, recipe) in plan.new_recipes.iter().enumerate() {
        if let Err(e) = recipe.check() {
            reject(None, None, format!("Recette nouvelle « {} » écartée : {e}", recipe.name.trim()));
        } else if kept_new.len() >= context.max_new_recipes {
            reject(None, None, format!("Recette nouvelle « {} » écartée : plafond atteint", recipe.name.trim()));
        } else {
            kept_new.push((index, recipe.clone()));
        }
    }
    let mut seen: HashSet<(Date, MealSlotId)> = HashSet::new();
    let mut used_new: HashSet<usize> = HashSet::new();
    let mut meals = Vec::new();
    for meal in plan.meals {
        let key = (meal.date, meal.slot_id);
        let (date, slot) = (Some(meal.date), Some(meal.slot_id));
        if !context.open.contains(&key) {
            reject(date, slot, "Créneau hors de la demande, ou déjà rempli".into());
            continue;
        }
        if !seen.insert(key) {
            reject(date, slot, "Deuxième proposition pour le même créneau".into());
            continue;
        }
        let mut items = Vec::new();
        for item in meal.items {
            let reason = match (item.recipe_id, item.new_recipe, item.lot_id) {
                (Some(r), None, None) if !context.recipes.contains(&r) => Some("Recette inconnue".to_owned()),
                (Some(r), None, None) if context.refused.get(&key).is_some_and(|set| set.contains(&r)) => {
                    Some("Recette refusée par un convive présent".to_owned())
                }
                (None, Some(n), None) if !kept_new.iter().any(|(i, _)| *i == n) => {
                    Some("Recette nouvelle absente ou écartée".to_owned())
                }
                (None, None, Some(l)) if !context.lots.contains(&l) => Some("Lot inconnu".to_owned()),
                (Some(_), None, None) | (None, Some(_), None) | (None, None, Some(_)) => None,
                _ => Some("Élément sans recette ni lot unique".to_owned()),
            };
            match reason {
                Some(reason) => reject(date, slot, reason),
                None => {
                    if let Some(n) = item.new_recipe {
                        used_new.insert(n);
                    }
                    items.push(item);
                }
            }
        }
        if items.is_empty() {
            reject(date, slot, "Aucun élément valide pour ce repas".into());
        } else {
            meals.push(AiMeal { items, ..meal });
        }
    }
    out.meals = meals;
    // Une recette nouvelle qu'aucun repas n'utilise n'a pas lieu d'être relue.
    out.new_recipes = kept_new.into_iter().filter(|(i, _)| used_new.contains(i)).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::recipe::tests::soup;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    /// Les éléments inconnus, refusés ou hors plafond sont écartés et signalés ; le reste est gardé
    /// (R-PLA-67, R-PLA-66).
    #[test]
    fn r_pla_67_invalid_items_are_rejected_not_fixed() {
        let (dinner, lunch) = (MealSlotId::new(), MealSlotId::new());
        let (gratin, lasagna, unknown) = (RecipeId::new(), RecipeId::new(), RecipeId::new());
        let leftovers = LotId::new();
        let monday = d("2026-11-02");
        let context = PlanContext {
            open: HashSet::from([(monday, dinner), (monday, lunch)]),
            recipes: HashSet::from([gratin, lasagna]),
            lots: HashSet::from([leftovers]),
            refused: HashMap::from([((monday, lunch), HashSet::from([lasagna]))]),
            max_new_recipes: 1,
        };
        let recipe = |id| AiItem { recipe_id: Some(id), new_recipe: None, lot_id: None };
        let fresh = |n| AiItem { recipe_id: None, new_recipe: Some(n), lot_id: None };
        let plan = AiPlan {
            meals: vec![
                AiMeal {
                    date: monday,
                    slot_id: dinner,
                    items: vec![recipe(gratin), recipe(unknown)],
                    reason: "".into(),
                },
                AiMeal { date: monday, slot_id: dinner, items: vec![recipe(gratin)], reason: "".into() },
                AiMeal {
                    date: monday,
                    slot_id: lunch,
                    items: vec![recipe(lasagna), fresh(0), fresh(1)],
                    reason: "".into(),
                },
                AiMeal { date: d("2026-11-09"), slot_id: lunch, items: vec![recipe(gratin)], reason: "".into() },
            ],
            new_recipes: vec![soup(), AiRecipe { name: "Velouté".into(), ..soup() }],
        };
        let checked = check_plan(plan, &context);
        assert_eq!(checked.meals.len(), 2);
        assert_eq!(checked.meals[0].items, vec![recipe(gratin)]);
        assert_eq!(checked.meals[1].items, vec![fresh(0)]);
        assert_eq!(checked.new_recipes.len(), 1, "plafond d'une recette nouvelle");
        let reasons: Vec<&str> = checked.rejected.iter().map(|r| r.reason.as_str()).collect();
        assert!(reasons.contains(&"Recette inconnue"));
        assert!(reasons.contains(&"Deuxième proposition pour le même créneau"));
        assert!(reasons.contains(&"Recette refusée par un convive présent"));
        assert!(reasons.contains(&"Recette nouvelle absente ou écartée"));
        assert!(reasons.contains(&"Créneau hors de la demande, ou déjà rempli"));
        assert!(reasons.iter().any(|r| r.contains("plafond atteint")));
    }

    #[test]
    fn a_meal_without_valid_item_is_dropped() {
        let dinner = MealSlotId::new();
        let monday = d("2026-11-02");
        let context = PlanContext { open: HashSet::from([(monday, dinner)]), ..Default::default() };
        let both = AiItem { recipe_id: Some(RecipeId::new()), new_recipe: Some(0), lot_id: None };
        let plan = AiPlan {
            meals: vec![AiMeal { date: monday, slot_id: dinner, items: vec![both], reason: "".into() }],
            new_recipes: vec![],
        };
        let checked = check_plan(plan, &context);
        assert!(checked.meals.is_empty());
        assert_eq!(checked.rejected.len(), 2);
    }
}

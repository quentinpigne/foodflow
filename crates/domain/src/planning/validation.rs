//! Sorties de stock proposées à la validation d'un repas (R-PLA-30, R-PLA-32).
//!
//! Le domaine développe les éléments cuisinés en besoins d'aliments et de familles, les additionne, puis
//! répartit chaque besoin sur les lots (R-STK-36) ; le serveur fournit les lots et écrit les mouvements.

use rust_decimal::Decimal;

use crate::recipes::graph::{Need, expand};
use crate::recipes::scaling::factor;
use crate::recipes::{RecipeBook, Target};
use crate::shared::ids::RecipeId;
use crate::shared::{Amount, DomainError, Measure};

/// Besoins d'une recette cuisinée pour `portions` (millièmes de portion, ou quantité de rendement pour une
/// recette en quantité), sous-recettes développées (R-REC-32).
pub fn cook_needs(book: &RecipeBook, recipe: RecipeId, portions: Amount) -> Result<Vec<Need>, DomainError> {
    let Some(found) = book.recipes.get(&recipe) else { return Ok(Vec::new()) };
    expand(book, recipe, factor(portions, &found.yield_)?)
}

/// Un besoin additionné : un aliment ou une famille, dans une mesure, en quantité exacte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Total {
    pub target: Target,
    pub measure: Measure,
    pub amount: Decimal,
}

/// Besoins additionnés par cible et par mesure, dans l'ordre de première apparition. Les ingrédients à
/// volonté et facultatifs ne sortent pas du stock ; ceux sans quantité convertible sont rendus à part, par
/// cible, pour être enregistrés comme utilisés sans sortie (R-PLA-32).
pub fn totals(needs: &[Need]) -> (Vec<Total>, Vec<Target>) {
    let mut out: Vec<Total> = Vec::new();
    let mut unquantified: Vec<Target> = Vec::new();
    for need in needs.iter().filter(|n| !n.to_taste && !n.optional) {
        match (need.amount, need.measure) {
            (Some(amount), Some(measure)) => {
                match out.iter_mut().find(|t| t.target == need.target && t.measure == measure) {
                    Some(total) => total.amount += amount,
                    None => out.push(Total { target: need.target, measure, amount }),
                }
            }
            _ => {
                if !unquantified.contains(&need.target) {
                    unquantified.push(need.target);
                }
            }
        }
    }
    (out, unquantified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::graph::tests::{ingredient, recipe};
    use crate::recipes::{Ingredient, Yield};
    use crate::shared::ids::FoodId;

    /// Valider un gratin cuisiné pour 6 portions (recette de 4) sort 1,5 fois ses ingrédients ; la même
    /// courgette demandée deux fois est additionnée (R-PLA-30).
    #[test]
    fn r_pla_30_needs_for_cooked_portions() {
        let zucchini = FoodId::new();
        let cheese = FoodId::new();
        let gratin = recipe(
            RecipeId::new(),
            Yield::Portions { portions: Amount::from_raw(4_000) },
            [(zucchini, 800_000), (cheese, 100_000), (zucchini, 200_000)]
                .into_iter()
                .map(|(food, amount)| Ingredient {
                    measure: Some(Measure::Mass),
                    ..ingredient(Target::Food(food), amount)
                })
                .collect(),
        );
        let mut book = RecipeBook::default();
        let id = gratin.id;
        book.recipes.insert(id, gratin);
        let needs = cook_needs(&book, id, Amount::from_raw(6_000)).unwrap();
        let (totals, unquantified) = totals(&needs);
        assert!(unquantified.is_empty());
        assert_eq!(
            totals.iter().map(|t| (t.target, t.amount)).collect::<Vec<_>>(),
            vec![(Target::Food(zucchini), Decimal::from(1_500_000)), (Target::Food(cheese), Decimal::from(150_000))]
        );
    }

    /// Un ingrédient sans quantité convertible est rendu à part : utilisé, sans sortie de stock (R-PLA-32).
    #[test]
    fn r_pla_32_unquantified() {
        let salt = FoodId::new();
        let need = Need { target: Target::Food(salt), amount: None, measure: None, to_taste: false, optional: false };
        let pepper = Need { to_taste: true, ..need.clone() };
        assert_eq!(totals(&[need, pepper]), (vec![], vec![Target::Food(salt)]));
    }
}

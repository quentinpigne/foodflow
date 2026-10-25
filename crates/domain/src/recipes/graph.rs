//! Sous-recettes : absence de cycle (R-REC-30) et développement en aliments (R-REC-32).

use std::collections::{HashMap, HashSet};

use rust_decimal::Decimal;

use crate::shared::DomainError;
use crate::shared::ids::RecipeId;

use super::scaling::{Factor, scaled};
use super::{RecipeBook, Target};

/// Sous-recettes directes de chaque recette.
fn children(book: &RecipeBook) -> HashMap<RecipeId, Vec<RecipeId>> {
    book.recipes
        .values()
        .map(|r| {
            let subs = r
                .ingredients
                .iter()
                .filter_map(|i| match i.target {
                    Target::Recipe(id) => Some(id),
                    _ => None,
                })
                .collect();
            (r.id, subs)
        })
        .collect()
}

/// Vérifie qu'aucune recette du livre n'est, directement ou non, son propre ingrédient (R-REC-30).
pub fn check_cycles(book: &RecipeBook) -> Result<(), DomainError> {
    let graph = children(book);
    // Parcours en profondeur avec trois couleurs : en cours, terminé.
    let mut done: HashSet<RecipeId> = HashSet::new();
    let mut ids: Vec<RecipeId> = graph.keys().copied().collect();
    ids.sort();
    for start in ids {
        let mut on_path: HashSet<RecipeId> = HashSet::new();
        visit(start, &graph, &mut on_path, &mut done)?;
    }
    Ok(())
}

fn visit(
    id: RecipeId,
    graph: &HashMap<RecipeId, Vec<RecipeId>>,
    on_path: &mut HashSet<RecipeId>,
    done: &mut HashSet<RecipeId>,
) -> Result<(), DomainError> {
    if done.contains(&id) {
        return Ok(());
    }
    if !on_path.insert(id) {
        return Err(DomainError::RecipeCycle);
    }
    for child in graph.get(&id).into_iter().flatten() {
        visit(*child, graph, on_path, done)?;
    }
    on_path.remove(&id);
    done.insert(id);
    Ok(())
}

/// Recettes qui utilisent `id`, directement ou non (pour refuser un cycle avant d'enregistrer).
pub fn ancestors(book: &RecipeBook, id: RecipeId) -> HashSet<RecipeId> {
    let graph = children(book);
    let mut found = HashSet::new();
    let mut frontier = vec![id];
    while let Some(current) = frontier.pop() {
        for (parent, subs) in &graph {
            if subs.contains(&current) && found.insert(*parent) {
                frontier.push(*parent);
            }
        }
    }
    found
}

/// Un besoin développé : un aliment ou une famille, avec sa quantité exacte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    pub target: Target,
    /// `None` : à volonté, ou quantité non convertible.
    pub amount: Option<Decimal>,
    /// Mesure de `amount` (celle de stock de l'aliment ou de la famille).
    pub measure: Option<crate::shared::Measure>,
    pub to_taste: bool,
    pub optional: bool,
}

/// Développe une recette, mise à l'échelle par `factor`, en ses besoins d'aliments et de familles
/// (R-REC-32). Les sous-recettes sont développées à leur tour, au prorata de la quantité demandée.
pub fn expand(book: &RecipeBook, id: RecipeId, factor: Factor) -> Result<Vec<Need>, DomainError> {
    let mut out = Vec::new();
    expand_into(book, id, factor, &mut Vec::new(), &mut out)?;
    Ok(out)
}

fn expand_into(
    book: &RecipeBook,
    id: RecipeId,
    factor: Factor,
    path: &mut Vec<RecipeId>,
    out: &mut Vec<Need>,
) -> Result<(), DomainError> {
    if path.contains(&id) {
        return Err(DomainError::RecipeCycle);
    }
    let Some(recipe) = book.recipes.get(&id) else { return Ok(()) };
    path.push(id);
    for ingredient in &recipe.ingredients {
        let amount = scaled(ingredient, factor);
        let sub = match ingredient.target {
            Target::Recipe(sub) => book.recipes.get(&sub).zip(amount),
            _ => None,
        };
        match sub {
            Some((sub_recipe, amount)) => {
                let sub_factor = Factor::new(amount, sub_recipe.yield_.amount().to_decimal())?;
                expand_into(book, sub_recipe.id, sub_factor, path, out)?;
            }
            None => out.push(Need {
                target: ingredient.target,
                amount,
                measure: ingredient.measure,
                to_taste: ingredient.to_taste,
                optional: ingredient.optional,
            }),
        }
    }
    path.pop();
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::recipes::{Ingredient, Recipe, Yield};
    use crate::shared::Amount;
    use crate::shared::ids::FoodId;

    pub fn ingredient(target: Target, amount: i64) -> Ingredient {
        Ingredient {
            target,
            amount: Some(Amount::from_raw(amount)),
            measure: None,
            to_taste: false,
            optional: false,
            fixed: false,
            label: String::new(),
        }
    }

    pub fn recipe(id: RecipeId, yield_: Yield, ingredients: Vec<Ingredient>) -> Recipe {
        Recipe { id, name: String::new(), yield_, ingredients, active_minutes: 0, passive_minutes: 0 }
    }

    #[test]
    fn r_rec_30_cycles_are_detected() {
        let (a, b) = (RecipeId::new(), RecipeId::new());
        let portions = Yield::Portions { portions: Amount::from_raw(4_000) };
        let mut book = RecipeBook::default();
        book.recipes.insert(a, recipe(a, portions, vec![ingredient(Target::Recipe(b), 1_000)]));
        book.recipes.insert(b, recipe(b, portions, vec![ingredient(Target::Food(FoodId::new()), 1_000)]));
        assert!(check_cycles(&book).is_ok());
        assert_eq!(ancestors(&book, b).into_iter().collect::<Vec<_>>(), vec![a]);
        // Critère d'acceptation : B qui utiliserait A forme un cycle.
        book.recipes.get_mut(&b).unwrap().ingredients.push(ingredient(Target::Recipe(a), 1_000));
        assert_eq!(check_cycles(&book), Err(DomainError::RecipeCycle));
    }

    /// Critère d'acceptation : des lasagnes qui utilisent 900 g d'une sauce de 1 800 g comptent la moitié de
    /// ses ingrédients.
    #[test]
    fn r_rec_32_sub_recipes_expand_pro_rata() {
        let (lasagna, sauce) = (RecipeId::new(), RecipeId::new());
        let (beef, pasta) = (FoodId::new(), FoodId::new());
        let mut book = RecipeBook::default();
        book.recipes.insert(
            sauce,
            recipe(
                sauce,
                Yield::Amount { measure: crate::shared::Measure::Mass, amount: Amount::from_raw(1_800_000) },
                vec![ingredient(Target::Food(beef), 500_000)],
            ),
        );
        book.recipes.insert(
            lasagna,
            recipe(
                lasagna,
                Yield::Portions { portions: Amount::from_raw(6_000) },
                vec![ingredient(Target::Recipe(sauce), 900_000), ingredient(Target::Food(pasta), 240_000)],
            ),
        );
        let needs = expand(&book, lasagna, Factor::ONE).unwrap();
        assert_eq!(
            needs[0],
            Need {
                target: Target::Food(beef),
                amount: Some(Decimal::from(250_000)),
                measure: None,
                to_taste: false,
                optional: false
            }
        );
        assert_eq!(needs[1].amount, Some(Decimal::from(240_000)));
    }
}

/// Temps d'une recette cuisinée sur place avec ses sous-recettes (R-REC-41) : (temps actif total, temps total
/// du planning pour un cuisinier, sans équipement).
pub fn times(book: &RecipeBook, id: RecipeId) -> Result<(u32, u32), DomainError> {
    use crate::scheduling::{Task, schedule};
    // Une tâche par recette de l'arbre, sous-recettes d'abord.
    let mut order: Vec<RecipeId> = Vec::new();
    collect(book, id, &mut Vec::new(), &mut order)?;
    let index: HashMap<RecipeId, usize> = order.iter().enumerate().map(|(i, r)| (*r, i)).collect();
    let tasks: Vec<Task> = order
        .iter()
        .filter_map(|r| book.recipes.get(r))
        .map(|r| {
            let after = r
                .ingredients
                .iter()
                .filter_map(|i| match i.target {
                    Target::Recipe(sub) => index.get(&sub).copied(),
                    _ => None,
                })
                .collect();
            Task::simple(r.active_minutes, r.passive_minutes, after)
        })
        .collect();
    let active = order.iter().filter_map(|r| book.recipes.get(r)).map(|r| r.active_minutes).sum();
    Ok((active, schedule(&tasks, 1, &[]).makespan))
}

fn collect(
    book: &RecipeBook,
    id: RecipeId,
    path: &mut Vec<RecipeId>,
    order: &mut Vec<RecipeId>,
) -> Result<(), DomainError> {
    if path.contains(&id) {
        return Err(DomainError::RecipeCycle);
    }
    if order.contains(&id) {
        return Ok(());
    }
    let Some(recipe) = book.recipes.get(&id) else { return Ok(()) };
    path.push(id);
    for ingredient in &recipe.ingredients {
        if let Target::Recipe(sub) = ingredient.target {
            collect(book, sub, path, order)?;
        }
    }
    path.pop();
    order.push(id);
    Ok(())
}

#[cfg(test)]
mod time_tests {
    use super::tests::{ingredient, recipe};
    use super::*;
    use crate::recipes::Yield;
    use crate::shared::Amount;

    /// Critère d'acceptation : le temps actif des lasagnes ajoute celui de la sauce ; la sauce finit avant.
    #[test]
    fn r_rec_41_times_with_sub_recipes() {
        let (lasagna, sauce) = (RecipeId::new(), RecipeId::new());
        let mut book = RecipeBook::default();
        let mut s = recipe(sauce, Yield::Portions { portions: Amount::from_raw(6_000) }, vec![]);
        (s.active_minutes, s.passive_minutes) = (20, 90);
        let mut l = recipe(
            lasagna,
            Yield::Portions { portions: Amount::from_raw(6_000) },
            vec![ingredient(Target::Recipe(sauce), 6_000)],
        );
        (l.active_minutes, l.passive_minutes) = (40, 50);
        book.recipes.insert(sauce, s);
        book.recipes.insert(lasagna, l);
        assert_eq!(times(&book, lasagna).unwrap(), (60, 200));
    }
}

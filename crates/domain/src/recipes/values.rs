//! Valeurs nutritionnelles calculées d'une recette (R-REC-50 à R-REC-52, R-REC-24).
//!
//! Calculées à chaque lecture, jamais stockées (R-REC-54). Un total qui inclut une valeur inconnue ou une
//! quantité non convertible est **incomplet** et dit ce qui manque (R-ALI-20).

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::foods::NutrientBasis;
use crate::foods::conversion::convert_exact;
use crate::shared::ids::{FoodId, RecipeId};
use crate::shared::{DomainError, Measure};

use super::scaling::Factor;
use super::{Ingredient, RecipeBook, Target, Yield};

/// Quantité de référence d'une base nutritionnelle : 100 g = 100 000 mg, 100 mL = 100 000 µL.
const HUNDRED: i64 = 100_000;

/// Total d'un nutriment, dans son unité de stockage (cal, mg ou µg).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NutrientTotal {
    pub code: String,
    pub value: Decimal,
    /// Une composante au moins est estimée (Ciqual « traces », aliment de référence d'une famille…).
    pub estimated: bool,
    /// Libellés des ingrédients sans valeur ou sans quantité convertible : le total est incomplet s'il y en a.
    pub missing: Vec<String>,
}

impl NutrientTotal {
    pub fn complete(&self) -> bool {
        self.missing.is_empty()
    }
}

/// Totaux d'une recette entière, nutriment par nutriment, dans l'ordre des codes demandés.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Totals {
    pub nutrients: BTreeMap<String, NutrientTotal>,
}

impl Totals {
    fn empty(codes: &[&str]) -> Self {
        let nutrients = codes
            .iter()
            .map(|c| {
                (
                    (*c).to_owned(),
                    NutrientTotal { code: (*c).to_owned(), value: Decimal::ZERO, estimated: false, missing: vec![] },
                )
            })
            .collect();
        Self { nutrients }
    }

    /// Applique un facteur à tous les totaux (part d'une sous-recette, valeurs par portion).
    pub fn scaled(&self, factor: Factor) -> Self {
        let mut out = self.clone();
        for total in out.nutrients.values_mut() {
            total.value = factor.apply(total.value);
        }
        out
    }

    fn missing_everywhere(&mut self, label: &str) {
        for total in self.nutrients.values_mut() {
            if !total.missing.iter().any(|m| m == label) {
                total.missing.push(label.to_owned());
            }
        }
    }

    fn add(&mut self, other: &Totals) {
        for (code, part) in &other.nutrients {
            if let Some(total) = self.nutrients.get_mut(code) {
                total.value += part.value;
                total.estimated |= part.estimated;
                for m in &part.missing {
                    if !total.missing.contains(m) {
                        total.missing.push(m.clone());
                    }
                }
            }
        }
    }
}

/// Totaux d'une recette entière pour les nutriments `codes` (R-REC-50).
pub fn totals(book: &RecipeBook, id: RecipeId, codes: &[&str]) -> Result<Totals, DomainError> {
    totals_inner(book, id, codes, &mut Vec::new())
}

fn totals_inner(
    book: &RecipeBook,
    id: RecipeId,
    codes: &[&str],
    path: &mut Vec<RecipeId>,
) -> Result<Totals, DomainError> {
    if path.contains(&id) {
        return Err(DomainError::RecipeCycle);
    }
    let mut out = Totals::empty(codes);
    let Some(recipe) = book.recipes.get(&id) else { return Ok(out) };
    path.push(id);
    for ingredient in &recipe.ingredients {
        // À volonté : ne compte pas (R-REC-22) ; facultatif : compté seulement s'il est demandé, donc pas ici.
        if ingredient.to_taste || ingredient.optional {
            continue;
        }
        let Some(amount) = ingredient.amount else {
            out.missing_everywhere(&ingredient.label);
            continue;
        };
        match ingredient.target {
            Target::Recipe(sub) => match book.recipes.get(&sub) {
                Some(sub_recipe) => {
                    let part = totals_inner(book, sub, codes, path)?;
                    out.add(&part.scaled(Factor::new(amount.to_decimal(), sub_recipe.yield_.amount().to_decimal())?));
                }
                None => out.missing_everywhere(&ingredient.label),
            },
            Target::Food(food) => out.add(&food_part(book, food, ingredient, amount.to_decimal(), codes, false)),
            // Une famille compte pour son aliment de référence, en valeurs estimées (R-REC-24).
            Target::Group(group) => match book.reference_foods.get(&group) {
                Some(food) => out.add(&food_part(book, *food, ingredient, amount.to_decimal(), codes, true)),
                None => out.missing_everywhere(&ingredient.label),
            },
        }
    }
    path.pop();
    Ok(out)
}

/// Contribution d'un aliment : quantité dans sa base (g ou mL) × valeur pour 100 ÷ 100 (R-REC-50, R-ALI-23).
fn food_part(
    book: &RecipeBook,
    food: FoodId,
    ingredient: &Ingredient,
    amount: Decimal,
    codes: &[&str],
    estimated: bool,
) -> Totals {
    let mut out = Totals::empty(codes);
    let Some(facts) = book.foods.get(&food) else {
        out.missing_everywhere(&ingredient.label);
        return out;
    };
    let from = ingredient.measure.unwrap_or(facts.stock_measure);
    let to = match facts.basis {
        NutrientBasis::Per100G => Measure::Mass,
        NutrientBasis::Per100Ml => Measure::Volume,
    };
    let Ok(quantity) = convert_exact(amount, from, to, facts.conversions) else {
        out.missing_everywhere(&ingredient.label);
        return out;
    };
    for (code, total) in out.nutrients.iter_mut() {
        match facts.nutrients.get(code) {
            Some(value) => {
                total.value = quantity * Decimal::from(value.value) / Decimal::from(HUNDRED);
                total.estimated = estimated || value.estimated;
            }
            None => total.missing.push(ingredient.label.clone()),
        }
    }
    out
}

/// Facteur pour passer des totaux de la recette à une portion adulte, ou à 100 g ou 100 mL de préparation
/// (R-REC-51). Un rendement en pièces donne les valeurs d'une pièce.
pub fn per_serving_factor(yield_: &Yield) -> Result<Factor, DomainError> {
    yield_.check()?;
    match yield_ {
        Yield::Portions { portions } => Factor::new(Decimal::from(1_000), portions.to_decimal()),
        Yield::Amount { measure: Measure::Count, amount } => Factor::new(Decimal::from(1_000), amount.to_decimal()),
        Yield::Amount { amount, .. } => Factor::new(Decimal::from(HUNDRED), amount.to_decimal()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::foods::ValueSource;
    use crate::foods::conversion::FoodConversions;
    use crate::foods::nutrients::NutrientValue;
    use crate::recipes::FoodFacts;
    use crate::recipes::graph::tests::{ingredient, recipe};
    use crate::shared::Amount;
    use crate::shared::ids::FoodGroupId;

    fn facts(name: &str, measure: Measure, conversions: FoodConversions, values: &[(&str, i64)]) -> FoodFacts {
        FoodFacts {
            name: name.into(),
            stock_measure: measure,
            conversions,
            basis: NutrientBasis::Per100G,
            nutrients: values
                .iter()
                .map(|(c, v)| {
                    ((*c).to_owned(), NutrientValue { value: *v, estimated: false, source: ValueSource::Ciqual })
                })
                .collect(),
        }
    }

    fn labelled(mut i: Ingredient, label: &str, measure: Measure) -> Ingredient {
        i.label = label.into();
        i.measure = Some(measure);
        i
    }

    /// Lait en volume (densité 1,03), farine en masse : énergie et fibres, avec une valeur inconnue.
    #[test]
    fn r_rec_50_and_52_totals_and_incompleteness() {
        let (milk, flour) = (FoodId::new(), FoodId::new());
        let id = RecipeId::new();
        let mut book = RecipeBook::default();
        book.foods.insert(
            milk,
            facts(
                "Lait",
                Measure::Volume,
                FoodConversions { density_mg_per_ml: Some(1_030), piece_weight_mg: None },
                &[("ENERGY", 46_000)],
            ),
        );
        book.foods.insert(
            flour,
            facts("Farine", Measure::Mass, FoodConversions::default(), &[("ENERGY", 350_000), ("FIBER", 3_000)]),
        );
        book.recipes.insert(
            id,
            recipe(
                id,
                Yield::Portions { portions: Amount::from_raw(4_000) },
                vec![
                    labelled(ingredient(Target::Food(milk), 500_000), "50 cL de lait", Measure::Volume),
                    labelled(ingredient(Target::Food(flour), 250_000), "250 g de farine", Measure::Mass),
                ],
            ),
        );
        let t = totals(&book, id, &["ENERGY", "FIBER"]).unwrap();
        // 500 mL × 1,03 = 515 g de lait → 236,9 kcal ; 250 g de farine → 875 kcal.
        assert_eq!(t.nutrients["ENERGY"].value, Decimal::from(1_111_900));
        assert!(t.nutrients["ENERGY"].complete());
        // Les fibres du lait sont inconnues : jamais zéro, le total est incomplet (R-ALI-20).
        assert_eq!(t.nutrients["FIBER"].value, Decimal::from(7_500));
        assert_eq!(t.nutrients["FIBER"].missing, vec!["50 cL de lait".to_owned()]);
        let per = t.scaled(per_serving_factor(&Yield::Portions { portions: Amount::from_raw(4_000) }).unwrap());
        assert_eq!(per.nutrients["ENERGY"].value, Decimal::from(277_975));
    }

    /// Critère d'acceptation : « 2 verres de farine » sans densité rend les totaux incomplets (R-REC-21).
    #[test]
    fn r_rec_21_unconvertible_quantity() {
        let flour = FoodId::new();
        let id = RecipeId::new();
        let mut book = RecipeBook::default();
        book.foods.insert(flour, facts("Farine", Measure::Mass, FoodConversions::default(), &[("ENERGY", 350_000)]));
        let glasses = labelled(ingredient(Target::Food(flour), 400_000), "2 verres de farine", Measure::Volume);
        book.recipes.insert(id, recipe(id, Yield::Portions { portions: Amount::from_raw(4_000) }, vec![glasses]));
        let t = totals(&book, id, &["ENERGY"]).unwrap();
        assert_eq!(t.nutrients["ENERGY"].missing, vec!["2 verres de farine".to_owned()]);
    }

    #[test]
    fn r_rec_24_group_uses_reference_food_estimated() {
        let (emmental, group) = (FoodId::new(), FoodGroupId::new());
        let id = RecipeId::new();
        let mut book = RecipeBook::default();
        book.foods
            .insert(emmental, facts("Emmental", Measure::Mass, FoodConversions::default(), &[("ENERGY", 377_000)]));
        book.recipes.insert(
            id,
            recipe(
                id,
                Yield::Portions { portions: Amount::from_raw(1_000) },
                vec![labelled(ingredient(Target::Group(group), 100_000), "100 g de fromage râpé", Measure::Mass)],
            ),
        );
        let without = totals(&book, id, &["ENERGY"]).unwrap();
        assert!(!without.nutrients["ENERGY"].complete(), "sans aliment de référence, la valeur est inconnue");
        book.reference_foods = HashMap::from([(group, emmental)]);
        let with = totals(&book, id, &["ENERGY"]).unwrap();
        assert_eq!(with.nutrients["ENERGY"].value, Decimal::from(377_000));
        assert!(with.nutrients["ENERGY"].estimated);
    }

    #[test]
    fn sub_recipe_contributes_pro_rata_and_to_taste_does_not_count() {
        let (beef, salt) = (FoodId::new(), FoodId::new());
        let (sauce, lasagna) = (RecipeId::new(), RecipeId::new());
        let mut book = RecipeBook::default();
        book.foods.insert(beef, facts("Bœuf", Measure::Mass, FoodConversions::default(), &[("ENERGY", 200_000)]));
        book.foods.insert(salt, facts("Sel", Measure::Mass, FoodConversions::default(), &[]));
        book.recipes.insert(
            sauce,
            recipe(
                sauce,
                Yield::Amount { measure: Measure::Mass, amount: Amount::from_raw(1_000_000) },
                vec![labelled(ingredient(Target::Food(beef), 500_000), "bœuf", Measure::Mass)],
            ),
        );
        let mut pinch = labelled(ingredient(Target::Food(salt), 0), "sel", Measure::Mass);
        (pinch.amount, pinch.to_taste) = (None, true);
        book.recipes.insert(
            lasagna,
            recipe(
                lasagna,
                Yield::Portions { portions: Amount::from_raw(4_000) },
                vec![labelled(ingredient(Target::Recipe(sauce), 500_000), "sauce", Measure::Mass), pinch],
            ),
        );
        let t = totals(&book, lasagna, &["ENERGY"]).unwrap();
        // La moitié de la sauce : 250 g de bœuf → 500 kcal ; le sel à volonté ne compte pas (R-REC-22).
        assert_eq!(t.nutrients["ENERGY"].value, Decimal::from(500_000));
        assert!(t.nutrients["ENERGY"].complete());
    }
}

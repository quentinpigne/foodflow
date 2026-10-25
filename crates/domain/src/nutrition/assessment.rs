//! Bilan nutritionnel d'une période (R-NUT-20 à R-NUT-44) : pour chaque critère, la valeur, la cible et un
//! statut. Le bilan compare des apports **par équivalent adulte** (R-NUT-21) ; il n'a pas de score global
//! (R-NUT-44).

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::Serialize;

use crate::foods::NutritionGroup;
use crate::recipes::values::Totals;
use crate::shared::Date;
use crate::shared::Ratio;
use crate::shared::ids::{FoodGroupId, MealSlotId, RecipeId};

use super::{Criterion, CriterionKind, Expression, Period};

/// Énergie d'un gramme de nutriment, en kcal (R-NUT-11) : `None` si le nutriment ne s'exprime pas en part de
/// l'énergie.
pub fn kcal_per_gram(code: &str) -> Option<i64> {
    match code {
        "FAT" | "SATURATED_FAT" => Some(9),
        "PROTEIN" | "CARBOHYDRATE" | "SUGARS" => Some(4),
        "FIBER" => Some(2),
        _ => None,
    }
}

/// Ce qu'un repas apporte, par équivalent adulte (R-NUT-20, R-NUT-21).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MealIntake {
    pub slot: Option<MealSlotId>,
    /// Nutriments, dans leur unité de stockage (cal, mg), avec leurs manques (R-NUT-24).
    pub totals: Totals,
    /// Portions de chaque groupe nutritionnel, en millièmes de portion (R-NUT-22).
    pub groups: BTreeMap<NutritionGroup, Decimal>,
    /// Quantité de chaque groupe nutritionnel, en mg.
    pub group_amounts: BTreeMap<NutritionGroup, Decimal>,
    /// Portions de chaque famille d'aliments, familles parentes comprises, en millièmes de portion (R-NUT-15).
    pub families: BTreeMap<FoodGroupId, Decimal>,
    /// Recettes cuisinées à ce repas : les restes mangés n'en font pas partie (R-NUT-15, ADR-0012).
    pub cooked_recipes: Vec<RecipeId>,
}

/// Une journée du bilan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub date: Date,
    /// Repas comptés : prévus pour le bilan prévu, validés pour le bilan réel (R-NUT-41).
    pub meals: Vec<MealIntake>,
    /// Part de l'énergie de la journée des créneaux pris hors foyer, en millièmes (R-NUT-31).
    pub out_of_home_share: Ratio,
    /// Libellés des créneaux actifs sans repas : la journée est incomplète (R-NUT-32).
    pub empty_slots: Vec<String>,
}

/// Statut d'un critère sur une période (R-NUT-40).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Met,
    Below,
    Above,
    /// Total incomplet : la valeur est partielle (R-NUT-24).
    Incomplete,
    /// Aucun repas compté.
    NotApplicable,
}

/// Bilan d'un critère : valeur, bornes effectives, statut et ce qui manque.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Assessment {
    /// Unité du critère (celle de `min_value`, `max_value`) ; `None` sans repas compté ou sans répétition.
    pub value: Option<Decimal>,
    /// Bornes effectives : tolérance appliquée, cible réduite des repas hors foyer.
    pub min: Option<Decimal>,
    pub max: Option<Decimal>,
    pub status: Status,
    pub missing: Vec<String>,
}

fn status(value: Decimal, min: Option<Decimal>, max: Option<Decimal>, incomplete: bool) -> Status {
    // Un total incomplet n'est jamais atteint, sauf un maximum déjà dépassé par la valeur partielle (R-NUT-24).
    if max.is_some_and(|m| value > m) {
        return Status::Above;
    }
    if incomplete {
        return Status::Incomplete;
    }
    if min.is_some_and(|m| value < m) {
        return Status::Below;
    }
    Status::Met
}

fn merge(into: &mut Vec<String>, more: impl IntoIterator<Item = String>) {
    for m in more {
        if !into.contains(&m) {
            into.push(m);
        }
    }
}

/// Valeur d'un critère journalier sur une journée, et ce qui la rend incomplète.
fn day_value(criterion: &Criterion, day: &Day) -> (Decimal, Vec<String>) {
    let mut missing: Vec<String> = day.empty_slots.iter().map(|s| format!("{s} : aucun repas")).collect();
    let mut nutrient = |code: &str| -> Decimal {
        let mut sum = Decimal::ZERO;
        for meal in &day.meals {
            if let Some(total) = meal.totals.nutrients.get(code) {
                sum += total.value;
                merge(&mut missing, total.missing.iter().cloned());
            }
        }
        sum
    };
    let value = match criterion.kind {
        CriterionKind::Energy | CriterionKind::Nutrient => {
            let code = criterion.nutrient_code.as_deref().unwrap_or("ENERGY");
            let amount = nutrient(code);
            match (criterion.expression, kcal_per_gram(code)) {
                // Part de l'énergie, en millièmes : mg × kcal/g = cal (R-NUT-11).
                (Expression::PercentEnergy, Some(kcal)) => {
                    let energy = nutrient("ENERGY");
                    if energy.is_zero() {
                        Decimal::ZERO
                    } else {
                        amount * Decimal::from(kcal) * Decimal::from(1_000) / energy
                    }
                }
                _ => amount,
            }
        }
        CriterionKind::GroupPortions => sum_groups(day, &criterion.nutrition_groups, |m| &m.groups),
        CriterionKind::GroupAmount => sum_groups(day, &criterion.nutrition_groups, |m| &m.group_amounts),
        CriterionKind::GroupFrequency | CriterionKind::RecipeRepeat | CriterionKind::FamilyRepeat => Decimal::ZERO,
    };
    (value, missing)
}

fn sum_groups(
    day: &Day,
    groups: &[NutritionGroup],
    field: impl Fn(&MealIntake) -> &BTreeMap<NutritionGroup, Decimal>,
) -> Decimal {
    day.meals.iter().flat_map(|m| groups.iter().filter_map(|g| field(m).get(g).copied())).sum()
}

/// Bornes d'un critère, tolérance appliquée (R-NUT-12) et cible réduite de `reduction` (R-NUT-31).
fn bounds(criterion: &Criterion, reduction: Decimal) -> (Option<Decimal>, Option<Decimal>) {
    let tolerance = criterion.tolerance.map_or(Decimal::ZERO, |t| t.to_decimal());
    let min = criterion.min_value.map(|v| Decimal::from(v) * (Decimal::ONE - tolerance) * reduction);
    let max = criterion.max_value.map(|v| Decimal::from(v) * (Decimal::ONE + tolerance) * reduction);
    (min, max)
}

/// La cible journalière est réduite de la part des créneaux hors foyer, pour l'énergie et les nutriments en
/// valeur absolue ; ni les parts de l'énergie, ni les critères de groupe (R-NUT-31).
fn reduced(criterion: &Criterion) -> bool {
    matches!(criterion.kind, CriterionKind::Energy | CriterionKind::Nutrient)
        && criterion.expression == Expression::Absolute
}

/// Bilan d'un critère sur des journées consécutives (R-NUT-40, R-NUT-42) :
/// - un critère journalier est évalué sur la **moyenne** des journées où un repas est compté ;
/// - un critère hebdomadaire, sur la **somme** de la période (portions, occurrences) ou sur ses répétitions.
pub fn assess(criterion: &Criterion, days: &[Day]) -> Assessment {
    let counted: Vec<&Day> = days.iter().filter(|d| !d.meals.is_empty()).collect();
    let not_applicable =
        || Assessment { value: None, min: None, max: None, status: Status::NotApplicable, missing: vec![] };
    if counted.is_empty() {
        return not_applicable();
    }
    let mut missing = Vec::new();
    match (criterion.kind, criterion.period) {
        (CriterionKind::GroupFrequency, _) => {
            let threshold = Decimal::from(criterion.occurrence_threshold.map_or(1_000, |a| a.raw()));
            let occurrences = counted
                .iter()
                .flat_map(|d| d.meals.iter())
                .filter(|m| {
                    criterion.nutrition_groups.iter().filter_map(|g| m.groups.get(g)).sum::<Decimal>() >= threshold
                })
                .count();
            merge(
                &mut missing,
                counted.iter().flat_map(|d| d.empty_slots.iter().map(|s| format!("{s} : aucun repas"))),
            );
            let (min, max) = bounds(criterion, Decimal::ONE);
            let value = Decimal::from(occurrences);
            Assessment { value: Some(value), min, max, status: status(value, min, max, !missing.is_empty()), missing }
        }
        (CriterionKind::FamilyRepeat, _) => {
            let Some(family) = criterion.food_group_id else { return not_applicable() };
            let occurrences = counted
                .iter()
                .flat_map(|d| d.meals.iter())
                .filter(|m| m.families.get(&family).is_some_and(|p| *p >= Decimal::from(1_000)))
                .count();
            let (min, max) = bounds(criterion, Decimal::ONE);
            let value = Decimal::from(occurrences);
            Assessment { value: Some(value), min, max, status: status(value, min, max, false), missing }
        }
        (CriterionKind::RecipeRepeat, _) => {
            // Plus petit écart, en jours, entre deux repas où la même recette est cuisinée.
            let mut seen: BTreeMap<RecipeId, Date> = BTreeMap::new();
            let mut smallest: Option<i64> = None;
            for day in &counted {
                for recipe in day.meals.iter().flat_map(|m| m.cooked_recipes.iter()) {
                    if let Some(previous) = seen.insert(*recipe, day.date) {
                        let gap = (day.date - previous).num_days();
                        smallest = Some(smallest.map_or(gap, |s| s.min(gap)));
                    }
                }
            }
            let (min, max) = bounds(criterion, Decimal::ONE);
            match smallest {
                Some(gap) => {
                    let value = Decimal::from(gap);
                    Assessment { value: Some(value), min, max, status: status(value, min, max, false), missing }
                }
                // Aucune recette répétée : le critère est respecté.
                None => Assessment { value: None, min, max, status: Status::Met, missing },
            }
        }
        (_, Period::Week) => {
            let mut value = Decimal::ZERO;
            for day in &counted {
                let (v, m) = day_value(criterion, day);
                value += v;
                merge(&mut missing, m);
            }
            let (min, max) = bounds(criterion, Decimal::ONE);
            Assessment { value: Some(value), min, max, status: status(value, min, max, !missing.is_empty()), missing }
        }
        (_, Period::Day) => {
            let n = Decimal::from(counted.len());
            let (mut value, mut min_sum, mut max_sum) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
            for day in &counted {
                let (v, m) = day_value(criterion, day);
                value += v;
                merge(&mut missing, m);
                let reduction =
                    if reduced(criterion) { Decimal::ONE - day.out_of_home_share.to_decimal() } else { Decimal::ONE };
                let (min, max) = bounds(criterion, reduction);
                min_sum += min.unwrap_or_default();
                max_sum += max.unwrap_or_default();
            }
            let min = criterion.min_value.map(|_| min_sum / n);
            let max = criterion.max_value.map(|_| max_sum / n);
            let value = value / n;
            Assessment { value: Some(value), min, max, status: status(value, min, max, !missing.is_empty()), missing }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nutrition::Importance;
    use crate::recipes::values::NutrientTotal;
    use crate::shared::Amount;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn criterion(kind: CriterionKind, code: Option<&str>, min: Option<i64>, max: Option<i64>) -> Criterion {
        Criterion {
            kind,
            nutrient_code: code.map(str::to_owned),
            nutrition_groups: vec![],
            food_group_id: None,
            expression: Expression::Absolute,
            period: Period::Day,
            min_value: min,
            max_value: max,
            tolerance: None,
            occurrence_threshold: None,
            importance: Importance::Preferred,
        }
    }

    fn meal(values: &[(&str, i64)], missing: &[&str]) -> MealIntake {
        let mut totals = Totals::default();
        for (code, value) in values {
            totals.nutrients.insert(
                (*code).to_owned(),
                NutrientTotal {
                    code: (*code).to_owned(),
                    value: Decimal::from(*value),
                    estimated: false,
                    missing: missing.iter().map(|m| (*m).to_owned()).collect(),
                },
            );
        }
        MealIntake { totals, ..Default::default() }
    }

    fn day(date: &str, meals: Vec<MealIntake>) -> Day {
        Day { date: d(date), meals, out_of_home_share: Ratio::from_thousandths(0), empty_slots: vec![] }
    }

    /// Énergie 2 000 kcal ± 10 % : 1 900 kcal est atteint, 1 700 kcal est en dessous (R-NUT-12).
    #[test]
    fn r_nut_12_energy_tolerance() {
        let mut energy = criterion(CriterionKind::Energy, Some("ENERGY"), Some(2_000_000), Some(2_000_000));
        energy.tolerance = Some(Ratio::from_thousandths(100));
        let ok = assess(&energy, &[day("2026-11-02", vec![meal(&[("ENERGY", 1_900_000)], &[])])]);
        assert_eq!(ok.status, Status::Met);
        assert_eq!(ok.min, Some(Decimal::from(1_800_000)));
        let low = assess(&energy, &[day("2026-11-02", vec![meal(&[("ENERGY", 1_700_000)], &[])])]);
        assert_eq!(low.status, Status::Below);
    }

    /// Un déjeuner pris hors foyer (35 % de l'énergie) réduit la cible du jour (R-NUT-31).
    #[test]
    fn r_nut_31_out_of_home_reduces_target() {
        let energy = criterion(CriterionKind::Energy, Some("ENERGY"), Some(2_000_000), None);
        let mut monday = day("2026-11-02", vec![meal(&[("ENERGY", 1_300_000)], &[])]);
        monday.out_of_home_share = Ratio::from_thousandths(350);
        let result = assess(&energy, &[monday]);
        assert_eq!(result.min, Some(Decimal::from(1_300_000)));
        assert_eq!(result.status, Status::Met);
    }

    /// Lipides 35 à 40 % de l'énergie : 80 g pour 2 000 kcal font 36 % (R-NUT-11).
    #[test]
    fn r_nut_11_percent_of_energy() {
        let mut fat = criterion(CriterionKind::Nutrient, Some("FAT"), Some(350), Some(400));
        fat.expression = Expression::PercentEnergy;
        let result = assess(&fat, &[day("2026-11-02", vec![meal(&[("ENERGY", 2_000_000), ("FAT", 80_000)], &[])])]);
        assert_eq!(result.value, Some(Decimal::from(360)));
        assert_eq!(result.status, Status::Met);
    }

    /// Un total incomplet n'est jamais atteint ; un maximum déjà dépassé l'est quand même (R-NUT-24).
    #[test]
    fn r_nut_24_incomplete() {
        let fiber = criterion(CriterionKind::Nutrient, Some("FIBER"), Some(30_000), None);
        let partial = assess(&fiber, &[day("2026-11-02", vec![meal(&[("FIBER", 35_000)], &["Pain maison"])])]);
        assert_eq!(partial.status, Status::Incomplete);
        assert_eq!(partial.missing, vec!["Pain maison"]);
        let sugars = criterion(CriterionKind::Nutrient, Some("SUGARS"), None, Some(100_000));
        let over = assess(&sugars, &[day("2026-11-02", vec![meal(&[("SUGARS", 120_000)], &["Gâteau"])])]);
        assert_eq!(over.status, Status::Above);
    }

    /// Un créneau sans repas rend la journée incomplète (R-NUT-32) ; sans aucun repas, le critère ne
    /// s'applique pas.
    #[test]
    fn r_nut_32_empty_slot() {
        let energy = criterion(CriterionKind::Energy, Some("ENERGY"), Some(2_000_000), None);
        let mut monday = day("2026-11-02", vec![meal(&[("ENERGY", 2_100_000)], &[])]);
        monday.empty_slots = vec!["Déjeuner du 02/11".into()];
        let result = assess(&energy, &[monday]);
        assert_eq!(result.status, Status::Incomplete);
        assert_eq!(result.missing, vec!["Déjeuner du 02/11 : aucun repas"]);
        assert_eq!(assess(&energy, &[day("2026-11-03", vec![])]).status, Status::NotApplicable);
    }

    /// Un jour sous la cible n'est pas une alerte si la moyenne de la semaine l'atteint (R-NUT-42).
    #[test]
    fn r_nut_42_daily_criteria_average_over_the_week() {
        let fiber = criterion(CriterionKind::Nutrient, Some("FIBER"), Some(30_000), None);
        let days = vec![
            day("2026-11-02", vec![meal(&[("FIBER", 25_000)], &[])]),
            day("2026-11-03", vec![meal(&[("FIBER", 36_000)], &[])]),
        ];
        let result = assess(&fiber, &days);
        assert_eq!(result.value, Some(Decimal::from(30_500)));
        assert_eq!(result.status, Status::Met);
    }

    /// « Fruits et légumes 5 portions par jour » et « légumineuses 2 fois par semaine, dès 1 portion »
    /// (R-NUT-13, R-NUT-14).
    #[test]
    fn r_nut_14_group_portions_and_frequency() {
        let mut fruits = criterion(CriterionKind::GroupPortions, None, Some(5_000), None);
        fruits.nutrition_groups = vec![NutritionGroup::Vegetables, NutritionGroup::Fruits];
        let lunch = MealIntake {
            groups: BTreeMap::from([
                (NutritionGroup::Vegetables, Decimal::from(2_000)),
                (NutritionGroup::Fruits, Decimal::from(1_000)),
                (NutritionGroup::Legumes, Decimal::from(1_200)),
            ]),
            ..Default::default()
        };
        let dinner = MealIntake {
            groups: BTreeMap::from([
                (NutritionGroup::Vegetables, Decimal::from(1_500)),
                (NutritionGroup::Legumes, Decimal::from(500)),
            ]),
            ..Default::default()
        };
        let days = vec![day("2026-11-02", vec![lunch, dinner])];
        let result = assess(&fruits, &days);
        assert_eq!((result.value, result.status), (Some(Decimal::from(4_500)), Status::Below));

        let mut legumes = criterion(CriterionKind::GroupFrequency, None, Some(2), None);
        legumes.period = Period::Week;
        legumes.nutrition_groups = vec![NutritionGroup::Legumes];
        legumes.occurrence_threshold = Some(Amount::from_raw(1_000));
        let result = assess(&legumes, &days);
        assert_eq!((result.value, result.status), (Some(Decimal::ONE), Status::Below));
    }

    /// « Pas la même recette à moins de 7 jours » : les restes mangés ne comptent pas (R-NUT-15).
    #[test]
    fn r_nut_15_recipe_repeat() {
        let mut repeat = criterion(CriterionKind::RecipeRepeat, None, Some(7), None);
        repeat.period = Period::Week;
        let gratin = RecipeId::new();
        let cooked = || MealIntake { cooked_recipes: vec![gratin], ..Default::default() };
        let leftovers = MealIntake::default();
        let days = vec![
            day("2026-11-02", vec![cooked()]),
            day("2026-11-03", vec![leftovers]),
            day("2026-11-06", vec![cooked()]),
        ];
        let result = assess(&repeat, &days);
        assert_eq!((result.value, result.status), (Some(Decimal::from(4)), Status::Below));
        assert_eq!(assess(&repeat, &days[..2]).status, Status::Met);
    }
}

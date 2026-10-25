//! Bilan nutritionnel d'une période (NUT-04 à NUT-07) : prévu (repas planifiés) ou réel (repas validés),
//! critère par critère, rapporté à l'équivalent adulte (R-NUT-21).

use std::collections::{BTreeMap, HashMap};

use chrono::Days;
use foodflow_domain::foods::groups::ancestors;
use foodflow_domain::nutrition::assessment::{self, Assessment, Day, MealIntake, Status};
use foodflow_domain::nutrition::{self, Criterion};
use foodflow_domain::planning::meals::{planned_leftover, scale_per_adult};
use foodflow_domain::planning::validation::cook_needs;
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::recipes::Target;
use foodflow_domain::recipes::graph::Need;
use foodflow_domain::recipes::scaling::Factor;
use foodflow_domain::recipes::values::{self, Totals};
use foodflow_domain::shared::ids::{FoodId, MealId, NutritionGoalId};
use foodflow_domain::{Amount, Date, DomainError, Measure, Ratio};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::meals::{Planner, portion_needs};
use crate::db::goals::Owner;
use crate::db::planning::{ItemRow, MealRow, Stocked, UseRow};
use crate::db::{self, Db};
use crate::error::AppResult;

#[derive(Debug, Deserialize, IntoParams)]
pub struct AssessmentQuery {
    /// Premier jour.
    pub start: Date,
    /// Nombre de jours (7 par défaut, 31 au plus).
    pub days: Option<u32>,
    /// Bilan réel (repas validés) plutôt que prévu (R-NUT-41).
    #[serde(default)]
    pub actual: bool,
}

/// Bilan d'un critère : valeur et bornes dans l'unité du critère, arrondies (R-NUT-40).
#[derive(Debug, Serialize, ToSchema)]
pub struct CriterionAssessment {
    pub criterion: Criterion,
    pub value: Option<i64>,
    /// Bornes effectives : tolérance appliquée, cible réduite des repas hors foyer (R-NUT-12, R-NUT-31).
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub status: Status,
    /// Ce qui rend la valeur incomplète (R-NUT-24).
    pub missing: Vec<String>,
}

fn rounded(value: Option<Decimal>) -> Option<i64> {
    value.and_then(|v| v.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64())
}

impl CriterionAssessment {
    fn new(criterion: Criterion, assessment: Assessment) -> Self {
        Self {
            criterion,
            value: rounded(assessment.value),
            min: rounded(assessment.min),
            max: rounded(assessment.max),
            status: assessment.status,
            missing: assessment.missing,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DaySummary {
    pub date: Date,
    /// Repas comptés ce jour-là.
    pub meals: u32,
    /// Énergie par équivalent adulte, en cal ; incomplète si `missing` n'est pas vide.
    pub energy: Option<i64>,
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NutritionAssessment {
    pub start: Date,
    pub end: Date,
    pub actual: bool,
    /// Version de l'objectif en vigueur le premier jour (R-NUT-43) ; vide sans objectif.
    pub goal_id: Option<NutritionGoalId>,
    pub criteria: Vec<CriterionAssessment>,
    pub days: Vec<DaySummary>,
}

/// Totaux nutritionnels de besoins développés (R-NUT-20) : un aliment pour sa quantité, une famille pour son
/// aliment de référence (estimé) ; un besoin sans quantité rend le total incomplet.
fn needs_totals(planner: &Planner, needs: &[Need], codes: &[&str]) -> Totals {
    let book = &planner.kitchen.book;
    let mut out = Totals::empty(codes);
    for need in needs.iter().filter(|n| !n.to_taste && !n.optional) {
        let label = planner.kitchen.target_name(&need.target);
        let food = match need.target {
            Target::Food(f) => Some(f),
            Target::Group(g) => book.reference_foods.get(&g).copied(),
            Target::Recipe(_) => None,
        };
        match (food, need.amount) {
            (Some(food), Some(amount)) => {
                let mut part = values::food_totals(book, food, amount, need.measure, &label, codes);
                if matches!(need.target, Target::Group(_)) {
                    for total in part.nutrients.values_mut() {
                        total.estimated = true;
                    }
                }
                out.add(&part);
            }
            _ => out.missing_everywhere(&label),
        }
    }
    out
}

/// Portions par famille d'aliments, familles parentes comprises (critère « même famille », R-NUT-15).
fn families(planner: &Planner, needs: &[Need]) -> BTreeMap<foodflow_domain::shared::ids::FoodGroupId, Decimal> {
    let facts = planner.kitchen.group_facts();
    let catalog = &planner.kitchen.catalog;
    let mut out = BTreeMap::new();
    for need in needs {
        let Target::Food(food) = need.target else { continue };
        let Some(row) = catalog.foods.get(&food) else { continue };
        let portions: Decimal =
            nutrition::group_portions(std::slice::from_ref(need), &facts, &planner.kitchen.book.reference_foods)
                .values()
                .sum();
        if portions.is_zero() {
            continue;
        }
        for group in std::iter::once(row.group_id).chain(ancestors(&catalog.parents, row.group_id)) {
            *out.entry(group).or_default() += portions;
        }
    }
    out
}

/// Besoins d'un aliment ou d'un produit (celui de son aliment), en quantité.
fn stocked_need(planner: &Planner, stocked: Stocked, amount: Decimal, measure: Option<Measure>) -> Option<Need> {
    let food: FoodId = match stocked {
        Stocked::Food(f) => f,
        Stocked::Product(p) => planner.kitchen.catalog.products.get(&p)?.food_id?,
    };
    Some(Need { target: Target::Food(food), amount: Some(amount), measure, to_taste: false, optional: false })
}

/// Ce qu'un repas prévu fait manger (R-NUT-20) : les recettes pour les convives (le reste prévu n'est pas
/// mangé), les parts de lots et de restes, les aliments.
fn planned_needs(
    planner: &Planner,
    items: &[&ItemRow],
    all_items: &[ItemRow],
    lots: &HashMap<foodflow_domain::shared::ids::LotId, crate::db::stock::LotRow>,
    equivalents: Ratio,
) -> Result<Vec<Need>, DomainError> {
    let mut needs = Vec::new();
    for item in items {
        match item.source {
            ItemSource::Cook => {
                let (Some(recipe), Some(cooked)) = (item.recipe_id, item.cooked_portions) else { continue };
                let eaten = Amount::from_raw(cooked.raw() - planned_leftover(cooked, equivalents).raw());
                if eaten.is_positive() {
                    needs.extend(cook_needs(&planner.kitchen.book, recipe, eaten)?);
                }
            }
            ItemSource::Lot => {
                let Some(amount) = item.amount else { continue };
                let lot = item.lot_id.and_then(|l| lots.get(&l));
                let recipe = lot.and_then(|l| l.recipe_id).or_else(|| {
                    item.leftover_of_item_id
                        .and_then(|s| all_items.iter().find(|i| i.id == s))
                        .and_then(|s| s.recipe_id)
                });
                match (recipe, lot.and_then(|l| l.food_id)) {
                    (Some(recipe), _) => needs.extend(portion_needs(&planner.kitchen, recipe, amount)?),
                    (None, Some(food)) => needs.extend(stocked_need(
                        planner,
                        Stocked::Food(food),
                        amount.to_decimal(),
                        lot.map(|l| l.measure),
                    )),
                    _ => {}
                }
            }
            ItemSource::Food => {
                let (Some(stocked), Some(amount)) = (item.stocked, item.amount) else { continue };
                let amount = if item.per_adult_equivalent { scale_per_adult(amount, equivalents)? } else { amount };
                let measure = planner.stocked(stocked).1;
                needs.extend(stocked_need(planner, stocked, amount.to_decimal(), measure));
            }
        }
    }
    Ok(needs)
}

/// Ce qu'un repas validé a fait manger (R-PLA-31) : les quantités réellement utilisées × la part mangée.
fn actual_needs(planner: &Planner, items: &[&ItemRow], uses: &[UseRow]) -> Result<Vec<Need>, DomainError> {
    let mut needs = Vec::new();
    for item in items {
        let ratio = item.eaten_ratio.unwrap_or(Ratio::ONE).to_decimal();
        for u in uses.iter().filter(|u| u.meal_item_id == item.id) {
            let eaten = u.amount.to_decimal() * ratio;
            match (u.recipe_id, u.stocked) {
                (Some(recipe), _) => needs.extend(portion_needs(&planner.kitchen, recipe, Amount::round(eaten)?)?),
                (None, Some(stocked)) => needs.extend(stocked_need(planner, stocked, eaten, Some(u.measure))),
                _ => {}
            }
        }
    }
    Ok(needs)
}

fn intake(
    planner: &Planner,
    needs: &[Need],
    equivalents: Ratio,
    cooked: Vec<foodflow_domain::shared::ids::RecipeId>,
) -> MealIntake {
    let codes: Vec<&str> = planner.kitchen.nutrient_codes.iter().map(String::as_str).collect();
    let facts = planner.kitchen.group_facts();
    let reference = &planner.kitchen.book.reference_foods;
    // Par équivalent adulte (R-NUT-21).
    let per_adult =
        Factor::new(Decimal::from(1_000), Decimal::from(equivalents.thousandths().max(1))).unwrap_or(Factor::ONE);
    fn per<K: Ord>(map: BTreeMap<K, Decimal>, factor: Factor) -> BTreeMap<K, Decimal> {
        map.into_iter().map(|(k, v)| (k, factor.apply(v))).collect()
    }
    MealIntake {
        slot: None,
        totals: needs_totals(planner, needs, &codes).scaled(per_adult),
        groups: per(nutrition::group_portions(needs, &facts, reference), per_adult),
        group_amounts: per(nutrition::group_amounts(needs, &facts, reference), per_adult),
        families: per(families(planner, needs), per_adult),
        cooked_recipes: cooked,
    }
}

/// Bilan d'une période (R-NUT-40 à R-NUT-44).
pub async fn assess(db: &Db, query: &AssessmentQuery, today: Date) -> AppResult<NutritionAssessment> {
    let count = query.days.unwrap_or(7).clamp(1, 31);
    let end = query.start + Days::new(u64::from(count - 1));
    let mut conn = db.read.acquire().await?;
    let planner = Planner::load(&mut conn).await?;
    let meals = db::planning::meals(&mut *conn, query.start, end).await?;
    let diners = db::planning::diners(&mut *conn, query.start, end).await?;
    let items = db::planning::items(&mut *conn, query.start, end).await?;
    let uses = db::planning::uses(&mut *conn, query.start, end).await?;
    let mut lots = HashMap::new();
    for id in items.iter().filter_map(|i| i.lot_id) {
        if let Some(lot) = db::stock::find(&mut *conn, id).await? {
            lots.insert(id, lot);
        }
    }
    let goals = db::goals::goals(&mut *conn).await?;
    let dated: Vec<(Date, NutritionGoalId)> = goals.iter().map(|g| (g.valid_from, g.id)).collect();
    let goal = nutrition::in_force(&dated, query.start).copied();
    let criteria: Vec<Criterion> = match goal {
        Some(id) => db::goals::criteria(&mut *conn)
            .await?
            .into_iter()
            .filter(|c| c.owner == Owner::Goal(id))
            .map(|c| c.criterion)
            .collect(),
        None => Vec::new(),
    };
    let active: Vec<_> = planner.slots.iter().filter(|s| s.active).collect();
    let equivalents = |meal: MealId| {
        Ratio::from_thousandths(
            diners.iter().filter(|d| d.meal_id == meal).map(|d| d.adult_equivalent.thousandths()).sum(),
        )
    };
    let mut days = Vec::new();
    let mut summaries = Vec::new();
    for offset in 0..count {
        let date = query.start + Days::new(u64::from(offset));
        let day_meals: Vec<&MealRow> = meals.iter().filter(|m| m.date == date).collect();
        let mut intakes = Vec::new();
        let mut out_of_home = 0;
        let mut empty = Vec::new();
        for slot in &active {
            let label = format!("{} du {}", slot.name, date.format("%d/%m"));
            match day_meals.iter().find(|m| m.slot_id == slot.id) {
                None => empty.push(label),
                Some(meal) => match meal.status {
                    MealStatus::OutOfHome => out_of_home += slot.energy_share.thousandths(),
                    MealStatus::Skipped => {}
                    // Bilan réel : un repas prévu passé n'a pas encore été validé (R-NUT-32).
                    MealStatus::Planned if query.actual => {
                        if meal.date < today {
                            empty.push(format!("{label} (à valider)"));
                        }
                    }
                    MealStatus::Planned | MealStatus::Done => {
                        let eq = equivalents(meal.id);
                        if !eq.thousandths().is_positive() {
                            continue;
                        }
                        let own: Vec<&ItemRow> = items.iter().filter(|i| i.meal_id == meal.id).collect();
                        let needs = if query.actual {
                            actual_needs(&planner, &own, &uses)?
                        } else {
                            planned_needs(&planner, &own, &items, &lots, eq)?
                        };
                        let cooked =
                            own.iter().filter(|i| i.source == ItemSource::Cook).filter_map(|i| i.recipe_id).collect();
                        intakes.push(MealIntake { slot: Some(slot.id), ..intake(&planner, &needs, eq, cooked) });
                    }
                },
            }
        }
        let energy = (!intakes.is_empty())
            .then(|| intakes.iter().filter_map(|m| m.totals.nutrients.get("ENERGY")).map(|t| t.value).sum::<Decimal>());
        let mut missing: Vec<String> = empty.iter().map(|e| format!("{e} : aucun repas")).collect();
        for meal in &intakes {
            if let Some(total) = meal.totals.nutrients.get("ENERGY") {
                for m in &total.missing {
                    if !missing.contains(m) {
                        missing.push(m.clone());
                    }
                }
            }
        }
        summaries.push(DaySummary { date, meals: intakes.len() as u32, energy: rounded(energy), missing });
        days.push(Day {
            date,
            meals: intakes,
            out_of_home_share: Ratio::from_thousandths(out_of_home),
            empty_slots: empty,
        });
    }
    Ok(NutritionAssessment {
        start: query.start,
        end,
        actual: query.actual,
        goal_id: goal,
        criteria: criteria
            .into_iter()
            .map(|c| {
                let result = assessment::assess(&c, &days);
                CriterionAssessment::new(c, result)
            })
            .collect(),
        days: summaries,
    })
}

//! Objectifs nutritionnels (spec 04) : critères, versions dans le temps, portions des groupes.
//!
//! Le bilan d'une période (R-NUT-40 à R-NUT-44) se calcule sur les repas : il arrive avec la planification
//! (spec 05).

pub mod assessment;

use std::collections::{BTreeMap, HashMap};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::foods::NutritionGroup;
use crate::foods::conversion::{FoodConversions, convert_exact};
use crate::recipes::graph::Need;
use crate::recipes::{RecipeBook, Target};
use crate::shared::ids::{FoodGroupId, FoodId};
use crate::shared::{Amount, Date, DomainError, Measure, Ratio};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CriterionKind {
    Energy,
    Nutrient,
    GroupPortions,
    GroupAmount,
    GroupFrequency,
    RecipeRepeat,
    FamilyRepeat,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Expression {
    /// Valeur dans l'unité de stockage du nutriment (cal, mg), ou du groupe (portions, mg).
    Absolute,
    /// Part de l'énergie, en millièmes (R-NUT-11).
    PercentEnergy,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Period {
    Day,
    Week,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Importance {
    /// La génération doit le respecter si une solution existe.
    Required,
    /// La génération cherche à s'en approcher.
    Preferred,
    /// Affiché dans le bilan seulement.
    Info,
}

/// Un critère d'objectif (R-NUT-10).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub kind: CriterionKind,
    pub nutrient_code: Option<String>,
    #[serde(default)]
    pub nutrition_groups: Vec<NutritionGroup>,
    pub food_group_id: Option<FoodGroupId>,
    pub expression: Expression,
    pub period: Period,
    /// Unité selon le critère : cal ou mg (nutriment), millièmes de portion (portions), mg (quantité),
    /// occurrences (fréquence, même famille), jours (même recette), millièmes (part de l'énergie).
    pub min_value: Option<i64>,
    pub max_value: Option<i64>,
    /// Énergie : tolérance, en millièmes (± 10 % → 100).
    pub tolerance: Option<Ratio>,
    /// Fréquence : portions minimales, par équivalent adulte, pour qu'un repas compte (R-NUT-14).
    pub occurrence_threshold: Option<Amount>,
    pub importance: Importance,
}

impl Criterion {
    /// Vérifie qu'un critère est complet et cohérent (R-NUT-10 à R-NUT-14).
    pub fn check(&self) -> Result<(), DomainError> {
        use CriterionKind::*;
        if self.min_value.is_none() && self.max_value.is_none() {
            return Err(DomainError::InvalidCriterion("un minimum ou un maximum est requis"));
        }
        if let (Some(min), Some(max)) = (self.min_value, self.max_value)
            && min > max
        {
            return Err(DomainError::InvalidCriterion("le minimum dépasse le maximum"));
        }
        if self.min_value.is_some_and(|v| v < 0) || self.max_value.is_some_and(|v| v < 0) {
            return Err(DomainError::InvalidCriterion("les bornes sont positives"));
        }
        match self.kind {
            Energy if self.tolerance.is_none() => {
                Err(DomainError::InvalidCriterion("une tolérance est requise pour l'énergie"))
            }
            Energy | Nutrient if self.nutrient_code.is_none() => {
                Err(DomainError::InvalidCriterion("un nutriment est requis"))
            }
            Nutrient
                if self.expression == Expression::PercentEnergy && self.nutrient_code.as_deref() == Some("ENERGY") =>
            {
                Err(DomainError::InvalidCriterion("l'énergie ne s'exprime pas en part d'elle-même"))
            }
            GroupPortions | GroupAmount | GroupFrequency if self.nutrition_groups.is_empty() => {
                Err(DomainError::InvalidCriterion("au moins un groupe nutritionnel est requis"))
            }
            FamilyRepeat if self.food_group_id.is_none() => {
                Err(DomainError::InvalidCriterion("une famille est requise"))
            }
            _ if self.expression == Expression::PercentEnergy && self.kind != Nutrient => {
                Err(DomainError::InvalidCriterion("seul un nutriment s'exprime en part de l'énergie"))
            }
            _ => Ok(()),
        }
    }
}

/// Version en vigueur à une date : la plus récente dont la date de début n'est pas postérieure (R-NUT-2).
pub fn in_force<T>(versions: &[(Date, T)], date: Date) -> Option<&T> {
    versions.iter().filter(|(from, _)| *from <= date).max_by_key(|(from, _)| *from).map(|(_, v)| v)
}

/// Une version entrée en vigueur ne se modifie plus : on en crée une nouvelle (R-NUT-3).
pub fn check_editable(valid_from: Date, today: Date) -> Result<(), DomainError> {
    if valid_from <= today { Err(DomainError::GoalVersionLocked) } else { Ok(()) }
}

/// Les parts d'énergie des créneaux actifs totalisent 100 % (R-NUT-30).
pub fn check_energy_shares(shares: &[Ratio]) -> Result<(), DomainError> {
    let total: i64 = shares.iter().map(|s| s.thousandths()).sum();
    if total == 1_000 { Ok(()) } else { Err(DomainError::EnergySharesNotFull) }
}

/// Groupe nutritionnel et taille de portion effectifs d'un aliment (R-ALI-2, R-NUT-22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupFacts {
    pub group: NutritionGroup,
    pub portion: Option<(Measure, Amount)>,
    pub conversions: FoodConversions,
}

/// Portions de chaque groupe nutritionnel dans des besoins développés (R-NUT-22), en millièmes de portion.
/// Un groupe sans taille de portion, ou une quantité non convertible, ne compte pas de portion.
pub fn group_portions(
    needs: &[Need],
    facts: &HashMap<FoodId, GroupFacts>,
    reference_foods: &HashMap<FoodGroupId, FoodId>,
) -> BTreeMap<NutritionGroup, Decimal> {
    let mut out: BTreeMap<NutritionGroup, Decimal> = BTreeMap::new();
    for need in needs.iter().filter(|n| !n.to_taste && !n.optional) {
        let food = match need.target {
            Target::Food(food) => Some(food),
            Target::Group(group) => reference_foods.get(&group).copied(),
            Target::Recipe(_) => None,
        };
        let (Some(info), Some(amount), Some(from)) = (food.and_then(|f| facts.get(&f)), need.amount, need.measure)
        else {
            continue;
        };
        let Some((measure, portion)) = info.portion else { continue };
        if let Ok(quantity) = convert_exact(amount, from, measure, info.conversions)
            && portion.is_positive()
        {
            *out.entry(info.group).or_default() += quantity * Decimal::from(1_000) / portion.to_decimal();
        }
    }
    out
}

/// Quantité de chaque groupe nutritionnel dans des besoins développés, en mg (critère `GROUP_AMOUNT`). Une
/// quantité non convertible en masse ne compte pas.
pub fn group_amounts(
    needs: &[Need],
    facts: &HashMap<FoodId, GroupFacts>,
    reference_foods: &HashMap<FoodGroupId, FoodId>,
) -> BTreeMap<NutritionGroup, Decimal> {
    let mut out: BTreeMap<NutritionGroup, Decimal> = BTreeMap::new();
    for need in needs.iter().filter(|n| !n.to_taste && !n.optional) {
        let food = match need.target {
            Target::Food(food) => Some(food),
            Target::Group(group) => reference_foods.get(&group).copied(),
            Target::Recipe(_) => None,
        };
        let (Some(info), Some(amount), Some(from)) = (food.and_then(|f| facts.get(&f)), need.amount, need.measure)
        else {
            continue;
        };
        if let Ok(quantity) = convert_exact(amount, from, Measure::Mass, info.conversions) {
            *out.entry(info.group).or_default() += quantity;
        }
    }
    out
}

/// Portions de chaque groupe dans une recette, pour le facteur donné (une portion adulte, en général).
pub fn recipe_group_portions(
    book: &RecipeBook,
    id: crate::shared::ids::RecipeId,
    per_serving: crate::recipes::scaling::Factor,
    facts: &HashMap<FoodId, GroupFacts>,
) -> Result<BTreeMap<NutritionGroup, Decimal>, DomainError> {
    let needs = crate::recipes::graph::expand(book, id, per_serving)?;
    Ok(group_portions(&needs, facts, &book.reference_foods))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn criterion(kind: CriterionKind) -> Criterion {
        Criterion {
            kind,
            nutrient_code: Some("FIBER".into()),
            nutrition_groups: vec![],
            food_group_id: None,
            expression: Expression::Absolute,
            period: Period::Day,
            min_value: Some(30_000),
            max_value: None,
            tolerance: None,
            occurrence_threshold: None,
            importance: Importance::Preferred,
        }
    }

    /// Critère d'acceptation : une version change le 15/11 ; la semaine du 02/11 garde l'ancienne (R-NUT-2, R-NUT-3).
    #[test]
    fn r_nut_2_and_3_versions() {
        let versions = vec![(d("2026-11-02"), "A"), (d("2026-11-15"), "B")];
        assert_eq!(in_force(&versions, d("2026-11-05")), Some(&"A"));
        assert_eq!(in_force(&versions, d("2026-11-15")), Some(&"B"));
        assert_eq!(in_force(&versions, d("2026-11-01")), None);
        assert_eq!(check_editable(d("2026-11-02"), d("2026-11-10")), Err(DomainError::GoalVersionLocked));
        assert!(check_editable(d("2026-11-15"), d("2026-11-10")).is_ok());
    }

    #[test]
    fn r_nut_10_criteria_checks() {
        assert!(criterion(CriterionKind::Nutrient).check().is_ok());
        let mut energy = criterion(CriterionKind::Energy);
        energy.nutrient_code = Some("ENERGY".into());
        assert!(energy.check().is_err(), "tolérance requise (R-NUT-12)");
        energy.tolerance = Some(Ratio::from_thousandths(100));
        assert!(energy.check().is_ok());
        let mut fish = criterion(CriterionKind::GroupFrequency);
        assert!(fish.check().is_err(), "un groupe est requis");
        fish.nutrition_groups = vec![NutritionGroup::Fish, NutritionGroup::FattyFish];
        assert!(fish.check().is_ok());
        let mut inverted = criterion(CriterionKind::Nutrient);
        inverted.max_value = Some(10);
        assert!(inverted.check().is_err());
    }

    #[test]
    fn r_nut_30_energy_shares() {
        let shares = [250, 350, 100, 300].map(Ratio::from_thousandths);
        assert!(check_energy_shares(&shares).is_ok());
        assert_eq!(check_energy_shares(&shares[..3]), Err(DomainError::EnergySharesNotFull));
    }

    #[test]
    fn r_nut_22_group_portions() {
        let (zucchini, cheese) = (FoodId::new(), FoodId::new());
        let facts = HashMap::from([
            (
                zucchini,
                GroupFacts {
                    group: NutritionGroup::Vegetables,
                    portion: Some((Measure::Mass, Amount::from_raw(80_000))),
                    conversions: FoodConversions::default(),
                },
            ),
            (
                cheese,
                GroupFacts {
                    group: NutritionGroup::Dairy,
                    portion: Some((Measure::Mass, Amount::from_raw(30_000))),
                    conversions: FoodConversions::default(),
                },
            ),
        ]);
        let need = |target, amount: i64| Need {
            target,
            amount: Some(Decimal::from(amount)),
            measure: Some(Measure::Mass),
            to_taste: false,
            optional: false,
        };
        let needs = vec![need(Target::Food(zucchini), 200_000), need(Target::Food(cheese), 30_000)];
        let portions = group_portions(&needs, &facts, &HashMap::new());
        assert_eq!(portions[&NutritionGroup::Vegetables], Decimal::from(2_500));
        assert_eq!(portions[&NutritionGroup::Dairy], Decimal::from(1_000));
    }
}

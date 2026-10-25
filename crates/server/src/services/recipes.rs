//! Recettes : saisie, fiche avec valeurs calculées et mise à l'échelle, recherche, variantes (spec 03,
//! REC-01 à REC-03, REC-05, REC-06, REC-09, REC-12, REC-16).

use std::collections::{BTreeMap, HashMap, HashSet};

use foodflow_domain::foods::NutritionGroup;
use foodflow_domain::foods::conversion::{FoodConversions, convert_exact};
use foodflow_domain::foods::groups::ancestors;
use foodflow_domain::nutrition::{self, GroupFacts};
use foodflow_domain::recipes::scaling::{self, Factor};
use foodflow_domain::recipes::{
    self as domain, FoodFacts, Ingredient, RecipeBook, RecipeKind, Target, Yield, graph, values,
};
use foodflow_domain::shared::ids::{FoodId, IngredientId, RecipeId, StepId, UnitId};
use foodflow_domain::text::matches;
use foodflow_domain::{Amount, DomainError, Measure, Ratio};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

use super::catalog::Catalog;
use super::{clean, required};
use crate::db::recipes::{IngredientRow, RecipeRow, StepRow};
use crate::db::units::UnitRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ===================================================================== Chargement

/// Tout ce qu'il faut pour calculer : référentiel, recettes, unités, et le livre du domaine.
pub struct Kitchen {
    pub catalog: Catalog,
    pub recipes: Vec<RecipeRow>,
    pub ingredients: Vec<IngredientRow>,
    pub tags: Vec<(RecipeId, String)>,
    pub units: Vec<UnitRow>,
    pub book: RecipeBook,
    pub nutrient_codes: Vec<String>,
}

impl Kitchen {
    pub async fn load(conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        let catalog = Catalog::load(&mut *conn).await?;
        let recipes = db::recipes::list(&mut *conn).await?;
        let ingredients = db::recipes::ingredients(&mut *conn).await?;
        let tags = db::recipes::tags(&mut *conn).await?;
        let units = db::units::all(&mut *conn).await?;
        let nutrient_codes = db::nutrients::list(&mut *conn).await?.into_iter().map(|n| n.code).collect();
        let mut values: HashMap<FoodId, HashMap<String, _>> = HashMap::new();
        for (food, code, value) in db::nutrients::all_food_values(&mut *conn).await? {
            values.entry(food).or_default().insert(code, value);
        }
        let mut kitchen =
            Self { catalog, recipes, ingredients, tags, units, book: RecipeBook::default(), nutrient_codes };
        kitchen.book.foods = kitchen
            .catalog
            .foods
            .values()
            .map(|f| {
                let facts = FoodFacts {
                    name: f.name.clone(),
                    stock_measure: kitchen.catalog.food_settings(f).stock_measure,
                    conversions: conversions(f),
                    basis: f.nutrient_basis,
                    nutrients: values.remove(&f.id).unwrap_or_default(),
                };
                (f.id, facts)
            })
            .collect();
        kitchen.book.reference_foods =
            kitchen.catalog.groups.values().filter_map(|g| g.reference_food_id.map(|f| (g.id, f))).collect();
        let rebuilt: Vec<domain::Recipe> =
            kitchen.recipes.iter().map(|r| kitchen.to_domain(r, &kitchen.ingredients)).collect();
        kitchen.book.recipes = rebuilt.into_iter().map(|r| (r.id, r)).collect();
        Ok(kitchen)
    }

    fn to_domain(&self, recipe: &RecipeRow, ingredients: &[IngredientRow]) -> domain::Recipe {
        domain::Recipe {
            id: recipe.id,
            name: recipe.name.clone(),
            yield_: recipe.yield_,
            ingredients: ingredients
                .iter()
                .filter(|i| i.recipe_id == recipe.id)
                .map(|i| Ingredient {
                    target: i.target,
                    amount: i.amount,
                    measure: i.measure,
                    to_taste: i.to_taste,
                    optional: i.optional,
                    fixed: i.fixed,
                    label: self.ingredient_text(i),
                })
                .collect(),
            active_minutes: recipe.active_minutes,
            passive_minutes: recipe.passive_minutes,
        }
    }

    pub fn target_name(&self, target: &Target) -> String {
        match target {
            Target::Food(id) => self.catalog.foods.get(id).map(|f| f.name.clone()),
            Target::Group(id) => self.catalog.groups.get(id).map(|g| g.name.clone()),
            Target::Recipe(id) => self.recipes.iter().find(|r| r.id == *id).map(|r| r.name.clone()),
        }
        .unwrap_or_else(|| "?".to_owned())
    }

    fn unit(&self, id: Option<UnitId>) -> Option<&UnitRow> {
        id.and_then(|id| self.units.iter().find(|u| u.id == id))
    }

    /// Texte d'un ingrédient tel que saisi : « 2 c. à s. huile d'olive », « Sel ».
    pub fn ingredient_text(&self, i: &IngredientRow) -> String {
        let name = self.target_name(&i.target);
        let Some(quantity) = i.entered_quantity else { return name };
        let unit = self
            .unit(i.entered_unit_id)
            .map(|u| u.abbreviation.clone().unwrap_or_else(|| u.name.clone()))
            .or_else(|| i.entered_unit_label.clone());
        match unit {
            Some(u) => format!("{} {u} {name}", decimal_text(quantity.to_decimal())),
            None => format!("{} {name}", decimal_text(quantity.to_decimal())),
        }
    }
}

fn conversions(food: &db::foods::FoodRow) -> FoodConversions {
    FoodConversions { density_mg_per_ml: food.density_mg_per_ml, piece_weight_mg: food.piece_weight_mg }
}

/// « 1,5 » : décimal à la française, sans zéros inutiles.
fn decimal_text(value: Decimal) -> String {
    value.normalize().to_string().replace('.', ",")
}

fn round_amount(value: Decimal) -> Option<Amount> {
    Amount::round(value).ok()
}

/// Arrondi d'affichage au plus proche, demi vers le haut (ADR-0001), et non au pair comme `Decimal::round`.
fn round_i64(value: Decimal) -> Option<i64> {
    value.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero).to_i64()
}

// ===================================================================== Vues

#[derive(Debug, Serialize, ToSchema)]
pub struct RecipeTimes {
    /// Temps actif de la recette seule.
    pub active_minutes: u32,
    pub passive_minutes: u32,
    /// Temps actif avec les sous-recettes.
    pub total_active_minutes: u32,
    /// Durée du planning avec les sous-recettes, pour un cuisinier (R-REC-41).
    pub total_minutes: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecipeSummary {
    pub id: RecipeId,
    pub name: String,
    pub kind: RecipeKind,
    pub tags: Vec<String>,
    #[serde(rename = "yield")]
    pub yield_: Yield,
    pub times: RecipeTimes,
    /// Se prépare à l'avance (R-REC-42).
    pub make_ahead: bool,
    pub freezable: bool,
    pub archived: bool,
    pub has_image: bool,
    /// Énergie par portion ou pour 100, en cal ; `null` si elle est incomplète (R-REC-52).
    pub energy: Option<i64>,
    /// Ingrédients couverts par le stock actuel et ingrédients comptés (REC-09).
    pub coverage: Option<Coverage>,
}

#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct Coverage {
    pub covered: u32,
    pub total: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngredientView {
    pub id: IngredientId,
    pub section: Option<String>,
    pub target: Target,
    pub target_name: String,
    /// Saisie d'origine, mise à l'échelle demandée (R-REC-12, R-REC-13).
    pub quantity: Option<String>,
    pub unit_id: Option<UnitId>,
    pub unit_label: Option<String>,
    /// Quantité convertie, mise à l'échelle, dans `measure` (portions pour une sous-recette en portions).
    pub amount: Option<Amount>,
    pub measure: Option<Measure>,
    /// Pour un aliment compté en pièces : arrondi à la demi-pièce (R-REC-14).
    pub display_amount: Option<Amount>,
    pub to_taste: bool,
    pub optional: bool,
    pub fixed: bool,
    pub note: Option<String>,
    pub source_line: Option<String>,
    /// Quantité saisie non convertible : elle rend les totaux incomplets (R-REC-21).
    pub unconvertible: bool,
    /// Texte prêt à afficher : « 2 c. à s. Huile d'olive ».
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Step {
    pub section: Option<String>,
    pub text: String,
    pub timer_minutes: Option<u32>,
}

/// Base des valeurs affichées (R-REC-51).
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValuesBasis {
    Portion,
    Per100G,
    Per100Ml,
    Piece,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NutrientView {
    pub code: String,
    /// Dans l'unité de stockage (cal, mg), arrondie.
    pub value: i64,
    pub estimated: bool,
    /// Ingrédients en cause si le total est incomplet (R-REC-52).
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GroupPortion {
    pub group: NutritionGroup,
    /// Portions, en millièmes, pour la base des valeurs (R-NUT-22).
    pub portions: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecipeRef {
    pub id: RecipeId,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecipeDetail {
    #[serde(flatten)]
    pub summary: RecipeSummary,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub fridge_days: Option<u32>,
    pub frozen_days: Option<u32>,
    pub finishing_minutes: Option<u32>,
    pub leftover_food_id: Option<FoodId>,
    pub leftover_food_name: Option<String>,
    pub source_url: Option<String>,
    /// Équivalents adulte servis, en millièmes, si la fiche est mise à l'échelle.
    pub servings: Option<Amount>,
    pub ingredients: Vec<IngredientView>,
    pub steps: Vec<Step>,
    pub values_basis: ValuesBasis,
    pub nutrients: Vec<NutrientView>,
    pub group_portions: Vec<GroupPortion>,
    /// Recettes qui l'utilisent comme sous-recette.
    pub used_by: Vec<RecipeRef>,
}

impl Kitchen {
    fn row(&self, id: RecipeId) -> AppResult<&RecipeRow> {
        self.recipes.iter().find(|r| r.id == id).ok_or(AppError::NotFound("recette"))
    }

    fn times(&self, recipe: &RecipeRow) -> RecipeTimes {
        let (total_active, total) = graph::times(&self.book, recipe.id).unwrap_or((recipe.active_minutes, 0));
        RecipeTimes {
            active_minutes: recipe.active_minutes,
            passive_minutes: recipe.passive_minutes,
            total_active_minutes: total_active,
            total_minutes: total,
        }
    }

    fn per_serving(&self, recipe: &RecipeRow) -> Option<values::Totals> {
        let codes: Vec<&str> = self.nutrient_codes.iter().map(String::as_str).collect();
        let totals = values::totals(&self.book, recipe.id, &codes).ok()?;
        Some(totals.scaled(values::per_serving_factor(&recipe.yield_).ok()?))
    }

    fn summary(&self, recipe: &RecipeRow, coverage: Option<Coverage>) -> RecipeSummary {
        let energy = self
            .per_serving(recipe)
            .and_then(|t| t.nutrients.get("ENERGY").cloned())
            .filter(values::NutrientTotal::complete)
            .and_then(|t| round_i64(t.value));
        RecipeSummary {
            id: recipe.id,
            name: recipe.name.clone(),
            kind: recipe.kind,
            tags: self.tags.iter().filter(|(r, _)| *r == recipe.id).map(|(_, t)| t.clone()).collect(),
            yield_: recipe.yield_,
            times: self.times(recipe),
            make_ahead: recipe.fridge_days.is_some(),
            freezable: recipe.freezable,
            archived: recipe.archived,
            has_image: recipe.image_path.is_some(),
            energy,
            coverage,
        }
    }

    /// Effectif d'un aliment pour les portions des groupes.
    fn group_facts(&self) -> HashMap<FoodId, GroupFacts> {
        self.catalog
            .foods
            .values()
            .filter_map(|f| {
                let s = self.catalog.food_settings(f);
                Some((
                    f.id,
                    GroupFacts {
                        group: s.nutrition_group?,
                        portion: s.portion_measure.zip(s.portion_amount),
                        conversions: conversions(f),
                    },
                ))
            })
            .collect()
    }

    pub fn detail(&self, id: RecipeId, servings: Option<Amount>) -> AppResult<RecipeDetail> {
        let recipe = self.row(id)?;
        let factor = match (servings, recipe.yield_) {
            (Some(eq), y @ Yield::Portions { .. }) => scaling::factor(eq, &y)?,
            _ => Factor::ONE,
        };
        let domain = self.book.recipes.get(&id).ok_or(AppError::NotFound("recette"))?;
        let ingredients = self
            .ingredients
            .iter()
            .filter(|i| i.recipe_id == id)
            .zip(domain.ingredients.iter())
            .map(|(row, ing)| {
                let scaled = scaling::scaled(ing, factor);
                let quantity = row.entered_quantity.map(|q| {
                    let q = q.to_decimal();
                    decimal_text(if row.fixed { q } else { factor.apply(q) }.round_dp(2))
                });
                let unit = self.unit(row.entered_unit_id);
                let display_amount = match row.measure {
                    Some(Measure::Count) => scaled.and_then(|v| scaling::display_count(v).ok()),
                    _ => None,
                };
                IngredientView {
                    id: row.id,
                    section: row.section.clone(),
                    target: row.target,
                    target_name: self.target_name(&row.target),
                    quantity: quantity.clone(),
                    unit_id: row.entered_unit_id,
                    unit_label: unit
                        .map(|u| u.abbreviation.clone().unwrap_or_else(|| u.name.clone()))
                        .or_else(|| row.entered_unit_label.clone()),
                    amount: scaled.and_then(round_amount),
                    measure: row.measure,
                    display_amount,
                    to_taste: row.to_taste,
                    optional: row.optional,
                    fixed: row.fixed,
                    note: row.note.clone(),
                    source_line: row.source_line.clone(),
                    unconvertible: ing.unconvertible(),
                    text: self.ingredient_text(row),
                }
            })
            .collect();
        let per_serving = values::per_serving_factor(&recipe.yield_)?;
        let nutrients = self
            .per_serving(recipe)
            .map(|t| {
                t.nutrients
                    .values()
                    .map(|n| NutrientView {
                        code: n.code.clone(),
                        value: round_i64(n.value).unwrap_or(0),
                        estimated: n.estimated,
                        missing: n.missing.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let portions: BTreeMap<NutritionGroup, Decimal> =
            nutrition::recipe_group_portions(&self.book, id, per_serving, &self.group_facts()).unwrap_or_default();
        let used_by = self
            .ingredients
            .iter()
            .filter(|i| i.target == Target::Recipe(id))
            .filter_map(|i| self.recipes.iter().find(|r| r.id == i.recipe_id))
            .map(|r| RecipeRef { id: r.id, name: r.name.clone() })
            .collect();
        Ok(RecipeDetail {
            summary: self.summary(recipe, None),
            description: recipe.description.clone(),
            notes: recipe.notes.clone(),
            fridge_days: recipe.fridge_days,
            frozen_days: recipe.frozen_days,
            finishing_minutes: recipe.finishing_minutes,
            leftover_food_id: recipe.leftover_food_id,
            leftover_food_name: recipe
                .leftover_food_id
                .and_then(|f| self.catalog.foods.get(&f))
                .map(|f| f.name.clone()),
            source_url: recipe.source_url.clone(),
            servings: servings.filter(|_| matches!(recipe.yield_, Yield::Portions { .. })),
            ingredients,
            steps: Vec::new(),
            values_basis: match recipe.yield_ {
                Yield::Portions { .. } => ValuesBasis::Portion,
                Yield::Amount { measure: Measure::Mass, .. } => ValuesBasis::Per100G,
                Yield::Amount { measure: Measure::Volume, .. } => ValuesBasis::Per100Ml,
                Yield::Amount { measure: Measure::Count, .. } => ValuesBasis::Piece,
            },
            nutrients,
            group_portions: portions
                .into_iter()
                .map(|(group, p)| GroupPortion { group, portions: round_i64(p).unwrap_or(0) })
                .collect(),
            used_by,
        })
    }

    /// Couverture d'une recette par le stock (REC-09) : ingrédients d'aliment ou de famille, hors à volonté et
    /// facultatifs, dont un lot est en stock.
    fn coverage(&self, id: RecipeId, in_stock: &HashSet<FoodId>) -> Coverage {
        let needs = graph::expand(&self.book, id, Factor::ONE).unwrap_or_default();
        let counted: Vec<_> = needs.iter().filter(|n| !n.to_taste && !n.optional).collect();
        let covered = counted
            .iter()
            .filter(|n| match n.target {
                Target::Food(food) => in_stock.contains(&food),
                Target::Group(group) => in_stock.iter().any(|f| {
                    self.catalog.foods.get(f).is_some_and(|food| {
                        food.group_id == group || ancestors(&self.catalog.parents, food.group_id).contains(&group)
                    })
                }),
                Target::Recipe(_) => false,
            })
            .count();
        Coverage { covered: covered as u32, total: counted.len() as u32 }
    }
}

// ===================================================================== Lecture

#[derive(Debug, Default, Deserialize, IntoParams)]
pub struct RecipeQuery {
    /// Mots du nom, des étiquettes ou des ingrédients.
    pub q: Option<String>,
    pub kind: Option<RecipeKind>,
    pub tag: Option<String>,
    /// Temps actif total maximal, en minutes.
    pub max_active: Option<u32>,
    #[serde(default)]
    pub make_ahead: bool,
    /// Recettes qui utilisent cet aliment (directement ou dans une sous-recette).
    pub food_id: Option<FoodId>,
    /// Trie par couverture par le stock actuel (« que cuisiner avec ce que j'ai ? »).
    #[serde(default)]
    pub in_stock: bool,
    #[serde(default)]
    pub include_archived: bool,
}

async fn stock_foods(conn: &mut SqliteConnection, catalog: &Catalog) -> anyhow::Result<HashSet<FoodId>> {
    Ok(db::stock::list(&mut *conn, None, None, false)
        .await?
        .into_iter()
        .filter_map(|l| {
            l.food_id.or_else(|| l.product_id.and_then(|p| catalog.products.get(&p)).and_then(|p| p.food_id))
        })
        .collect())
}

pub async fn search(db: &Db, query: &RecipeQuery) -> AppResult<Vec<RecipeSummary>> {
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    let stock = if query.in_stock { Some(stock_foods(&mut conn, &kitchen.catalog).await?) } else { None };
    let text = clean(query.q.as_deref());
    let mut found: Vec<RecipeSummary> = kitchen
        .recipes
        .iter()
        .filter(|r| query.include_archived || !r.archived)
        .filter(|r| query.kind.is_none_or(|k| r.kind == k))
        .filter(|r| !query.make_ahead || r.fridge_days.is_some())
        .filter(|r| {
            query
                .tag
                .as_deref()
                .is_none_or(|t| kitchen.tags.iter().any(|(id, tag)| *id == r.id && tag.eq_ignore_ascii_case(t)))
        })
        .filter(|r| {
            query.food_id.is_none_or(|food| {
                graph::expand(&kitchen.book, r.id, Factor::ONE)
                    .unwrap_or_default()
                    .iter()
                    .any(|n| n.target == Target::Food(food))
            })
        })
        .filter(|r| {
            text.as_deref().is_none_or(|q| {
                let mut haystack = r.name.clone();
                for (_, tag) in kitchen.tags.iter().filter(|(id, _)| *id == r.id) {
                    haystack.push(' ');
                    haystack.push_str(tag);
                }
                for i in kitchen.ingredients.iter().filter(|i| i.recipe_id == r.id) {
                    haystack.push(' ');
                    haystack.push_str(&kitchen.target_name(&i.target));
                }
                matches(&foodflow_domain::text::normalize(&haystack), q)
            })
        })
        .map(|r| kitchen.summary(r, stock.as_ref().map(|s| kitchen.coverage(r.id, s))))
        .filter(|s| query.max_active.is_none_or(|max| s.times.total_active_minutes <= max))
        .collect();
    if query.in_stock {
        // Les recettes entièrement couvertes d'abord, puis par part couverte (R-GAS-21).
        found.sort_by(|a, b| {
            let share = |s: &RecipeSummary| {
                s.coverage.map_or(Decimal::ZERO, |c| {
                    if c.total == 0 { Decimal::ONE } else { Decimal::from(c.covered) / Decimal::from(c.total) }
                })
            };
            share(b).cmp(&share(a)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
    Ok(found)
}

pub async fn get(db: &Db, id: RecipeId, servings: Option<Amount>) -> AppResult<RecipeDetail> {
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    let mut detail = kitchen.detail(id, servings)?;
    detail.steps = db::recipes::steps(&mut *conn, id)
        .await?
        .into_iter()
        .map(|s| Step { section: s.section, text: s.text, timer_minutes: s.timer_minutes })
        .collect();
    Ok(detail)
}

/// Toutes les étiquettes utilisées, pour les filtres.
pub async fn tags(db: &Db) -> AppResult<Vec<String>> {
    let mut tags: Vec<String> = db::recipes::tags(&db.read).await?.into_iter().map(|(_, t)| t).collect();
    tags.sort();
    tags.dedup();
    Ok(tags)
}

// ===================================================================== Écriture

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct IngredientInput {
    pub section: Option<String>,
    pub target: Target,
    /// Quantité saisie, décimale (« 1,5 », « 0.5 ») ; vide pour un ingrédient à volonté.
    pub quantity: Option<String>,
    /// Unité générale ou propre à l'aliment ; sans unité, la quantité compte des pièces, ou des portions
    /// pour une sous-recette en portions.
    pub unit_id: Option<UnitId>,
    /// Unité non reconnue (import), gardée pour l'affichage ; la quantité est alors non convertible.
    pub unit_label: Option<String>,
    #[serde(default)]
    pub to_taste: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub fixed: bool,
    pub note: Option<String>,
    pub source_line: Option<String>,
    /// Import : nom analysé de la ligne, pour mémoriser le rattachement validé (R-REC-66). Non enregistré.
    #[serde(default)]
    pub import_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RecipeInput {
    pub name: String,
    pub kind: RecipeKind,
    pub description: Option<String>,
    pub notes: Option<String>,
    #[serde(rename = "yield")]
    pub yield_: Yield,
    #[serde(default)]
    pub active_minutes: u32,
    #[serde(default)]
    pub passive_minutes: u32,
    pub fridge_days: Option<u32>,
    #[serde(default)]
    pub freezable: bool,
    pub frozen_days: Option<u32>,
    pub finishing_minutes: Option<u32>,
    pub leftover_food_id: Option<FoodId>,
    pub source_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub ingredients: Vec<IngredientInput>,
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// Lit une quantité saisie (« 1,5 », « 1/2 »).
pub fn parse_quantity(text: &str) -> Option<Decimal> {
    let t = text.trim().replace(',', ".");
    if let Some((num, den)) = t.split_once('/') {
        let (num, den): (Decimal, Decimal) = (num.trim().parse().ok()?, den.trim().parse().ok()?);
        return (!den.is_zero()).then(|| num / den);
    }
    t.parse().ok()
}

impl Kitchen {
    /// Mesure et conversions de la cible d'un ingrédient ; `None` en mesure : portions.
    fn target_measure(&self, target: &Target) -> AppResult<(Option<Measure>, FoodConversions)> {
        match target {
            Target::Food(id) => {
                let food = self.catalog.foods.get(id).ok_or(AppError::NotFound("aliment"))?;
                Ok((Some(self.catalog.food_settings(food).stock_measure), conversions(food)))
            }
            Target::Group(id) => {
                let group = self.catalog.groups.get(id).ok_or(AppError::NotFound("famille"))?;
                let reference = group.reference_food_id.and_then(|f| self.catalog.foods.get(&f));
                Ok((
                    Some(self.catalog.group_settings(*id).stock_measure),
                    reference.map(conversions).unwrap_or_default(),
                ))
            }
            Target::Recipe(id) => match self.row(*id)?.yield_ {
                Yield::Portions { .. } => Ok((None, FoodConversions::default())),
                Yield::Amount { measure, .. } => Ok((Some(measure), FoodConversions::default())),
            },
        }
    }

    /// Convertit une saisie dans la mesure de la cible (R-REC-21) : (quantité, mesure), quantité `None` si la
    /// saisie n'est pas convertible.
    fn convert(&self, input: &IngredientInput, quantity: Decimal) -> AppResult<(Option<Amount>, Option<Measure>)> {
        let (measure, conv) = self.target_measure(&input.target)?;
        if input.unit_label.is_some() && input.unit_id.is_none() {
            return Ok((None, measure));
        }
        let unit = match input.unit_id {
            Some(id) => Some(self.unit(Some(id)).ok_or(AppError::NotFound("unité"))?),
            None => None,
        };
        let amount = match (unit, measure) {
            // Sous-recette en portions : sans unité, des portions ; avec une unité, non convertible.
            (None, None) => Some(quantity * Decimal::from(1_000)),
            (Some(_), None) => None,
            (Some(u), Some(m)) => convert_exact(quantity * u.amount.to_decimal(), u.measure, m, conv).ok(),
            // Sans unité : des pièces.
            (None, Some(m)) => convert_exact(quantity * Decimal::from(1_000), Measure::Count, m, conv).ok(),
        };
        Ok((amount.and_then(round_amount).filter(|a| a.is_positive()), measure))
    }

    /// Construit les lignes d'une recette à partir d'une saisie, en vérifiant les règles (R-REC-1 à R-REC-42).
    fn build(
        &self,
        id: RecipeId,
        input: &RecipeInput,
        existing: Option<&RecipeRow>,
    ) -> AppResult<(RecipeRow, Vec<IngredientRow>, Vec<StepRow>)> {
        let name = required(&input.name, "name", 200)?;
        if input.freezable && input.frozen_days.is_none() {
            return Err(DomainError::FrozenDaysRequired.into());
        }
        if let Some(food) = input.leftover_food_id
            && !self.catalog.foods.contains_key(&food)
        {
            return Err(AppError::NotFound("aliment de reste"));
        }
        let recipe = RecipeRow {
            id,
            name,
            kind: input.kind,
            description: clean(input.description.as_deref()),
            notes: clean(input.notes.as_deref()),
            yield_: input.yield_,
            active_minutes: input.active_minutes,
            passive_minutes: input.passive_minutes,
            fridge_days: input.fridge_days.filter(|d| *d > 0),
            freezable: input.freezable,
            frozen_days: input.frozen_days.filter(|_| input.freezable),
            finishing_minutes: input.finishing_minutes,
            leftover_food_id: input.leftover_food_id,
            source_url: clean(input.source_url.as_deref()).or_else(|| existing.and_then(|e| e.source_url.clone())),
            image_path: existing.and_then(|e| e.image_path.clone()),
            archived: existing.is_some_and(|e| e.archived),
            created_at: existing.map(|e| e.created_at.clone()).unwrap_or_default(),
        };
        let mut ingredients = Vec::new();
        for (position, i) in input.ingredients.iter().enumerate() {
            if i.target == Target::Recipe(id) {
                return Err(DomainError::RecipeCycle.into());
            }
            let quantity = match i.quantity.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
                Some(q) if i.to_taste => {
                    return Err(AppError::invalid_field(
                        "TO_TASTE_WITH_QUANTITY",
                        "ingredients",
                        format!("« {q} » : un ingrédient à volonté n'a pas de quantité (R-REC-22)"),
                    ));
                }
                Some(q) => {
                    Some(parse_quantity(q).filter(|v| v.is_sign_positive() && !v.is_zero()).ok_or_else(|| {
                        AppError::invalid_field(
                            "INVALID_QUANTITY",
                            "ingredients",
                            format!("quantité invalide : « {q} »"),
                        )
                    })?)
                }
                None => None,
            };
            let (amount, measure) = match quantity {
                Some(q) => self.convert(i, q)?,
                None => (None, self.target_measure(&i.target)?.0),
            };
            ingredients.push(IngredientRow {
                id: IngredientId::new(),
                recipe_id: id,
                position: i64::try_from(position).map_err(anyhow::Error::from)?,
                section: clean(i.section.as_deref()),
                target: i.target,
                amount,
                measure,
                to_taste: i.to_taste,
                entered_quantity: quantity
                    .and_then(|q| round_i64(q * Decimal::from(1_000)))
                    .map(Ratio::from_thousandths),
                entered_unit_id: i.unit_id,
                entered_unit_label: if i.unit_id.is_some() { None } else { clean(i.unit_label.as_deref()) },
                note: clean(i.note.as_deref()),
                optional: i.optional,
                fixed: i.fixed,
                source_line: clean(i.source_line.as_deref()),
            });
        }
        let steps = input
            .steps
            .iter()
            .filter(|s| !s.text.trim().is_empty())
            .enumerate()
            .map(|(position, s)| StepRow {
                id: StepId::new(),
                position: position as i64,
                section: clean(s.section.as_deref()),
                text: s.text.trim().to_owned(),
                timer_minutes: s.timer_minutes.filter(|m| *m > 0),
            })
            .collect();
        // Règles du domaine et absence de cycle (R-REC-1, R-REC-22, R-REC-30), sur le livre modifié.
        let mut book = self.book.clone();
        let candidate = self.to_domain(&recipe, &ingredients);
        domain::check(&candidate)?;
        book.recipes.insert(id, candidate);
        graph::check_cycles(&book)?;
        Ok((recipe, ingredients, steps))
    }
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> =
        tags.iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty() && t.chars().count() <= 60).collect();
    out.sort();
    out.dedup();
    out
}

pub async fn create(db: &Db, input: &RecipeInput) -> AppResult<RecipeId> {
    let mut tx = db.write.begin().await?;
    let kitchen = Kitchen::load(&mut tx).await?;
    let id = RecipeId::new();
    let (recipe, ingredients, steps) = kitchen.build(id, input, None)?;
    db::recipes::save(&mut tx, &recipe, &ingredients, &steps, &clean_tags(&input.tags), true).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: RecipeId, input: &RecipeInput) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let kitchen = Kitchen::load(&mut tx).await?;
    let existing = kitchen.row(id)?.clone();
    let (recipe, ingredients, steps) = kitchen.build(id, input, Some(&existing))?;
    db::recipes::save(&mut tx, &recipe, &ingredients, &steps, &clean_tags(&input.tags), false).await?;
    tx.commit().await?;
    Ok(())
}

/// Supprime une recette qui n'est la sous-recette d'aucune autre ; sinon, l'archiver (R-REC-2).
pub async fn delete(db: &Db, id: RecipeId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::recipes::find(&mut *tx, id).await?.ok_or(AppError::NotFound("recette"))?;
    if db::recipes::used_as_sub_recipe(&mut *tx, id).await? > 0 {
        return Err(AppError::invalid(
            "RECIPE_IN_USE",
            "cette recette est la sous-recette d'une autre : l'archiver plutôt",
        ));
    }
    db::recipes::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn set_archived(db: &Db, id: RecipeId, archived: bool) -> AppResult<()> {
    db::recipes::find(&db.read, id).await?.ok_or(AppError::NotFound("recette"))?;
    db::recipes::set_archived(&db.write, id, archived).await?;
    Ok(())
}

/// Duplique une recette pour en faire une variante (REC-12).
pub async fn duplicate(db: &Db, id: RecipeId) -> AppResult<RecipeId> {
    let mut tx = db.write.begin().await?;
    let kitchen = Kitchen::load(&mut tx).await?;
    let source = kitchen.row(id)?;
    let copy = RecipeId::new();
    let recipe = RecipeRow {
        id: copy,
        name: format!("{} (variante)", source.name),
        image_path: None,
        archived: false,
        ..source.clone()
    };
    let ingredients: Vec<IngredientRow> = kitchen
        .ingredients
        .iter()
        .filter(|i| i.recipe_id == id)
        .map(|i| IngredientRow { id: IngredientId::new(), recipe_id: copy, ..i.clone() })
        .collect();
    let steps: Vec<StepRow> =
        db::recipes::steps(&mut *tx, id).await?.into_iter().map(|s| StepRow { id: StepId::new(), ..s }).collect();
    let tags: Vec<String> = kitchen.tags.iter().filter(|(r, _)| *r == id).map(|(_, t)| t.clone()).collect();
    db::recipes::save(&mut tx, &recipe, &ingredients, &steps, &tags, true).await?;
    tx.commit().await?;
    Ok(copy)
}

// ===================================================================== Export JSON-LD (R-REC-70)

fn category(kind: RecipeKind) -> &'static str {
    match kind {
        RecipeKind::Main => "Plat principal",
        RecipeKind::Side => "Accompagnement",
        RecipeKind::Starter => "Entrée",
        RecipeKind::Dessert => "Dessert",
        RecipeKind::Breakfast => "Petit-déjeuner",
        RecipeKind::Snack => "Goûter",
        RecipeKind::Base => "Préparation de base",
    }
}

/// Une recette au format schema.org/Recipe JSON-LD, avec les quantités dans l'unité saisie (R-REC-70).
pub async fn json_ld(db: &Db, id: RecipeId) -> AppResult<serde_json::Value> {
    use foodflow_domain::recipes::durations::format_minutes;
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    let recipe = kitchen.row(id)?;
    let steps = db::recipes::steps(&mut *conn, id).await?;
    let ingredients: Vec<String> = kitchen
        .ingredients
        .iter()
        .filter(|i| i.recipe_id == id)
        .map(|i| match &i.note {
            Some(note) => format!("{}, {note}", kitchen.ingredient_text(i)),
            None => kitchen.ingredient_text(i),
        })
        .collect();
    // Les étapes regroupées par section, dans l'ordre (HowToSection).
    let mut instructions: Vec<serde_json::Value> = Vec::new();
    let mut current: Option<(Option<String>, Vec<serde_json::Value>)> = None;
    for step in &steps {
        let item = serde_json::json!({ "@type": "HowToStep", "text": step.text });
        match current.as_mut() {
            Some((section, items)) if *section == step.section => items.push(item),
            _ => {
                if let Some((section, items)) = current.take() {
                    instructions.push(section_value(section, items));
                }
                current = Some((step.section.clone(), vec![item]));
            }
        }
    }
    if let Some((section, items)) = current {
        instructions.push(section_value(section, items));
    }
    let yield_text = match recipe.yield_ {
        Yield::Portions { portions } => decimal_text(portions.to_base(Measure::Count)),
        Yield::Amount { measure, amount } => {
            format!("{} {}", decimal_text(amount.to_base(measure)), measure.base_symbol())
        }
    };
    let tags: Vec<String> = kitchen.tags.iter().filter(|(r, _)| *r == id).map(|(_, t)| t.clone()).collect();
    Ok(serde_json::json!({
        "@context": "https://schema.org",
        "@type": "Recipe",
        "name": recipe.name,
        "description": recipe.description,
        "recipeCategory": category(recipe.kind),
        "recipeYield": yield_text,
        "prepTime": format_minutes(recipe.active_minutes),
        "cookTime": format_minutes(recipe.passive_minutes),
        "keywords": tags.join(", "),
        "recipeIngredient": ingredients,
        "recipeInstructions": instructions.into_iter().flat_map(|v| match v {
            serde_json::Value::Array(items) => items,
            other => vec![other],
        }).collect::<Vec<_>>(),
        "url": recipe.source_url,
    }))
}

/// Une section nommée, ou ses étapes telles quelles si elle n'a pas de nom.
fn section_value(section: Option<String>, items: Vec<serde_json::Value>) -> serde_json::Value {
    match section {
        Some(name) => serde_json::json!({ "@type": "HowToSection", "name": name, "itemListElement": items }),
        None => serde_json::Value::Array(items),
    }
}

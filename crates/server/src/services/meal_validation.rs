//! Validation d'un repas (PLA-04, ADR-0005) : sorties de stock proposées et ajustables, consommation réelle,
//! restes déclarés (ADR-0012), validation groupée, annulation.

use std::collections::{HashMap, HashSet};

use foodflow_domain::foods::groups::ancestors;
use foodflow_domain::planning::household::{self, Diner as DomainDiner};
use foodflow_domain::planning::meals::{eaten_ratio, leftover_deadline, planned_leftover, scale_per_adult};
use foodflow_domain::planning::validation::{cook_needs, totals};
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::recipes::{Target, Yield};
use foodflow_domain::shared::ids::{BatchId, FoodGroupId, FoodId, LocationId, LotId, MealId, MealItemId, RecipeId};
use foodflow_domain::stock::expiry::EffectiveExpiry;
use foodflow_domain::stock::selection::{Candidate, allocate};
use foodflow_domain::stock::{ExpiryKind, ExpiryStatus, LocationKind, MovementKind};
use foodflow_domain::{Amount, Date, DomainError, Measure, Ratio};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::meals::Planner;
use super::stock::{self, Lot};
use crate::db::planning::{ItemRow, MealRow, Stocked, UseRow};
use crate::db::stock::LotRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Ce qu'une ligne de sortie vise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", content = "id", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LineTarget {
    Food(FoodId),
    /// Une famille « au choix » : n'importe quel aliment de la famille (R-REC-20).
    Group(FoodGroupId),
    /// Une part d'un lot de préparation ou de restes.
    Lot(LotId),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LotTake {
    pub lot_id: LotId,
    pub name: String,
    pub amount: Amount,
    pub status: ExpiryStatus,
    pub expiry: Option<Date>,
}

/// Un besoin d'un élément et les lots où le prendre (R-STK-36).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProposedLine {
    pub target: LineTarget,
    pub name: String,
    /// Mesure des quantités ; vide pour des portions.
    pub measure: Option<Measure>,
    pub needed: Amount,
    pub takes: Vec<LotTake>,
    /// Lots à date dépassée, à confirmer « encore bon » avant d'y puiser (R-STK-37).
    pub to_confirm: Vec<LotTake>,
    /// Quantité que les lots ne couvrent pas : utilisée sans sortie de stock (R-PLA-32).
    pub shortfall: Amount,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProposedItem {
    pub item_id: MealItemId,
    pub name: String,
    pub source: ItemSource,
    pub recipe_id: Option<RecipeId>,
    /// `COOK` : portions cuisinées et reste prévu, proposé comme reste déclaré.
    pub cooked_portions: Option<Amount>,
    pub planned_leftover: Option<Amount>,
    /// Aliment de reste de la recette, s'il y en a un (ADR-0012).
    pub leftover_food_id: Option<FoodId>,
    pub leftover_food_name: Option<String>,
    pub freezable: bool,
    pub lines: Vec<ProposedLine>,
    /// Ingrédients sans quantité convertible : utilisés, sans sortie (R-PLA-32).
    pub unquantified: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ValidationProposal {
    pub meal_id: MealId,
    pub date: Date,
    pub items: Vec<ProposedItem>,
    /// Emplacement proposé pour les restes : le premier réfrigérateur.
    pub leftover_location_id: Option<LocationId>,
}

/// Aliments d'une famille et de ses sous-familles.
fn foods_of_group(planner: &Planner, group: FoodGroupId) -> HashSet<FoodId> {
    let catalog = &planner.kitchen.catalog;
    catalog
        .foods
        .values()
        .filter(|f| f.group_id == group || ancestors(&catalog.parents, f.group_id).contains(&group))
        .map(|f| f.id)
        .collect()
}

fn candidate(lot: &Lot, available: Amount) -> Candidate {
    Candidate {
        lot: lot.id,
        available,
        expiry: lot.effective_expiry,
        status: lot.status,
        opened: lot.opened_on.is_some(),
    }
}

fn take(lot: &Lot, amount: Amount) -> LotTake {
    LotTake {
        lot_id: lot.id,
        name: lot.name.clone(),
        amount,
        status: lot.status,
        expiry: lot.effective_expiry.map(|e: EffectiveExpiry| e.date),
    }
}

/// Répartit un besoin sur des lots, en tenant compte de ce que les lignes précédentes y ont déjà pris.
fn line(
    target: LineTarget,
    name: String,
    measure: Option<Measure>,
    needed: Amount,
    lots: &[&Lot],
    remaining: &mut HashMap<LotId, Amount>,
) -> ProposedLine {
    let candidates: Vec<Candidate> =
        lots.iter().map(|l| candidate(l, *remaining.entry(l.id).or_insert(l.quantity.unwrap_or_default()))).collect();
    let allocation = allocate(&candidates, needed);
    let by_id = |id: LotId| lots.iter().find(|l| l.id == id);
    let takes: Vec<LotTake> = allocation
        .takes
        .iter()
        .filter_map(|(id, amount)| {
            if let Some(r) = remaining.get_mut(id) {
                *r -= *amount;
            }
            by_id(*id).map(|l| take(l, *amount))
        })
        .collect();
    let to_confirm = allocation
        .to_confirm
        .iter()
        .filter_map(|id| by_id(*id).map(|l| take(l, l.quantity.unwrap_or_default())))
        .collect();
    ProposedLine { target, name, measure, needed, takes, to_confirm, shortfall: allocation.shortfall }
}

async fn load(conn: &mut SqliteConnection, id: MealId) -> AppResult<(MealRow, Vec<ItemRow>, Planner)> {
    let meal = db::planning::find_meal(&mut *conn, id).await?.ok_or(AppError::NotFound("repas"))?;
    let items = db::planning::meal_items(&mut *conn, id).await?;
    let planner = Planner::load(&mut *conn).await?;
    Ok((meal, items, planner))
}

/// Sorties proposées pour valider un repas (R-PLA-30) : ingrédients mis à l'échelle des portions cuisinées,
/// pris dans les lots premier périmé, premier sorti.
pub async fn proposal(db: &Db, id: MealId, today: Date) -> AppResult<ValidationProposal> {
    let mut conn = db.read.acquire().await?;
    let (meal, items, planner) = load(&mut conn, id).await?;
    build(&mut conn, &meal, &items, &planner, today).await
}

async fn build(
    conn: &mut SqliteConnection,
    meal: &MealRow,
    items: &[ItemRow],
    planner: &Planner,
    today: Date,
) -> AppResult<ValidationProposal> {
    let diners = db::planning::diners(&mut *conn, meal.date, meal.date).await?;
    let domain: Vec<DomainDiner> = diners
        .iter()
        .filter(|d| d.meal_id == meal.id)
        .map(|d| DomainDiner { member: d.member_id, adult_equivalent: d.adult_equivalent })
        .collect();
    let equivalents = household::adult_equivalents(&domain);
    let catalog = &planner.kitchen.catalog;
    let book = &planner.kitchen.book;

    // Besoins de chaque élément, puis les lots qui peuvent les couvrir.
    struct Need {
        target: LineTarget,
        name: String,
        measure: Option<Measure>,
        amount: Amount,
    }
    let mut needs: Vec<(MealItemId, Vec<Need>, Vec<String>)> = Vec::new();
    let mut foods: HashSet<FoodId> = HashSet::new();
    let mut explicit_lots: HashSet<LotId> = HashSet::new();
    for item in items {
        let mut list = Vec::new();
        let mut unquantified = Vec::new();
        match item.source {
            ItemSource::Cook => {
                let (Some(recipe), Some(portions)) = (item.recipe_id, item.cooked_portions) else { continue };
                let developed = cook_needs(book, recipe, portions)?;
                let (sums, missing) = totals(&developed);
                for total in sums {
                    let amount = Amount::round(total.amount)?;
                    match total.target {
                        Target::Food(f) => {
                            foods.insert(f);
                            let name = catalog.foods.get(&f).map(|x| x.name.clone()).unwrap_or_default();
                            list.push(Need { target: LineTarget::Food(f), name, measure: Some(total.measure), amount });
                        }
                        Target::Group(g) => {
                            foods.extend(foods_of_group(planner, g));
                            list.push(Need {
                                target: LineTarget::Group(g),
                                name: catalog.group_path(g),
                                measure: Some(total.measure),
                                amount,
                            });
                        }
                        Target::Recipe(_) => {}
                    }
                }
                unquantified.extend(missing.iter().map(|t| planner.kitchen.target_name(t)));
            }
            ItemSource::Lot => {
                let lot = item.lot_id.or(match item.leftover_of_item_id {
                    Some(source) => db::planning::find_item(&mut *conn, source).await?.and_then(|s| s.leftover_lot_id),
                    None => None,
                });
                match (lot, item.amount) {
                    (Some(lot), Some(amount)) => {
                        explicit_lots.insert(lot);
                        list.push(Need { target: LineTarget::Lot(lot), name: String::new(), measure: None, amount });
                    }
                    // Le repas d'origine n'est pas encore validé : son reste n'existe pas en stock.
                    _ => unquantified
                        .push("restes d'un repas pas encore validé : validez d'abord le repas d'origine".to_owned()),
                }
            }
            ItemSource::Food => {
                let (Some(stocked), Some(amount)) = (item.stocked, item.amount) else { continue };
                let amount = if item.per_adult_equivalent { scale_per_adult(amount, equivalents)? } else { amount };
                let food = match stocked {
                    Stocked::Food(f) => Some(f),
                    Stocked::Product(p) => catalog.products.get(&p).and_then(|p| p.food_id),
                };
                let (name, measure) = planner.stocked(stocked);
                match food {
                    Some(f) => {
                        foods.insert(f);
                        list.push(Need { target: LineTarget::Food(f), name, measure, amount });
                    }
                    None => unquantified.push(name),
                }
            }
        }
        needs.push((item.id, list, unquantified));
    }
    let mut lots = stock::available_lots(&mut *conn, today, &foods, &HashSet::new()).await?;
    for id in explicit_lots {
        if !lots.iter().any(|l| l.id == id) {
            lots.push(stock::get_in(&mut *conn, id, today).await?);
        }
    }

    // Un lot inventorié après le repas ne sert pas : l'inventaire a déjà constaté ce qu'il en restait
    // (R-STK-14) ; ce qui en a été pris compte comme utilisé, sans sortie.
    lots.retain(|l| l.last_inventory.is_none_or(|d| d <= meal.date));
    let mut remaining: HashMap<LotId, Amount> = HashMap::new();
    let mut proposed = Vec::new();
    for (item_id, list, unquantified) in needs {
        let Some(item) = items.iter().find(|i| i.id == item_id) else { continue };
        let mut lines = Vec::new();
        for need in list {
            let pool: Vec<&Lot> = match need.target {
                LineTarget::Food(f) => {
                    lots.iter().filter(|l| l.food_id == Some(f) && Some(l.measure) == need.measure).collect()
                }
                LineTarget::Group(g) => {
                    let members = foods_of_group(planner, g);
                    lots.iter()
                        .filter(|l| l.food_id.is_some_and(|f| members.contains(&f)) && Some(l.measure) == need.measure)
                        .collect()
                }
                LineTarget::Lot(id) => lots.iter().filter(|l| l.id == id).collect(),
            };
            let (name, measure) = match need.target {
                LineTarget::Lot(_) => {
                    let lot = pool.first();
                    (
                        lot.map(|l| l.name.clone()).unwrap_or_default(),
                        lot.and_then(|l| l.recipe_id.is_none().then_some(l.measure)),
                    )
                }
                _ => (need.name, need.measure),
            };
            lines.push(line(need.target, name, measure, need.amount, &pool, &mut remaining));
        }
        let recipe = item.recipe_id.and_then(|r| planner.kitchen.recipes.iter().find(|x| x.id == r));
        let cooking = item.source == ItemSource::Cook;
        proposed.push(ProposedItem {
            item_id,
            name: match item.source {
                ItemSource::Cook => recipe.map(|r| r.name.clone()).unwrap_or_default(),
                ItemSource::Lot => lines.first().map(|l| l.name.clone()).unwrap_or_else(|| "Restes".to_owned()),
                ItemSource::Food => item.stocked.map(|s| planner.stocked(s).0).unwrap_or_default(),
            },
            source: item.source,
            recipe_id: item.recipe_id,
            cooked_portions: item.cooked_portions,
            planned_leftover: item.cooked_portions.filter(|_| cooking).map(|c| planned_leftover(c, equivalents)),
            leftover_food_id: recipe.and_then(|r| r.leftover_food_id),
            leftover_food_name: recipe
                .and_then(|r| r.leftover_food_id)
                .and_then(|f| catalog.foods.get(&f))
                .map(|f| f.name.clone()),
            freezable: recipe.is_some_and(|r| r.freezable),
            lines,
            unquantified,
        });
    }
    let locations = db::locations::list(&mut *conn).await?;
    let fridge = locations.iter().find(|l| l.kind == LocationKind::Fridge).or(locations.first()).map(|l| l.id);
    Ok(ValidationProposal { meal_id: meal.id, date: meal.date, items: proposed, leftover_location_id: fridge })
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TakeInput {
    pub lot_id: LotId,
    pub amount: Amount,
}

/// Une ligne retenue : ce qui a été réellement utilisé, et les lots où on l'a pris.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LineInput {
    pub item_id: MealItemId,
    pub target: LineTarget,
    pub measure: Option<Measure>,
    /// Quantité réellement utilisée (R-PLA-31) ; au-delà des sorties, utilisée sans sortie (R-PLA-32).
    pub used: Amount,
    #[serde(default)]
    pub takes: Vec<TakeInput>,
}

/// Reste déclaré d'une recette cuisinée (ADR-0012) : en portions, ou en quantité de l'aliment de reste.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LeftoverInput {
    pub item_id: MealItemId,
    pub amount: Amount,
    pub location_id: LocationId,
    /// Lot de l'aliment de reste de la recette plutôt qu'un lot de la recette.
    #[serde(default)]
    pub as_food: bool,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct ValidationInput {
    /// Lignes retenues ; vide : la proposition telle quelle (validation groupée, R-PLA-33).
    pub lines: Option<Vec<LineInput>>,
    #[serde(default)]
    pub leftovers: Vec<LeftoverInput>,
}

/// Valide un repas (R-PLA-30, R-PLA-31) : sorties de stock, consommation réelle, restes déclarés, en un lot
/// d'opérations qui s'annule en bloc (R-STK-12).
pub async fn validate(db: &Db, id: MealId, input: &ValidationInput, today: Date) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    validate_in(&mut tx, id, input, today).await?;
    tx.commit().await?;
    Ok(())
}

async fn validate_in(conn: &mut SqliteConnection, id: MealId, input: &ValidationInput, today: Date) -> AppResult<()> {
    let (meal, items, planner) = load(&mut *conn, id).await?;
    if meal.status != MealStatus::Planned {
        return Err(DomainError::MealNotPlanned.into());
    }
    if meal.date > today {
        return Err(DomainError::MovementInFuture.into());
    }
    let lines: Vec<LineInput> = match &input.lines {
        Some(lines) => lines.clone(),
        None => build(&mut *conn, &meal, &items, &planner, today)
            .await?
            .items
            .iter()
            .flat_map(|i| {
                i.lines.iter().map(|l| LineInput {
                    item_id: i.item_id,
                    target: l.target,
                    measure: l.measure,
                    used: l.needed,
                    takes: l.takes.iter().map(|t| TakeInput { lot_id: t.lot_id, amount: t.amount }).collect(),
                })
            })
            .collect(),
    };
    let batch = BatchId::new();
    let catalog = &planner.kitchen.catalog;
    for line in &lines {
        let item = items.iter().find(|i| i.id == line.item_id).ok_or(AppError::NotFound("élément de repas"))?;
        let mut taken_total = Amount::ZERO;
        for take in line.takes.iter().filter(|t| t.amount.is_positive()) {
            let lot = db::stock::find(&mut *conn, take.lot_id).await?.ok_or(AppError::NotFound("lot"))?;
            if stock::get_in(&mut *conn, lot.id, today).await?.last_inventory.is_some_and(|d| d > meal.date) {
                continue;
            }
            let (movement, taken) = stock::withdraw(&mut *conn, lot.id, take.amount, meal.date, today, batch).await?;
            taken_total += taken;
            let row = UseRow {
                meal_item_id: item.id,
                stocked: match (lot.product_id, lot.food_id) {
                    (Some(p), _) => Some(Stocked::Product(p)),
                    (None, Some(f)) => Some(Stocked::Food(f)),
                    _ => None,
                },
                recipe_id: lot.recipe_id,
                amount: taken,
                measure: lot.measure,
                movement_id: Some(movement),
            };
            db::planning::insert_use(&mut *conn, &row).await?;
        }
        // Le reste de ce qui a été utilisé ne sort d'aucun lot : il compte quand même (R-PLA-32).
        let rest = line.used - taken_total;
        if rest.is_positive() {
            let (stocked, recipe) = match line.target {
                LineTarget::Food(f) => (Some(Stocked::Food(f)), None),
                LineTarget::Group(g) => {
                    (catalog.groups.get(&g).and_then(|x| x.reference_food_id).map(Stocked::Food), None)
                }
                LineTarget::Lot(l) => (None, db::stock::find(&mut *conn, l).await?.and_then(|x| x.recipe_id)),
            };
            if stocked.is_some() || recipe.is_some() {
                let measure = line.measure.unwrap_or(Measure::Count);
                let row = UseRow {
                    meal_item_id: item.id,
                    stocked,
                    recipe_id: recipe,
                    amount: rest,
                    measure,
                    movement_id: None,
                };
                db::planning::insert_use(&mut *conn, &row).await?;
            }
        }
    }

    // Restes déclarés : un lot de la recette, en portions, ou de son aliment de reste (ADR-0012, R-STK-34).
    let locations = db::locations::list(&mut *conn).await?;
    for item in &items {
        let declared = input.leftovers.iter().find(|l| l.item_id == item.id && l.amount.is_positive());
        let mut updated = ItemRow { eaten_ratio: Some(Ratio::ONE), ..item.clone() };
        if let (ItemSource::Cook, Some(leftover)) = (item.source, declared) {
            let recipe = item
                .recipe_id
                .and_then(|r| planner.kitchen.recipes.iter().find(|x| x.id == r))
                .ok_or(AppError::NotFound("recette"))?;
            let location =
                locations.iter().find(|l| l.id == leftover.location_id).ok_or(AppError::NotFound("emplacement"))?;
            let freezer = location.kind == LocationKind::Freezer;
            if freezer && !recipe.freezable {
                return Err(AppError::invalid_field(
                    "NOT_FREEZABLE",
                    "location_id",
                    "cette recette ne se congèle pas (R-STK-34)",
                ));
            }
            let (food, measure) = match (leftover.as_food, recipe.leftover_food_id) {
                (true, Some(food)) => {
                    let row = catalog.foods.get(&food).ok_or(AppError::NotFound("aliment"))?;
                    (Some(food), catalog.food_settings(row).stock_measure)
                }
                (true, None) => return Err(DomainError::MealItemContent.into()),
                (false, _) => (
                    None,
                    match recipe.yield_ {
                        Yield::Portions { .. } => Measure::Count,
                        Yield::Amount { measure, .. } => measure,
                    },
                ),
            };
            let lot = LotRow {
                id: LotId::new(),
                product_id: None,
                food_id: food,
                recipe_id: if food.is_some() { None } else { Some(recipe.id) },
                location_id: location.id,
                tracking_mode: foodflow_domain::foods::TrackingMode::Quantity,
                measure,
                expiry_kind: ExpiryKind::UseBy,
                expiry_date: Some(leftover_deadline(meal.date, recipe.fridge_days)),
                expiry_missing: false,
                stocked_on: meal.date,
                opened_on: None,
                frozen_on: freezer.then_some(meal.date),
                thawed_on: None,
                checked_until: None,
                unit_price: None,
                needs_review: false,
                depleted: false,
                note: None,
            };
            stock::create_leftover(&mut *conn, &lot, leftover.amount, MovementKind::Leftover, batch).await?;
            if !leftover.as_food {
                updated.eaten_ratio = item.cooked_portions.map(|c| eaten_ratio(c, leftover.amount));
            }
            updated.leftover_lot_id = Some(lot.id);
            db::planning::attach_leftover_lot(&mut *conn, item.id, Some(lot.id)).await?;
        }
        db::planning::save_item(&mut *conn, &updated).await?;
    }
    let done = MealRow { status: MealStatus::Done, validation_batch_id: Some(batch), ..meal };
    db::planning::upsert_meal(&mut *conn, &done).await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchValidationInput {
    pub meal_ids: Vec<MealId>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FailedValidation {
    pub meal_id: MealId,
    pub reason: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchValidationResult {
    pub validated: Vec<MealId>,
    /// Repas non validés, avec la raison.
    pub failed: Vec<FailedValidation>,
}

/// Valide plusieurs repas avec les valeurs proposées, sans restes déclarés (R-PLA-33), dans l'ordre des dates.
pub async fn validate_many(db: &Db, input: &BatchValidationInput, today: Date) -> AppResult<BatchValidationResult> {
    let mut meals = Vec::new();
    for id in &input.meal_ids {
        if let Some(meal) = db::planning::find_meal(&db.read, *id).await? {
            meals.push(meal);
        }
    }
    meals.sort_by_key(|m| m.date);
    let mut result = BatchValidationResult { validated: vec![], failed: vec![] };
    for meal in meals {
        let mut tx = db.write.begin().await?;
        match validate_in(&mut tx, meal.id, &ValidationInput::default(), today).await {
            Ok(()) => {
                tx.commit().await?;
                result.validated.push(meal.id);
            }
            Err(e) => result.failed.push(FailedValidation { meal_id: meal.id, reason: e.to_string() }),
        }
    }
    Ok(result)
}

/// Annule la validation d'un repas (R-PLA-34) : ses sorties et ses restes sont annulés, il revient à `PLANNED`.
/// Des restes déjà mangés à un repas validé depuis empêchent l'annulation : annuler d'abord ce repas-là.
pub async fn unvalidate(db: &Db, id: MealId, today: Date) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let meal = db::planning::find_meal(&mut *tx, id).await?.ok_or(AppError::NotFound("repas"))?;
    let (MealStatus::Done, Some(batch)) = (meal.status, meal.validation_batch_id) else {
        return Err(DomainError::MealNotPlanned.into());
    };
    let items = db::planning::meal_items(&mut *tx, id).await?;
    for lot in items.iter().filter_map(|i| i.leftover_lot_id) {
        let movements = db::stock::movements_of_lot(&mut *tx, lot).await?;
        let reversed = |m: &crate::db::stock::MovementRow| movements.iter().any(|x| x.reverses_id == Some(m.id));
        if movements.iter().any(|m| m.batch_id != Some(batch) && m.reverses_id.is_none() && !reversed(m)) {
            return Err(AppError::invalid_field(
                "LEFTOVERS_USED",
                "meal",
                "les restes de ce repas ont déjà été utilisés : annuler d'abord le repas qui les a mangés",
            ));
        }
    }
    stock::reverse_batch(&mut tx, batch, today).await?;
    for item in &items {
        db::planning::attach_leftover_lot(&mut *tx, item.id, None).await?;
    }
    db::planning::clear_validation(&mut tx, id).await?;
    let planned = MealRow { status: MealStatus::Planned, validation_batch_id: None, ..meal };
    db::planning::upsert_meal(&mut *tx, &planned).await?;
    tx.commit().await?;
    Ok(())
}

//! Calendrier des repas (PLA-03, PLA-05, PLA-09, PLA-13) : semaine planifiée, repas créés d'après la semaine
//! type, éléments et restes prévus, signalements, copie d'une semaine.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Days, NaiveTime};
use foodflow_domain::planning::household::{self, Diner as DomainDiner, Preference};
use foodflow_domain::planning::meals::{leftover_deadline, planned_leftover, scale_per_adult, window_minutes};
use foodflow_domain::planning::signals::{self, MealFacts, Signal};
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::recipes::Target;
use foodflow_domain::recipes::graph::{self, expand};
use foodflow_domain::shared::ids::{
    FoodId, LotId, MealId, MealItemId, MealSlotId, MemberId, ProductId, RecipeId, SlotRoleId,
};
use foodflow_domain::{Amount, Date, DomainError, Measure, Ratio};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

use super::clean;
use super::recipes::Kitchen;
use crate::db::goals::SlotRow;
use crate::db::planning::{DinerRow, ItemRow, MealRow, MemberRow, RoleRow, ScheduleRow, Stocked};
use crate::db::stock::LotRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Un convive d'un repas : un membre, ou un invité (`member_id` vide).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Diner {
    pub member_id: Option<MemberId>,
    pub guest_label: Option<String>,
    pub adult_equivalent: Ratio,
    /// Nom du membre ou libellé de l'invité (lecture seule).
    #[serde(default)]
    pub name: String,
}

/// Un élément de repas (R-PLA-12) : une recette cuisinée, une part d'un lot ou d'un reste prévu, un aliment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MealItem {
    /// Vide pour un nouvel élément.
    pub id: Option<MealItemId>,
    pub role_id: Option<SlotRoleId>,
    pub source: ItemSource,
    pub recipe_id: Option<RecipeId>,
    /// `COOK` : portions cuisinées, en millièmes (les équivalents adulte du repas par défaut, R-PLA-12).
    pub cooked_portions: Option<Amount>,
    /// `LOT` : un lot de préparation, de restes ou d'aliment de reste…
    pub lot_id: Option<LotId>,
    /// … ou le reste prévu d'un élément cuisiné d'un repas antérieur (R-PLA-13).
    pub leftover_of_item_id: Option<MealItemId>,
    pub food_id: Option<FoodId>,
    pub product_id: Option<ProductId>,
    /// `LOT` : portions servies (ou quantité) ; `FOOD` : quantité, dans la mesure de stock.
    pub amount: Option<Amount>,
    /// `FOOD` : quantité par équivalent adulte (repas habituel, R-PLA-20) plutôt que totale.
    #[serde(default)]
    pub per_adult_equivalent: bool,
    // ----- Lecture seule
    #[serde(default)]
    pub name: String,
    /// Mesure de `amount` ; vide pour des portions.
    pub measure: Option<Measure>,
    /// `COOK` : reste prévu (portions cuisinées − équivalents adulte) et sa date limite (R-PLA-13).
    pub planned_leftover: Option<Amount>,
    pub leftover_deadline: Option<Date>,
    /// Après validation : part mangée, en millièmes (R-PLA-31).
    pub eaten_ratio: Option<Ratio>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Meal {
    pub id: MealId,
    pub date: Date,
    pub slot_id: MealSlotId,
    pub slot_name: String,
    pub status: MealStatus,
    pub available_active_minutes: u32,
    #[schema(value_type = String)]
    pub earliest_start: NaiveTime,
    #[schema(value_type = String)]
    pub ready_by: NaiveTime,
    pub locked: bool,
    pub note: Option<String>,
    pub diners: Vec<Diner>,
    /// Somme des coefficients des convives (R-PLA-11).
    pub adult_equivalents: Ratio,
    pub items: Vec<MealItem>,
    /// Temps actif et temps total des recettes cuisinées sur place.
    pub active_minutes: u32,
    pub total_minutes: u32,
    pub signals: Vec<Signal>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Slot {
    pub id: MealSlotId,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlanDay {
    pub date: Date,
    pub meals: Vec<Meal>,
    /// Créneaux actifs sans repas ce jour-là (R-PLA-50).
    pub empty_slots: Vec<MealSlotId>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Plan {
    pub slots: Vec<Slot>,
    pub days: Vec<PlanDay>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PlanQuery {
    /// Premier jour.
    pub start: Date,
    /// Nombre de jours (7 par défaut, 31 au plus).
    pub days: Option<u32>,
}

/// Tout ce qu'il faut pour présenter des repas.
pub(crate) struct Planner {
    pub kitchen: Kitchen,
    pub slots: Vec<SlotRow>,
    pub members: Vec<MemberRow>,
    pub preferences: Vec<Preference>,
    pub roles: Vec<RoleRow>,
    pub schedules: Vec<ScheduleRow>,
}

impl Planner {
    pub async fn load(conn: &mut SqliteConnection) -> anyhow::Result<Self> {
        let kitchen = Kitchen::load(&mut *conn).await?;
        let slots = db::goals::slots(&mut *conn).await?;
        let members = db::planning::members(&mut *conn).await?;
        let preferences = db::planning::preferences(&mut *conn)
            .await?
            .into_iter()
            .map(|p| Preference { member: p.member_id, target: p.target, level: p.level })
            .collect();
        let roles = db::planning::roles(&mut *conn).await?;
        let schedules = db::planning::schedules(&mut *conn).await?;
        Ok(Self { kitchen, slots, members, preferences, roles, schedules })
    }

    fn slot_name(&self, id: MealSlotId) -> String {
        self.slots.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default()
    }

    /// Horaires du créneau un jour donné ; à défaut de semaine type, un créneau d'une heure à midi.
    pub fn schedule(&self, slot: MealSlotId, date: Date) -> ScheduleRow {
        let weekday = date.weekday().number_from_monday();
        self.schedules.iter().find(|s| s.slot_id == slot && s.weekday == weekday).cloned().unwrap_or(ScheduleRow {
            slot_id: slot,
            weekday,
            available_active_minutes: 20,
            earliest_start: NaiveTime::from_hms_opt(12, 0, 0).unwrap_or_default(),
            ready_by: NaiveTime::from_hms_opt(13, 0, 0).unwrap_or_default(),
            out_of_home: false,
            habitual_meal_id: None,
        })
    }

    fn recipe_name(&self, id: RecipeId) -> String {
        self.kitchen.recipes.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_default()
    }

    fn food_of(&self, stocked: Stocked) -> Option<FoodId> {
        match stocked {
            Stocked::Food(f) => Some(f),
            Stocked::Product(p) => self.kitchen.catalog.products.get(&p).and_then(|p| p.food_id),
        }
    }

    /// Nom et mesure de stock d'un aliment ou d'un produit.
    pub fn stocked(&self, stocked: Stocked) -> (String, Option<Measure>) {
        let catalog = &self.kitchen.catalog;
        match stocked {
            Stocked::Food(f) => catalog
                .foods
                .get(&f)
                .map(|food| (food.name.clone(), Some(catalog.food_settings(food).stock_measure)))
                .unwrap_or_default(),
            Stocked::Product(p) => catalog
                .products
                .get(&p)
                .map(|product| (product.name.clone(), Some(catalog.product_settings(product).stock_measure)))
                .unwrap_or_default(),
        }
    }

    /// Nom d'un lot : la recette (lot de préparation ou de restes), l'aliment ou le produit.
    pub fn lot_name(&self, lot: &LotRow) -> String {
        match (lot.recipe_id, lot.food_id, lot.product_id) {
            (Some(r), ..) => format!("Restes : {}", self.recipe_name(r)),
            (None, Some(f), _) => self.stocked(Stocked::Food(f)).0,
            (None, None, Some(p)) => self.stocked(Stocked::Product(p)).0,
            _ => String::new(),
        }
    }

    /// Cibles d'un élément pour les préférences : la recette, ses sous-recettes, ses aliments et familles.
    fn targets(&self, item: &ItemRow, lots: &HashMap<LotId, LotRow>, items: &[ItemRow]) -> HashSet<Target> {
        let recipe = match item.source {
            ItemSource::Cook => item.recipe_id,
            ItemSource::Lot => item
                .lot_id
                .and_then(|l| lots.get(&l))
                .and_then(|l| l.recipe_id)
                .or_else(|| self.leftover_source(item, items).and_then(|s| s.recipe_id)),
            ItemSource::Food => None,
        };
        let mut out = HashSet::new();
        if let Some(recipe) = recipe {
            out.insert(Target::Recipe(recipe));
            let mut stack = vec![recipe];
            while let Some(r) = stack.pop() {
                for i in self.kitchen.ingredients.iter().filter(|i| i.recipe_id == r) {
                    if out.insert(i.target)
                        && let Target::Recipe(sub) = i.target
                    {
                        stack.push(sub);
                    }
                }
            }
        }
        let food = match item.source {
            ItemSource::Food => item.stocked.and_then(|s| self.food_of(s)),
            ItemSource::Lot => item.lot_id.and_then(|l| lots.get(&l)).and_then(|l| l.food_id),
            ItemSource::Cook => None,
        };
        if let Some(food) = food {
            out.insert(Target::Food(food));
            if let Some(row) = self.kitchen.catalog.foods.get(&food) {
                out.insert(Target::Group(row.group_id));
            }
        }
        out
    }

    fn leftover_source<'a>(&self, item: &ItemRow, items: &'a [ItemRow]) -> Option<&'a ItemRow> {
        item.leftover_of_item_id.and_then(|id| items.iter().find(|i| i.id == id))
    }

    /// Temps (actif, total) des recettes cuisinées sur place ; chacune compte une fois (R-REC-15), elles sont
    /// menées de front.
    fn times(&self, items: &[&ItemRow]) -> (u32, u32) {
        let mut seen = HashSet::new();
        let (mut active, mut total) = (0, 0);
        for recipe in items.iter().filter(|i| i.source == ItemSource::Cook).filter_map(|i| i.recipe_id) {
            if seen.insert(recipe)
                && let Ok((a, t)) = graph::times(&self.kitchen.book, recipe)
            {
                active += a;
                total = total.max(t);
            }
        }
        (active, total)
    }

    /// Présente un repas, avec ses signalements à la date du jour.
    pub fn view(
        &self,
        meal: &MealRow,
        diners: &[DinerRow],
        items: &[ItemRow],
        meals: &HashMap<MealId, MealRow>,
        lots: &HashMap<LotId, LotRow>,
        today: Date,
    ) -> Meal {
        let own: Vec<&ItemRow> = items.iter().filter(|i| i.meal_id == meal.id).collect();
        let own_diners: Vec<&DinerRow> = diners.iter().filter(|d| d.meal_id == meal.id).collect();
        let domain_diners: Vec<DomainDiner> = own_diners
            .iter()
            .map(|d| DomainDiner { member: d.member_id, adult_equivalent: d.adult_equivalent })
            .collect();
        let equivalents = household::adult_equivalents(&domain_diners);
        let (active, total) = self.times(&own);
        let mut refusals = Vec::new();
        let mut views = Vec::new();
        let mut leftover = Vec::new();
        for item in &own {
            let view = self.item_view(item, meal, equivalents, items, lots);
            for member in household::refusals(&domain_diners, &self.preferences, &self.targets(item, lots, items)) {
                refusals.push((member, view.name.clone()));
            }
            if let (Some(planned), Some(deadline)) = (view.planned_leftover, view.leftover_deadline)
                && planned.is_positive()
            {
                let uses: Vec<(Date, Amount)> = items
                    .iter()
                    .filter(|x| x.leftover_of_item_id == Some(item.id))
                    .filter_map(|x| Some((meals.get(&x.meal_id)?.date, x.amount?)))
                    .collect();
                leftover.extend(signals::leftover_signals(planned, deadline, &uses));
            }
            views.push(view);
        }
        let empty_required_roles = self
            .roles
            .iter()
            .filter(|r| r.slot_id == meal.slot_id && r.required && !own.iter().any(|i| i.role_id == Some(r.id)))
            .map(|r| r.name.clone())
            .collect();
        let facts = MealFacts {
            date: meal.date,
            status: meal.status,
            adult_equivalents: equivalents,
            available_active_minutes: meal.available_active_minutes,
            window_minutes: window_minutes(meal.earliest_start, meal.ready_by).unwrap_or(0),
            active_minutes: active,
            total_minutes: total,
            empty_required_roles,
            refusals,
        };
        let mut signals = signals::meal_signals(&facts, today);
        if meal.status == MealStatus::Planned {
            signals.extend(leftover);
        }
        Meal {
            id: meal.id,
            date: meal.date,
            slot_id: meal.slot_id,
            slot_name: self.slot_name(meal.slot_id),
            status: meal.status,
            available_active_minutes: meal.available_active_minutes,
            earliest_start: meal.earliest_start,
            ready_by: meal.ready_by,
            locked: meal.locked,
            note: meal.note.clone(),
            diners: own_diners
                .iter()
                .map(|d| Diner {
                    member_id: d.member_id,
                    guest_label: d.guest_label.clone(),
                    adult_equivalent: d.adult_equivalent,
                    name: match d.member_id {
                        Some(m) => self.members.iter().find(|x| x.id == m).map(|x| x.name.clone()).unwrap_or_default(),
                        None => d.guest_label.clone().unwrap_or_else(|| "Invité".to_owned()),
                    },
                })
                .collect(),
            adult_equivalents: equivalents,
            items: views,
            active_minutes: active,
            total_minutes: total,
            signals,
        }
    }

    fn item_view(
        &self,
        item: &ItemRow,
        meal: &MealRow,
        equivalents: Ratio,
        items: &[ItemRow],
        lots: &HashMap<LotId, LotRow>,
    ) -> MealItem {
        let (food_id, product_id) = match item.stocked {
            Some(Stocked::Food(f)) => (Some(f), None),
            Some(Stocked::Product(p)) => (None, Some(p)),
            None => (None, None),
        };
        let (name, measure) = match item.source {
            ItemSource::Cook => (item.recipe_id.map(|r| self.recipe_name(r)).unwrap_or_default(), None),
            ItemSource::Lot => match item.lot_id.and_then(|l| lots.get(&l)) {
                Some(lot) => (self.lot_name(lot), lot.recipe_id.is_none().then_some(lot.measure)),
                None => {
                    let source = self.leftover_source(item, items);
                    let recipe = source.and_then(|s| s.recipe_id).map(|r| self.recipe_name(r)).unwrap_or_default();
                    (format!("Restes : {recipe}"), None)
                }
            },
            ItemSource::Food => item.stocked.map(|s| self.stocked(s)).unwrap_or_default(),
        };
        let fridge_days =
            item.recipe_id.and_then(|r| self.kitchen.recipes.iter().find(|x| x.id == r)).and_then(|r| r.fridge_days);
        let cooking = item.source == ItemSource::Cook;
        MealItem {
            id: Some(item.id),
            role_id: item.role_id,
            source: item.source,
            recipe_id: item.recipe_id,
            cooked_portions: item.cooked_portions,
            lot_id: item.lot_id,
            leftover_of_item_id: item.leftover_of_item_id,
            food_id,
            product_id,
            amount: item.amount,
            per_adult_equivalent: item.per_adult_equivalent,
            name,
            measure,
            planned_leftover: item.cooked_portions.filter(|_| cooking).map(|c| planned_leftover(c, equivalents)),
            leftover_deadline: cooking.then(|| leftover_deadline(meal.date, fridge_days)),
            eaten_ratio: item.eaten_ratio,
        }
    }
}

async fn lots_of(conn: &mut SqliteConnection, items: &[ItemRow]) -> anyhow::Result<HashMap<LotId, LotRow>> {
    let mut out = HashMap::new();
    let ids: Vec<LotId> = items.iter().flat_map(|i| [i.lot_id, i.leftover_lot_id]).flatten().collect();
    for id in ids {
        if let std::collections::hash_map::Entry::Vacant(entry) = out.entry(id)
            && let Some(lot) = db::stock::find(&mut *conn, id).await?
        {
            entry.insert(lot);
        }
    }
    Ok(out)
}

/// Repas d'une période, jour par jour, avec les créneaux vides (PLA-03, R-PLA-50).
pub async fn plan(db: &Db, query: &PlanQuery, today: Date) -> AppResult<Plan> {
    let days = query.days.unwrap_or(7).clamp(1, 31);
    let end = query.start + Days::new(u64::from(days - 1));
    let mut conn = db.read.acquire().await?;
    let planner = Planner::load(&mut conn).await?;
    let meals = db::planning::meals(&mut *conn, query.start, end).await?;
    let diners = db::planning::diners(&mut *conn, query.start, end).await?;
    let items = db::planning::items(&mut *conn, query.start, end).await?;
    let mut by_id: HashMap<MealId, MealRow> = meals.iter().map(|m| (m.id, m.clone())).collect();
    // Repas hors période qui mangent ou fournissent un reste prévu : leur date sert aux signalements.
    for item in &items {
        if let std::collections::hash_map::Entry::Vacant(entry) = by_id.entry(item.meal_id)
            && let Some(meal) = db::planning::find_meal(&mut *conn, item.meal_id).await?
        {
            entry.insert(meal);
        }
    }
    let lots = lots_of(&mut conn, &items).await?;
    let active: Vec<&SlotRow> = planner.slots.iter().filter(|s| s.active).collect();
    let days = (0..days)
        .map(|offset| {
            let date = query.start + Days::new(u64::from(offset));
            let day_meals: Vec<Meal> = active
                .iter()
                .filter_map(|slot| meals.iter().find(|m| m.date == date && m.slot_id == slot.id))
                .map(|m| planner.view(m, &diners, &items, &by_id, &lots, today))
                .collect();
            let empty_slots =
                active.iter().filter(|s| !day_meals.iter().any(|m| m.slot_id == s.id)).map(|s| s.id).collect();
            PlanDay { date, meals: day_meals, empty_slots }
        })
        .collect();
    Ok(Plan { slots: active.iter().map(|s| Slot { id: s.id, name: s.name.clone() }).collect(), days })
}

pub async fn get(db: &Db, id: MealId, today: Date) -> AppResult<Meal> {
    let mut conn = db.read.acquire().await?;
    let meal = db::planning::find_meal(&mut *conn, id).await?.ok_or(AppError::NotFound("repas"))?;
    let planner = Planner::load(&mut conn).await?;
    let diners = db::planning::diners(&mut *conn, meal.date, meal.date).await?;
    let items = db::planning::items(&mut *conn, meal.date, meal.date).await?;
    let mut by_id: HashMap<MealId, MealRow> = HashMap::from([(meal.id, meal.clone())]);
    for item in &items {
        if let std::collections::hash_map::Entry::Vacant(entry) = by_id.entry(item.meal_id)
            && let Some(other) = db::planning::find_meal(&mut *conn, item.meal_id).await?
        {
            entry.insert(other);
        }
    }
    let lots = lots_of(&mut conn, &items).await?;
    Ok(planner.view(&meal, &diners, &items, &by_id, &lots, today))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewMeal {
    pub date: Date,
    pub slot_id: MealSlotId,
}

/// Crée un repas d'après la semaine type : horaires, convives présents par défaut, hors foyer, repas habituel
/// (R-PLA-3, R-PLA-5, R-PLA-21).
pub async fn create(db: &Db, input: &NewMeal) -> AppResult<MealId> {
    let mut tx = db.write.begin().await?;
    let planner = Planner::load(&mut tx).await?;
    if !planner.slots.iter().any(|s| s.id == input.slot_id) {
        return Err(AppError::NotFound("créneau"));
    }
    let id = create_from_template(&mut tx, &planner, input.date, input.slot_id).await?;
    tx.commit().await?;
    Ok(id)
}

pub(crate) async fn create_from_template(
    conn: &mut SqliteConnection,
    planner: &Planner,
    date: Date,
    slot: MealSlotId,
) -> AppResult<MealId> {
    if db::planning::meal_at(&mut *conn, date, slot).await?.is_some() {
        return Err(DomainError::MealSlotTaken.into());
    }
    let schedule = planner.schedule(slot, date);
    let absent: HashSet<MemberId> = db::planning::presences(&mut *conn)
        .await?
        .into_iter()
        .filter(|p| p.slot_id == slot && p.weekday == schedule.weekday && !p.present)
        .map(|p| p.member_id)
        .collect();
    let members: Vec<household::Member> = planner
        .members
        .iter()
        .map(|m| household::Member {
            id: m.id,
            name: m.name.clone(),
            adult_equivalent: m.adult_equivalent,
            active: m.active,
        })
        .collect();
    let diners = household::default_diners(&members, &absent);
    let equivalents = household::adult_equivalents(&diners);
    let meal = MealRow {
        id: MealId::new(),
        date,
        slot_id: slot,
        status: if schedule.out_of_home { MealStatus::OutOfHome } else { MealStatus::Planned },
        available_active_minutes: schedule.available_active_minutes,
        earliest_start: schedule.earliest_start,
        ready_by: schedule.ready_by,
        locked: false,
        validation_batch_id: None,
        note: None,
    };
    db::planning::upsert_meal(&mut *conn, &meal).await?;
    let rows: Vec<DinerRow> = diners
        .iter()
        .map(|d| DinerRow {
            meal_id: meal.id,
            member_id: d.member,
            guest_label: None,
            adult_equivalent: d.adult_equivalent,
        })
        .collect();
    db::planning::replace_diners(&mut *conn, meal.id, &rows).await?;
    if let Some(habitual) = schedule.habitual_meal_id.filter(|_| !schedule.out_of_home) {
        let habituals = db::planning::habitual_meals(&mut *conn).await?;
        if let Some(h) = habituals.iter().find(|h| h.id == habitual) {
            let mut items = Vec::new();
            for (position, item) in h.items.iter().enumerate() {
                let position = i64::try_from(position).map_err(anyhow::Error::from)?;
                let row = match (item.stocked, item.recipe_id) {
                    // Une recette d'un repas habituel : portions par équivalent adulte (R-PLA-20).
                    (None, Some(recipe)) => ItemRow {
                        source: ItemSource::Cook,
                        recipe_id: Some(recipe),
                        cooked_portions: Some(scale_per_adult(item.amount, equivalents)?).filter(|p| p.is_positive()),
                        ..empty_item(meal.id, position)
                    },
                    (stocked, _) => ItemRow {
                        source: ItemSource::Food,
                        stocked,
                        amount: Some(item.amount),
                        per_adult_equivalent: true,
                        ..empty_item(meal.id, position)
                    },
                };
                if row.source != ItemSource::Cook || row.cooked_portions.is_some() {
                    items.push(row);
                }
            }
            db::planning::replace_items(&mut *conn, meal.id, &items).await?;
        }
    }
    Ok(meal.id)
}

fn empty_item(meal: MealId, position: i64) -> ItemRow {
    ItemRow {
        id: MealItemId::new(),
        meal_id: meal,
        role_id: None,
        position,
        source: ItemSource::Food,
        recipe_id: None,
        cooked_portions: None,
        lot_id: None,
        leftover_of_item_id: None,
        stocked: None,
        amount: None,
        per_adult_equivalent: false,
        eaten_ratio: None,
        leftover_lot_id: None,
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MealInput {
    /// `PLANNED`, `SKIPPED` ou `OUT_OF_HOME` ; `DONE` s'obtient par la validation (PLA-04).
    pub status: MealStatus,
    pub available_active_minutes: u32,
    #[schema(value_type = String, example = "18:30")]
    pub earliest_start: NaiveTime,
    #[schema(value_type = String, example = "19:30")]
    pub ready_by: NaiveTime,
    #[serde(default)]
    pub locked: bool,
    pub note: Option<String>,
    pub diners: Vec<Diner>,
    pub items: Vec<MealItem>,
}

/// Modifie un repas prévu : statut, horaires, convives, éléments (R-PLA-5, R-PLA-11, R-PLA-12). La saisie
/// n'est jamais bloquée par un dépassement : il est signalé (R-PLA-50).
pub async fn update(db: &Db, id: MealId, input: &MealInput) -> AppResult<()> {
    if input.status == MealStatus::Done {
        return Err(DomainError::MealNotPlanned.into());
    }
    window_minutes(input.earliest_start, input.ready_by)?;
    let mut tx = db.write.begin().await?;
    let meal = db::planning::find_meal(&mut *tx, id).await?.ok_or(AppError::NotFound("repas"))?;
    if meal.status == MealStatus::Done {
        return Err(DomainError::MealValidated.into());
    }
    let planner = Planner::load(&mut tx).await?;
    let mut diners = Vec::new();
    let mut seen = HashSet::new();
    for d in &input.diners {
        if !d.adult_equivalent.thousandths().is_positive() {
            return Err(AppError::invalid_field(
                "NOT_POSITIVE",
                "adult_equivalent",
                "un coefficient positif est attendu",
            ));
        }
        if let Some(m) = d.member_id {
            if !planner.members.iter().any(|x| x.id == m) {
                return Err(AppError::NotFound("membre"));
            }
            if !seen.insert(m) {
                continue;
            }
        }
        diners.push(DinerRow {
            meal_id: id,
            member_id: d.member_id,
            guest_label: if d.member_id.is_none() { clean(d.guest_label.as_deref()) } else { None },
            adult_equivalent: d.adult_equivalent,
        });
    }
    let existing = db::planning::meal_items(&mut *tx, id).await?;
    let mut items = Vec::new();
    for (position, item) in input.items.iter().enumerate() {
        let position = i64::try_from(position).map_err(anyhow::Error::from)?;
        let id_kept = item.id.filter(|i| existing.iter().any(|e| e.id == *i));
        let row = item_row(&mut tx, &planner, &meal, item, id_kept.unwrap_or_else(MealItemId::new), position).await?;
        items.push(row);
    }
    let updated = MealRow {
        status: input.status,
        available_active_minutes: input.available_active_minutes,
        earliest_start: input.earliest_start,
        ready_by: input.ready_by,
        locked: input.locked,
        note: clean(input.note.as_deref()),
        ..meal
    };
    db::planning::upsert_meal(&mut *tx, &updated).await?;
    db::planning::replace_diners(&mut tx, id, &diners).await?;
    db::planning::replace_items(&mut tx, id, &items).await?;
    tx.commit().await?;
    Ok(())
}

/// Vérifie un élément saisi et le prépare pour la base (R-PLA-12).
async fn item_row(
    conn: &mut SqliteConnection,
    planner: &Planner,
    meal: &MealRow,
    item: &MealItem,
    id: MealItemId,
    position: i64,
) -> AppResult<ItemRow> {
    let base = ItemRow { id, role_id: item.role_id, ..empty_item(meal.id, position) };
    if item.role_id.is_some_and(|r| !planner.roles.iter().any(|x| x.id == r && x.slot_id == meal.slot_id)) {
        return Err(AppError::NotFound("rôle"));
    }
    let positive = |a: Option<Amount>| a.filter(|a| a.is_positive()).ok_or(DomainError::NotPositive);
    Ok(match item.source {
        ItemSource::Cook => {
            let recipe = item.recipe_id.ok_or(DomainError::MealItemContent)?;
            if !planner.kitchen.recipes.iter().any(|r| r.id == recipe) {
                return Err(AppError::NotFound("recette"));
            }
            ItemRow {
                source: ItemSource::Cook,
                recipe_id: Some(recipe),
                cooked_portions: Some(positive(item.cooked_portions)?),
                ..base
            }
        }
        ItemSource::Lot => {
            let amount = positive(item.amount)?;
            match (item.lot_id, item.leftover_of_item_id) {
                (Some(lot), None) => {
                    db::stock::find(&mut *conn, lot).await?.ok_or(AppError::NotFound("lot"))?;
                    ItemRow { source: ItemSource::Lot, lot_id: Some(lot), amount: Some(amount), ..base }
                }
                (None, Some(source)) => {
                    let found =
                        db::planning::find_item(&mut *conn, source).await?.ok_or(AppError::NotFound("reste"))?;
                    let source_meal =
                        db::planning::find_meal(&mut *conn, found.meal_id).await?.ok_or(AppError::NotFound("repas"))?;
                    // Un reste prévu vient d'une recette cuisinée à un repas antérieur : un jour d'avant, ou un
                    // créneau plus tôt le même jour.
                    let position = |slot: MealSlotId| planner.slots.iter().find(|s| s.id == slot).map(|s| s.position);
                    let earlier =
                        (source_meal.date, position(source_meal.slot_id)) < (meal.date, position(meal.slot_id));
                    if found.source != ItemSource::Cook || !earlier {
                        return Err(DomainError::MealItemContent.into());
                    }
                    ItemRow {
                        source: ItemSource::Lot,
                        leftover_of_item_id: Some(source),
                        // Une fois le repas d'origine validé, ses restes sont un lot.
                        lot_id: found.leftover_lot_id,
                        amount: Some(amount),
                        ..base
                    }
                }
                _ => return Err(DomainError::MealItemContent.into()),
            }
        }
        ItemSource::Food => {
            let stocked = match (item.food_id, item.product_id) {
                (Some(f), None) => {
                    planner.kitchen.catalog.foods.get(&f).ok_or(AppError::NotFound("aliment"))?;
                    Stocked::Food(f)
                }
                (None, Some(p)) => {
                    planner.kitchen.catalog.products.get(&p).ok_or(AppError::NotFound("produit"))?;
                    Stocked::Product(p)
                }
                _ => return Err(DomainError::MealItemContent.into()),
            };
            ItemRow {
                source: ItemSource::Food,
                stocked: Some(stocked),
                amount: Some(positive(item.amount)?),
                per_adult_equivalent: item.per_adult_equivalent,
                ..base
            }
        }
    })
}

/// Supprime un repas prévu ; un repas validé s'annule d'abord (R-PLA-34).
pub async fn delete(db: &Db, id: MealId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let meal = db::planning::find_meal(&mut *tx, id).await?.ok_or(AppError::NotFound("repas"))?;
    if meal.status == MealStatus::Done {
        return Err(DomainError::MealValidated.into());
    }
    db::planning::delete_meal(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopyInput {
    /// Premier jour de la semaine copiée.
    pub from: Date,
    /// Premier jour de la semaine de destination.
    pub to: Date,
    /// Nombre de jours (7 par défaut).
    pub days: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CopyResult {
    pub created: u32,
    /// Repas non copiés : le créneau de destination avait déjà un repas.
    pub skipped: u32,
}

/// Copie une période comme point de départ (PLA-13) : chaque repas devient un repas prévu, aux convives par
/// défaut de la destination, avec ses recettes cuisinées et ses aliments ; les parts de lots et de restes ne
/// sont pas copiées, et un créneau déjà rempli est laissé tel quel.
pub async fn copy(db: &Db, input: &CopyInput) -> AppResult<CopyResult> {
    let days = input.days.unwrap_or(7).clamp(1, 31);
    let end = input.from + Days::new(u64::from(days - 1));
    let mut tx = db.write.begin().await?;
    let planner = Planner::load(&mut tx).await?;
    let meals = db::planning::meals(&mut *tx, input.from, end).await?;
    let items = db::planning::items(&mut *tx, input.from, end).await?;
    let offset = (input.to - input.from).num_days();
    let mut result = CopyResult { created: 0, skipped: 0 };
    for meal in meals.iter().filter(|m| m.status != MealStatus::Skipped) {
        let date = meal.date + chrono::Duration::days(offset);
        if db::planning::meal_at(&mut *tx, date, meal.slot_id).await?.is_some() {
            result.skipped += 1;
            continue;
        }
        let id = create_from_template(&mut tx, &planner, date, meal.slot_id).await?;
        let copied: Vec<ItemRow> = items
            .iter()
            .filter(|i| i.meal_id == meal.id && i.source != ItemSource::Lot)
            .enumerate()
            .map(|(position, i)| ItemRow {
                eaten_ratio: None,
                leftover_lot_id: None,
                ..ItemRow { id: MealItemId::new(), meal_id: id, position: position as i64, ..i.clone() }
            })
            .collect();
        if !copied.is_empty() {
            db::planning::replace_items(&mut tx, id, &copied).await?;
        }
        result.created += 1;
    }
    tx.commit().await?;
    Ok(result)
}

/// Besoins d'une part de reste ou de lot de préparation : `portions` de la recette.
pub fn portion_needs(
    kitchen: &Kitchen,
    recipe: RecipeId,
    portions: Amount,
) -> Result<Vec<foodflow_domain::recipes::graph::Need>, DomainError> {
    let Some(row) = kitchen.recipes.iter().find(|r| r.id == recipe) else { return Ok(Vec::new()) };
    expand(&kitchen.book, recipe, foodflow_domain::recipes::scaling::factor(portions, &row.yield_)?)
}

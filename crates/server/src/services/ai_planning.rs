//! Planning de menus par l'IA (PLA-12, ADR-0014) : aperçu des données envoyées, proposition contrôlée par le
//! domaine (R-PLA-67), recettes nouvelles en brouillon (R-PLA-66), acceptation repas par repas (R-PLA-64).

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Days};
use foodflow_domain::ai::plan::{AiItem, AiPlan, PlanContext, Rejection, check_plan};
use foodflow_domain::nutrition::{Criterion, CriterionKind, Expression, Period};
use foodflow_domain::planning::household::{self, Diner as DomainDiner, Preference};
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::recipes::{RecipeKind, Target};
use foodflow_domain::shared::ids::{LotId, MealId, MealItemId, MealSlotId, MemberId, RecipeDraftId, RecipeId};
use foodflow_domain::{Amount, Date, DomainError, Ratio};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::ai::{self, AiPreview, SYSTEM, decimal, household_text, lots_text, priority_lots, recipe_schema};
use super::meals::{Planner, create_from_template};
use super::recipes::Kitchen;
use super::stock::{self, LotQuery};
use crate::clients::ai::{AiClient, AiRequest};
use crate::clients::photos::PhotoSearch;
use crate::clients::web::Fetcher;
use crate::config::DataDir;
use crate::db::generation::{RunId, RunRow};
use crate::db::planning::{DinerRow, ItemRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const WEEKDAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PlanIdeaInput {
    /// Premier jour.
    pub start: Date,
    /// Nombre de jours (7 par défaut, 14 au plus).
    pub days: Option<u32>,
    /// Créneaux à remplir ; tous les créneaux actifs si vide.
    #[serde(default)]
    pub slot_ids: Vec<MealSlotId>,
    /// Consigne libre (« semaine italienne », « plus léger »).
    pub prompt: Option<String>,
}

/// Un créneau à remplir et ses convives.
struct OpenSlot {
    date: Date,
    slot: MealSlotId,
    diners: Vec<DomainDiner>,
}

/// Contexte complet d'une demande : le texte envoyé et de quoi contrôler la réponse.
struct Prepared {
    request: AiRequest,
    context: PlanContext,
    end: Date,
}

/// Cibles d'une recette pour les préférences : elle, ses sous-recettes, leurs aliments et familles.
fn recipe_targets(kitchen: &Kitchen, recipe: RecipeId) -> HashSet<Target> {
    let mut out = HashSet::from([Target::Recipe(recipe)]);
    let mut stack = vec![recipe];
    while let Some(r) = stack.pop() {
        for i in kitchen.ingredients.iter().filter(|i| i.recipe_id == r) {
            if out.insert(i.target) {
                match i.target {
                    Target::Recipe(sub) => stack.push(sub),
                    Target::Food(food) => {
                        if let Some(row) = kitchen.catalog.foods.get(&food) {
                            out.insert(Target::Group(row.group_id));
                        }
                    }
                    Target::Group(_) => {}
                }
            }
        }
    }
    out
}

/// Un critère d'objectif en une ligne, pour l'IA.
fn criterion_text(c: &Criterion) -> String {
    let what = match c.kind {
        CriterionKind::Energy => "énergie (kcal)".to_owned(),
        CriterionKind::Nutrient => match c.expression {
            Expression::PercentEnergy => {
                format!("{} en part de l'énergie (‰)", c.nutrient_code.as_deref().unwrap_or(""))
            }
            Expression::Absolute => format!("{} (mg)", c.nutrient_code.as_deref().unwrap_or("")),
        },
        CriterionKind::GroupPortions => format!("portions (millièmes) de {:?}", c.nutrition_groups),
        CriterionKind::GroupAmount => format!("quantité (mg) de {:?}", c.nutrition_groups),
        CriterionKind::GroupFrequency => format!("repas avec {:?}", c.nutrition_groups),
        CriterionKind::RecipeRepeat => "jours entre deux fois la même recette".to_owned(),
        CriterionKind::FamilyRepeat => "repas avec la même famille d'aliments".to_owned(),
    };
    let period = match c.period {
        Period::Day => "par jour et par équivalent adulte",
        Period::Week => "par semaine",
    };
    let min = c.min_value.map(|v| format!(" au moins {v}")).unwrap_or_default();
    let max = c.max_value.map(|v| format!(" au plus {v}")).unwrap_or_default();
    format!("- {what}{min}{max}, {period}")
}

fn plan_schema(max_new: u32) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["meals", "new_recipes"],
        "properties": {
            "meals": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["date", "slot_id", "items", "reason"],
                    "properties": {
                        "date": { "type": "string", "description": "AAAA-MM-JJ" },
                        "slot_id": { "type": "string" },
                        "items": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["recipe_id", "new_recipe", "lot_id"],
                                "properties": {
                                    "recipe_id": { "type": ["string", "null"] },
                                    "new_recipe": { "type": ["integer", "null"], "description": "rang dans new_recipes" },
                                    "lot_id": { "type": ["string", "null"] }
                                }
                            }
                        },
                        "reason": { "type": "string" }
                    }
                }
            },
            "new_recipes": { "type": "array", "items": recipe_schema(), "maxItems": max_new }
        }
    })
}

/// Construit la demande, à l'identique pour l'aperçu et pour l'envoi (R-PLA-66).
async fn prepare(conn: &mut SqliteConnection, input: &PlanIdeaInput, today: Date) -> AppResult<Prepared> {
    let days = input.days.unwrap_or(7).clamp(1, 14);
    let end = input.start + Days::new(u64::from(days - 1));
    let planner = Planner::load(&mut *conn).await?;
    let settings = ai::settings(&mut *conn, today).await?;
    let slots: Vec<_> = planner
        .slots
        .iter()
        .filter(|s| s.active && (input.slot_ids.is_empty() || input.slot_ids.contains(&s.id)))
        .collect();
    if slots.is_empty() {
        return Err(AppError::invalid_field("REQUIRED", "slot_ids", "choisir au moins un créneau"));
    }
    let meals = db::planning::meals(&mut *conn, input.start, end).await?;
    let diners = db::planning::diners(&mut *conn, input.start, end).await?;
    let items = db::planning::items(&mut *conn, input.start, end).await?;
    let presences = db::planning::presences(&mut *conn).await?;
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

    // Créneaux à remplir : sans repas, ou avec un repas prévu, vide et non verrouillé (R-PLA-60).
    let mut open = Vec::new();
    for offset in 0..days {
        let date = input.start + Days::new(u64::from(offset));
        for slot in &slots {
            match meals.iter().find(|m| m.date == date && m.slot_id == slot.id) {
                None => {
                    let schedule = planner.schedule(slot.id, date);
                    if schedule.out_of_home || schedule.habitual_meal_id.is_some() {
                        continue;
                    }
                    let absent: HashSet<MemberId> = presences
                        .iter()
                        .filter(|p| p.slot_id == slot.id && p.weekday == schedule.weekday && !p.present)
                        .map(|p| p.member_id)
                        .collect();
                    open.push(OpenSlot { date, slot: slot.id, diners: household::default_diners(&members, &absent) });
                }
                Some(meal)
                    if meal.status == MealStatus::Planned
                        && !meal.locked
                        && !items.iter().any(|i| i.meal_id == meal.id) =>
                {
                    let own: Vec<DomainDiner> = diners
                        .iter()
                        .filter(|d| d.meal_id == meal.id)
                        .map(|d| DomainDiner { member: d.member_id, adult_equivalent: d.adult_equivalent })
                        .collect();
                    open.push(OpenSlot { date, slot: slot.id, diners: own });
                }
                Some(_) => {}
            }
        }
    }
    if open.is_empty() {
        return Err(AppError::invalid("NOTHING_TO_PLAN", "aucun créneau à remplir sur cette période"));
    }

    let kitchen = &planner.kitchen;
    let (household, letters) = household_text(&mut *conn, kitchen).await?;
    let letter = |m: MemberId| letters.iter().find(|(id, _)| *id == m).map(|(_, l)| l.clone()).unwrap_or_default();
    let candidates: Vec<_> =
        kitchen.recipes.iter().filter(|r| !r.archived && r.kind != RecipeKind::Base).take(200).collect();
    let preferences: Vec<Preference> = planner.preferences.clone();
    let targets: HashMap<RecipeId, HashSet<Target>> =
        candidates.iter().map(|r| (r.id, recipe_targets(kitchen, r.id))).collect();
    let mut refused: HashMap<(Date, MealSlotId), HashSet<RecipeId>> = HashMap::new();
    let mut slot_lines = Vec::new();
    for o in &open {
        let name = planner.slots.iter().find(|s| s.id == o.slot).map(|s| s.name.to_lowercase()).unwrap_or_default();
        let schedule = planner.schedule(o.slot, o.date);
        let who: Vec<String> = o
            .diners
            .iter()
            .map(|d| {
                d.member
                    .map(letter)
                    .unwrap_or_else(|| format!("invité ({})", decimal(d.adult_equivalent.thousandths())))
            })
            .collect();
        let roles: Vec<String> = planner
            .roles
            .iter()
            .filter(|r| r.slot_id == o.slot)
            .map(|r| if r.required { format!("{} (obligatoire)", r.name) } else { r.name.clone() })
            .collect();
        slot_lines.push(format!(
            "- {} ({} {}) [créneau {}] : {} min actives, prêt à {} ; convives {} ({} éq. adulte){}",
            o.date,
            WEEKDAYS[o.date.weekday().num_days_from_monday() as usize],
            name,
            o.slot,
            schedule.available_active_minutes,
            schedule.ready_by.format("%H:%M"),
            who.join(", "),
            decimal(household::adult_equivalents(&o.diners).thousandths()),
            if roles.is_empty() { String::new() } else { format!(" ; rôles : {}", roles.join(", ")) }
        ));
        let set: HashSet<RecipeId> = targets
            .iter()
            .filter(|(_, t)| !household::refusals(&o.diners, &preferences, t).is_empty())
            .map(|(r, _)| *r)
            .collect();
        refused.insert((o.date, o.slot), set);
    }
    let recipe_lines: Vec<String> = candidates
        .iter()
        .map(|r| {
            let summary = kitchen.summary(r, None);
            let energy = summary.energy.map(|e| format!(", {} kcal/portion", e / 1_000)).unwrap_or_default();
            let keeps = r.fridge_days.map(|d| format!(", se garde {d} j")).unwrap_or_default();
            format!(
                "- [recette {}] {} ({:?}, {} min actives, {} min au total{energy}{keeps})",
                r.id, r.name, r.kind, summary.times.total_active_minutes, summary.times.total_minutes
            )
        })
        .collect();
    let lots =
        stock::list_in(&mut *conn, &LotQuery { location_id: None, food_id: None, include_depleted: false }, today)
            .await?;
    let priority = priority_lots(&lots);
    let leftovers: HashSet<LotId> = priority.iter().filter(|l| l.recipe_id.is_some()).map(|l| l.id).collect();
    let goals = db::goals::goals(&mut *conn).await?;
    let dated: Vec<(Date, _)> = goals.iter().map(|g| (g.valid_from, g.id)).collect();
    let criteria: Vec<String> = match foodflow_domain::nutrition::in_force(&dated, input.start) {
        Some(id) => db::goals::criteria(&mut *conn)
            .await?
            .iter()
            .filter(|c| c.owner == db::goals::Owner::Goal(*id))
            .map(|c| criterion_text(&c.criterion))
            .collect(),
        None => vec![],
    };
    let max_new = settings.new_recipes_per_plan;
    let prompt = input.prompt.as_deref().map(str::trim).filter(|p| !p.is_empty());
    let user = [
        household,
        format!("Créneaux à remplir :\n{}", slot_lines.join("\n")),
        if recipe_lines.is_empty() {
            "Recettes connues : aucune.".to_owned()
        } else {
            format!("Recettes connues :\n{}", recipe_lines.join("\n"))
        },
        lots_text("À utiliser en priorité (les restes se servent par leur identifiant de lot)", &priority, true),
        if criteria.is_empty() {
            "Objectif nutritionnel : aucun.".to_owned()
        } else {
            format!("Objectif nutritionnel du foyer :\n{}", criteria.join("\n"))
        },
        format!(
            "Propose un repas pour chaque créneau à remplir : des recettes connues par leur identifiant, des restes \
             par leur identifiant de lot, ou au plus {max_new} recette(s) nouvelle(s) dans new_recipes, référencées \
             par leur rang (0, 1…). Varie les plats, respecte les temps disponibles et les refus des convives, \
             utilise d'abord les restes et ce qui est à consommer en priorité, vise l'objectif nutritionnel. \
             Pour chaque repas, une justification courte.{}",
            prompt.map(|p| format!(" Consigne du foyer : {p}")).unwrap_or_default()
        ),
    ]
    .join("\n\n");
    let context = PlanContext {
        open: open.iter().map(|o| (o.date, o.slot)).collect(),
        recipes: candidates.iter().map(|r| r.id).collect(),
        lots: leftovers,
        refused,
        max_new_recipes: max_new as usize,
    };
    Ok(Prepared {
        request: AiRequest {
            system: SYSTEM.to_owned(),
            user,
            schema_name: "planning".to_owned(),
            schema: plan_schema(max_new),
        },
        context,
        end,
    })
}

/// Aperçu de ce qui sera envoyé pour un planning (R-PLA-66).
pub async fn preview(db: &Db, input: &PlanIdeaInput, today: Date) -> AppResult<AiPreview> {
    let mut conn = db.read.acquire().await?;
    let prepared = prepare(&mut conn, input, today).await?;
    Ok(AiPreview { system: prepared.request.system, user: prepared.request.user })
}

// ===================================================================== Proposition enregistrée

/// Une recette nouvelle de la proposition : son brouillon, puis la recette créée à sa validation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NewRecipe {
    pub index: usize,
    pub name: String,
    pub draft_id: Option<RecipeDraftId>,
    pub recipe_id: Option<RecipeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMeal {
    date: Date,
    slot_id: MealSlotId,
    items: Vec<AiItem>,
    reason: String,
    accepted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    meals: Vec<StoredMeal>,
    new_recipes: Vec<NewRecipe>,
    rejected: Vec<Rejection>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProposedItem {
    pub name: String,
    pub recipe_id: Option<RecipeId>,
    pub lot_id: Option<LotId>,
    /// Recette nouvelle encore en brouillon : à relire avant d'accepter ce repas.
    pub draft_id: Option<RecipeDraftId>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ProposedMeal {
    pub date: Date,
    pub slot_id: MealSlotId,
    pub slot_name: String,
    pub reason: String,
    pub items: Vec<ProposedItem>,
    pub accepted: bool,
    /// Toutes ses recettes existent : il peut être accepté.
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RejectedItem {
    pub date: Option<Date>,
    pub slot_name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Proposal {
    pub id: RunId,
    pub start: Date,
    pub end: Date,
    pub prompt: Option<String>,
    /// `PENDING`, `PARTIALLY_ACCEPTED`, `ACCEPTED`, `DISCARDED`.
    pub status: String,
    pub meals: Vec<ProposedMeal>,
    pub new_recipes: Vec<NewRecipe>,
    pub rejected: Vec<RejectedItem>,
    pub created_at: String,
}

fn view(row: &RunRow, stored: &Stored, planner: &Planner, lots: &HashMap<LotId, String>) -> Proposal {
    let slot_name =
        |s: MealSlotId| planner.slots.iter().find(|x| x.id == s).map(|x| x.name.clone()).unwrap_or_default();
    let recipe_name = |r: RecipeId| planner.kitchen.recipes.iter().find(|x| x.id == r).map(|x| x.name.clone());
    let meals = stored
        .meals
        .iter()
        .map(|m| {
            let items: Vec<ProposedItem> = m
                .items
                .iter()
                .map(|i| match (i.recipe_id, i.new_recipe, i.lot_id) {
                    (Some(r), ..) => ProposedItem {
                        name: recipe_name(r).unwrap_or_else(|| "Recette supprimée".into()),
                        recipe_id: Some(r),
                        lot_id: None,
                        draft_id: None,
                        ready: recipe_name(r).is_some(),
                    },
                    (None, Some(n), _) => {
                        let new = stored.new_recipes.iter().find(|x| x.index == n);
                        ProposedItem {
                            name: new.map(|x| x.name.clone()).unwrap_or_default(),
                            recipe_id: new.and_then(|x| x.recipe_id),
                            lot_id: None,
                            draft_id: new.and_then(|x| x.draft_id),
                            ready: new.is_some_and(|x| x.recipe_id.is_some()),
                        }
                    }
                    (None, None, Some(l)) => ProposedItem {
                        name: lots.get(&l).cloned().unwrap_or_else(|| "Lot épuisé".into()),
                        recipe_id: None,
                        lot_id: Some(l),
                        draft_id: None,
                        ready: lots.contains_key(&l),
                    },
                    _ => ProposedItem {
                        name: String::new(),
                        recipe_id: None,
                        lot_id: None,
                        draft_id: None,
                        ready: false,
                    },
                })
                .collect();
            ProposedMeal {
                date: m.date,
                slot_id: m.slot_id,
                slot_name: slot_name(m.slot_id),
                reason: m.reason.clone(),
                ready: items.iter().all(|i| i.ready),
                items,
                accepted: m.accepted,
            }
        })
        .collect();
    Proposal {
        id: row.id,
        start: row.period_start,
        end: row.period_end,
        prompt: row.prompt.clone(),
        status: row.status.clone(),
        meals,
        new_recipes: stored.new_recipes.clone(),
        rejected: stored
            .rejected
            .iter()
            .map(|r| RejectedItem { date: r.date, slot_name: r.slot_id.map(slot_name), reason: r.reason.clone() })
            .collect(),
        created_at: row.created_at.clone(),
    }
}

async fn load_view(conn: &mut SqliteConnection, row: &RunRow, today: Date) -> AppResult<Proposal> {
    let stored: Stored = serde_json::from_str(&row.proposal).map_err(anyhow::Error::from)?;
    let planner = Planner::load(&mut *conn).await?;
    let lots: HashMap<LotId, String> =
        stock::list_in(&mut *conn, &LotQuery { location_id: None, food_id: None, include_depleted: false }, today)
            .await?
            .into_iter()
            .map(|l| (l.id, l.name))
            .collect();
    Ok(view(row, &stored, &planner, &lots))
}

/// Demande un planning à l'IA (PLA-12) : la réponse est contrôlée, ses recettes nouvelles deviennent des
/// brouillons, et rien n'est écrit dans les repas avant l'acceptation (R-PLA-64).
pub async fn generate(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    ai_client: &dyn AiClient,
    input: &PlanIdeaInput,
    today: Date,
) -> AppResult<Proposal> {
    let (prepared, config) = {
        let mut tx = db.write.begin().await?;
        let prepared = prepare(&mut tx, input, today).await?;
        let config = ai::reserve_call(&mut tx, today).await?;
        tx.commit().await?;
        (prepared, config)
    };
    let content = ai_client
        .complete(&config, &prepared.request)
        .await
        .map_err(|e| AppError::invalid("AI_UNAVAILABLE", e.to_string()))?;
    let plan: AiPlan = serde_json::from_str(&content)
        .map_err(|_| AppError::from(DomainError::AiResponse("le planning ne suit pas le format demandé")))?;
    let checked = check_plan(plan, &prepared.context);
    let mut rejected = checked.rejected;
    let mut new_recipes = Vec::new();
    for (index, recipe) in &checked.new_recipes {
        match ai::draft_from(db, data, fetcher, photos, recipe).await {
            Ok(draft) => new_recipes.push(NewRecipe {
                index: *index,
                name: recipe.name.clone(),
                draft_id: Some(draft.id),
                recipe_id: None,
            }),
            Err(e) => {
                rejected.push(Rejection { date: None, slot_id: None, reason: format!("« {} » : {e}", recipe.name) })
            }
        }
    }
    let stored = Stored {
        meals: checked
            .meals
            .into_iter()
            .map(|m| StoredMeal { date: m.date, slot_id: m.slot_id, items: m.items, reason: m.reason, accepted: false })
            .collect(),
        new_recipes,
        rejected,
    };
    let row = RunRow {
        id: RunId::new(),
        period_start: input.start,
        period_end: prepared.end,
        prompt: input.prompt.clone().filter(|p| !p.trim().is_empty()),
        request_payload: Some(json!({ "system": prepared.request.system, "user": prepared.request.user }).to_string()),
        response_payload: Some(content),
        proposal: serde_json::to_string(&stored).map_err(anyhow::Error::from)?,
        status: "PENDING".into(),
        created_at: String::new(),
    };
    let mut conn = db.write.acquire().await?;
    db::generation::insert(&mut *conn, &row).await?;
    let saved = db::generation::find(&mut *conn, row.id).await?.ok_or(AppError::NotFound("proposition"))?;
    load_view(&mut conn, &saved, today).await
}

pub async fn get(db: &Db, id: RunId, today: Date) -> AppResult<Proposal> {
    let mut conn = db.read.acquire().await?;
    let row = db::generation::find(&mut *conn, id).await?.ok_or(AppError::NotFound("proposition"))?;
    load_view(&mut conn, &row, today).await
}

/// Propositions encore ouvertes.
pub async fn list(db: &Db, today: Date) -> AppResult<Vec<Proposal>> {
    let mut conn = db.read.acquire().await?;
    let mut out = Vec::new();
    for row in db::generation::open(&mut *conn).await? {
        out.push(load_view(&mut conn, &row, today).await?);
    }
    Ok(out)
}

/// À la validation d'un brouillon d'une recette nouvelle, les propositions qui l'attendaient la retiennent.
pub async fn draft_committed(db: &Db, draft: RecipeDraftId, recipe: RecipeId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    for row in db::generation::open(&mut *tx).await? {
        let mut stored: Stored = serde_json::from_str(&row.proposal).map_err(anyhow::Error::from)?;
        let mut changed = false;
        for new in stored.new_recipes.iter_mut().filter(|n| n.draft_id == Some(draft)) {
            new.recipe_id = Some(recipe);
            changed = true;
        }
        if changed {
            let proposal = serde_json::to_string(&stored).map_err(anyhow::Error::from)?;
            db::generation::update(&mut *tx, row.id, &proposal, &row.status).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
pub struct MealKey {
    pub date: Date,
    pub slot_id: MealSlotId,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AcceptInput {
    /// Repas à accepter ; tous ceux qui sont prêts si vide.
    #[serde(default)]
    pub meals: Vec<MealKey>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AcceptResult {
    pub accepted: u32,
    /// Repas non acceptés, avec la raison.
    pub skipped: Vec<String>,
}

/// Accepte une proposition, en tout ou repas par repas (R-PLA-64) : chaque repas est créé d'après la semaine
/// type (ou complète un repas prévu resté vide), ses recettes cuisinées pour ses convives.
pub async fn accept(db: &Db, id: RunId, input: &AcceptInput, today: Date) -> AppResult<AcceptResult> {
    let mut tx = db.write.begin().await?;
    let row = db::generation::find(&mut *tx, id).await?.ok_or(AppError::NotFound("proposition"))?;
    if row.status == "ACCEPTED" || row.status == "DISCARDED" {
        return Err(AppError::invalid("PROPOSAL_CLOSED", "cette proposition est close"));
    }
    let mut stored: Stored = serde_json::from_str(&row.proposal).map_err(anyhow::Error::from)?;
    let planner = Planner::load(&mut tx).await?;
    let lots: HashMap<LotId, foodflow_domain::Amount> =
        stock::list_in(&mut tx, &LotQuery { location_id: None, food_id: None, include_depleted: false }, today)
            .await?
            .into_iter()
            .filter_map(|l| Some((l.id, l.quantity?)))
            .collect();
    let mut result = AcceptResult { accepted: 0, skipped: vec![] };
    let new_recipes = stored.new_recipes.clone();
    for meal in stored.meals.iter_mut().filter(|m| !m.accepted) {
        if !input.meals.is_empty() && !input.meals.iter().any(|k| k.date == meal.date && k.slot_id == meal.slot_id) {
            continue;
        }
        let label = format!(
            "{} {}",
            meal.date.format("%d/%m"),
            planner.slots.iter().find(|s| s.id == meal.slot_id).map(|s| s.name.clone()).unwrap_or_default()
        );
        // Recettes : connues, ou nouvelles dont le brouillon est validé (R-PLA-66).
        let mut recipes = Vec::new();
        let mut leftovers = Vec::new();
        let mut missing = None;
        for item in &meal.items {
            match (item.recipe_id, item.new_recipe, item.lot_id) {
                (Some(r), ..) if planner.kitchen.recipes.iter().any(|x| x.id == r) => recipes.push(r),
                (None, Some(n), _) => match new_recipes.iter().find(|x| x.index == n).and_then(|x| x.recipe_id) {
                    Some(r) => recipes.push(r),
                    None => missing = Some("une recette nouvelle est encore en brouillon"),
                },
                (None, None, Some(l)) if lots.contains_key(&l) => leftovers.push(l),
                _ => missing = Some("une recette ou un reste n'existe plus"),
            }
        }
        if let Some(reason) = missing {
            result.skipped.push(format!("{label} : {reason}"));
            continue;
        }
        let meal_id = match db::planning::meal_at(&mut *tx, meal.date, meal.slot_id).await? {
            Some(existing) => {
                let found = db::planning::find_meal(&mut *tx, existing).await?.ok_or(AppError::NotFound("repas"))?;
                let filled = !db::planning::meal_items(&mut *tx, existing).await?.is_empty();
                if found.status != MealStatus::Planned || found.locked || filled {
                    result.skipped.push(format!("{label} : le repas a été rempli entre-temps"));
                    continue;
                }
                existing
            }
            None => create_from_template(&mut tx, &planner, meal.date, meal.slot_id).await?,
        };
        let diners: Vec<DinerRow> = db::planning::diners(&mut *tx, meal.date, meal.date)
            .await?
            .into_iter()
            .filter(|d| d.meal_id == meal_id)
            .collect();
        let equivalents = Ratio::from_thousandths(diners.iter().map(|d| d.adult_equivalent.thousandths()).sum());
        let portions = Amount::from_raw(equivalents.thousandths().max(1_000));
        let roles: Vec<_> = planner.roles.iter().filter(|r| r.slot_id == meal.slot_id).collect();
        let mut taken = HashSet::new();
        let mut rows = Vec::new();
        let mut role_for = |kind: Option<RecipeKind>| {
            let role = roles
                .iter()
                .find(|r| !taken.contains(&r.id) && kind.is_some_and(|k| r.recipe_kinds.contains(&k)))
                .or_else(|| roles.iter().find(|r| !taken.contains(&r.id) && r.required));
            role.map(|r| {
                taken.insert(r.id);
                r.id
            })
        };
        for lot in &leftovers {
            let available = lots.get(lot).copied().unwrap_or(portions);
            rows.push(ItemRow {
                id: MealItemId::new(),
                meal_id,
                role_id: role_for(None),
                position: rows.len() as i64,
                source: ItemSource::Lot,
                recipe_id: None,
                cooked_portions: None,
                lot_id: Some(*lot),
                leftover_of_item_id: None,
                stocked: None,
                amount: Some(if available.raw() < portions.raw() { available } else { portions }),
                per_adult_equivalent: false,
                eaten_ratio: None,
                leftover_lot_id: None,
            });
        }
        for recipe in &recipes {
            let kind = planner.kitchen.recipes.iter().find(|x| x.id == *recipe).map(|x| x.kind);
            rows.push(ItemRow {
                id: MealItemId::new(),
                meal_id,
                role_id: role_for(kind),
                position: rows.len() as i64,
                source: ItemSource::Cook,
                recipe_id: Some(*recipe),
                cooked_portions: Some(portions),
                lot_id: None,
                leftover_of_item_id: None,
                stocked: None,
                amount: None,
                per_adult_equivalent: false,
                eaten_ratio: None,
                leftover_lot_id: None,
            });
        }
        db::planning::replace_items(&mut tx, meal_id, &rows).await?;
        let _: MealId = meal_id;
        meal.accepted = true;
        result.accepted += 1;
    }
    let status = if stored.meals.iter().all(|m| m.accepted) {
        "ACCEPTED"
    } else if stored.meals.iter().any(|m| m.accepted) {
        "PARTIALLY_ACCEPTED"
    } else {
        "PENDING"
    };
    let proposal = serde_json::to_string(&stored).map_err(anyhow::Error::from)?;
    db::generation::update(&mut *tx, id, &proposal, status).await?;
    tx.commit().await?;
    Ok(result)
}

/// Écarte une proposition : rien n'est écrit (R-PLA-64). Ses brouillons restent, à relire ou abandonner.
pub async fn discard(db: &Db, id: RunId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let row = db::generation::find(&mut *tx, id).await?.ok_or(AppError::NotFound("proposition"))?;
    db::generation::update(&mut *tx, id, &row.proposal, "DISCARDED").await?;
    tx.commit().await?;
    Ok(())
}

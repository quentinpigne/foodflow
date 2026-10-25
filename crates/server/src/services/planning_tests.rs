//! Tests d'intégration de la planification (spec 05) et du bilan (spec 04), sur le jeu de démonstration.

use foodflow_domain::nutrition::assessment::Status;
use foodflow_domain::planning::signals::Signal;
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::shared::ids::MealId;
use foodflow_domain::{Amount, Date, Ratio};

use super::assessment::{self, AssessmentQuery};
use super::household::{self, MemberInput};
use super::meal_validation::{self, LeftoverInput, LineTarget, ValidationInput};
use super::meals::{self, CopyInput, Diner, Meal, MealInput, MealItem, NewMeal, PlanQuery};
use super::recipes::{self, RecipeQuery};
use super::week::{self, HabitualItem, HabitualMealInput, SlotTemplateInput};
use super::{demo, foods, stock};
use crate::db::{self, Db};
use crate::error::AppError;

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const TODAY: &str = "2026-10-25";

/// La démonstration, sans ses repas : chaque test planifie les siens. Son foyer : Alex et Sam (1,0), Lou (0,6),
/// absent le midi en semaine.
async fn setup() -> (Db, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    demo::load(&db, d(TODAY)).await.unwrap();
    for meal in db::planning::meals(&db.read, d("2026-01-01"), d("2026-12-31")).await.unwrap() {
        db::planning::delete_meal(&db.write, meal.id).await.unwrap();
    }
    (db, dir)
}

async fn slot(db: &Db, name: &str) -> foodflow_domain::shared::ids::MealSlotId {
    week::template(db).await.unwrap().into_iter().find(|s| s.name == name).unwrap().slot_id
}

async fn recipe(db: &Db, name: &str) -> foodflow_domain::shared::ids::RecipeId {
    recipes::search(db, &RecipeQuery::default()).await.unwrap().into_iter().find(|r| r.name == name).unwrap().id
}

/// Un repas tel qu'on le renvoie à l'enregistrement, éléments remplacés.
fn input(meal: &Meal, items: Vec<MealItem>) -> MealInput {
    MealInput {
        status: meal.status,
        available_active_minutes: meal.available_active_minutes,
        earliest_start: meal.earliest_start,
        ready_by: meal.ready_by,
        locked: meal.locked,
        note: meal.note.clone(),
        diners: meal.diners.clone(),
        items,
    }
}

fn cook(recipe: foodflow_domain::shared::ids::RecipeId, portions: i64) -> MealItem {
    MealItem {
        id: None,
        role_id: None,
        source: ItemSource::Cook,
        recipe_id: Some(recipe),
        cooked_portions: Some(Amount::from_raw(portions)),
        lot_id: None,
        leftover_of_item_id: None,
        food_id: None,
        product_id: None,
        amount: None,
        per_adult_equivalent: false,
        name: String::new(),
        measure: None,
        planned_leftover: None,
        leftover_deadline: None,
        eaten_ratio: None,
    }
}

async fn create(db: &Db, date: &str, slot_name: &str) -> Meal {
    let slot_id = slot(db, slot_name).await;
    let id = meals::create(db, &NewMeal { date: d(date), slot_id }).await.unwrap();
    meals::get(db, id, d(TODAY)).await.unwrap()
}

/// La semaine type par défaut, la présence par défaut et le repas habituel (R-PLA-2, R-PLA-5, R-PLA-20,
/// R-PLA-21) ; critère d'acceptation : « Tartines », mises à l'échelle des convives.
#[tokio::test]
async fn r_pla_5_and_20_meal_from_template() {
    let (db, _dir) = setup().await;
    let template = week::template(&db).await.unwrap();
    let dinner = template.iter().find(|s| s.name == "Dîner").unwrap();
    assert_eq!(dinner.days.len(), 7);
    assert_eq!(dinner.days.iter().find(|d| d.weekday == 1).unwrap().available_active_minutes, 30);
    assert_eq!(dinner.days.iter().find(|d| d.weekday == 6).unwrap().available_active_minutes, 45);
    assert_eq!(
        dinner.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["Plat", "Accompagnement", "Laitage ou fruit"]
    );

    // Lundi midi : l'enfant est absent par défaut.
    let lunch = create(&db, "2026-10-26", "Déjeuner").await;
    assert_eq!(lunch.adult_equivalents, Ratio::from_thousandths(2_000));
    assert_eq!(
        meals::create(&db, &NewMeal { date: d("2026-10-26"), slot_id: lunch.slot_id })
            .await
            .err()
            .map(|e| e.to_string()),
        Some("il y a déjà un repas sur ce créneau ce jour-là".into())
    );

    // Repas habituel du petit-déjeuner : 250 mL de lait par équivalent adulte.
    let milk = foods::search(&db, &foods::FoodQuery { q: Some("lait".into()), ..Default::default() })
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.name == "Lait demi-écrémé")
        .unwrap();
    let habitual = week::save_habitual(
        &db,
        None,
        &HabitualMealInput {
            name: "Tartines".into(),
            items: vec![HabitualItem {
                food_id: Some(milk.id),
                product_id: None,
                recipe_id: None,
                amount: Amount::from_raw(250_000),
                name: String::new(),
                measure: None,
            }],
        },
    )
    .await
    .unwrap();
    let breakfast = template.iter().find(|s| s.name == "Petit-déjeuner").unwrap();
    let mut days = breakfast.days.clone();
    for day in &mut days {
        day.habitual_meal_id = Some(habitual);
    }
    week::save_template(&db, breakfast.slot_id, &SlotTemplateInput { days, roles: breakfast.roles.clone() })
        .await
        .unwrap();
    let morning = create(&db, "2026-10-24", "Petit-déjeuner").await;
    assert_eq!(morning.adult_equivalents, Ratio::from_thousandths(2_600));
    assert_eq!(morning.items.len(), 1);
    assert!(morning.items[0].per_adult_equivalent);
    let proposal = meal_validation::proposal(&db, morning.id, d(TODAY)).await.unwrap();
    assert_eq!(proposal.items[0].lines[0].needed, Amount::from_raw(650_000), "250 mL × 2,6");
}

/// Des crêpes (20 min actives, 80 min au total) sur un dîner de semaine (30 min, de 18 h 30 à 19 h 30) :
/// acceptées, mais signalées (R-PLA-50).
#[tokio::test]
async fn r_pla_50_signals_do_not_block() {
    let (db, _dir) = setup().await;
    let meal = create(&db, "2026-10-27", "Dîner").await;
    assert!(meal.signals.contains(&Signal::RequiredRoleEmpty { role: "Plat".into() }));
    let crepes = recipe(&db, "Crêpes").await;
    meals::update(&db, meal.id, &input(&meal, vec![cook(crepes, 2_600)])).await.unwrap();
    let meal = meals::get(&db, meal.id, d(TODAY)).await.unwrap();
    assert!(meal.signals.contains(&Signal::ReadyByExceeded { needed: 80, available: 60 }), "{:?}", meal.signals);
    assert!(!meal.signals.iter().any(|s| matches!(s, Signal::TimeExceeded { .. })));
}

/// Le gratin de samedi, cuisiné pour 6 à 2,6 équivalents adulte, laisse 3,4 portions ; dimanche midi en
/// mange 2,6 (R-PLA-13). Valider samedi sort les ingrédients, crée le lot des restes, et le repas de
/// dimanche le consomme ; les annulations se font dans l'ordre inverse (R-PLA-30 à R-PLA-34).
#[tokio::test]
async fn r_pla_13_30_34_cook_for_more_validate_and_undo() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let saturday = create(&db, "2026-10-24", "Dîner").await;
    meals::update(&db, saturday.id, &input(&saturday, vec![cook(gratin, 6_000)])).await.unwrap();
    let saturday = meals::get(&db, saturday.id, d(TODAY)).await.unwrap();
    assert_eq!(saturday.items[0].planned_leftover, Some(Amount::from_raw(3_400)));
    assert!(saturday.signals.contains(&Signal::ToValidate));
    assert!(saturday.signals.contains(&Signal::LeftoverUnassigned { portions: Amount::from_raw(3_400) }));

    let sunday = create(&db, "2026-10-25", "Déjeuner").await;
    let leftover = MealItem {
        source: ItemSource::Lot,
        recipe_id: None,
        cooked_portions: None,
        leftover_of_item_id: saturday.items[0].id,
        amount: Some(Amount::from_raw(2_600)),
        ..cook(gratin, 1)
    };
    meals::update(&db, sunday.id, &input(&sunday, vec![leftover])).await.unwrap();
    let saturday = meals::get(&db, saturday.id, d(TODAY)).await.unwrap();
    assert!(saturday.signals.contains(&Signal::LeftoverUnassigned { portions: Amount::from_raw(800) }));

    // Proposition : 1,5 kg de courgettes demandés, plus qu'il n'y en a en stock.
    let proposal = meal_validation::proposal(&db, saturday.id, d(TODAY)).await.unwrap();
    let item = &proposal.items[0];
    assert_eq!(item.planned_leftover, Some(Amount::from_raw(3_400)));
    let zucchini = item.lines.iter().find(|l| l.name == "Courgette").unwrap();
    let zucchini_lot = zucchini.takes[0].lot_id;
    let in_stock = stock::get(&db, zucchini_lot, d(TODAY)).await.unwrap().lot.quantity.unwrap();
    assert_eq!(zucchini.needed, Amount::from_raw(1_500_000));
    assert_eq!(zucchini.takes[0].amount, in_stock);
    assert_eq!(zucchini.shortfall, Amount::from_raw(1_500_000) - in_stock);
    // Le poivre, à volonté, ne sort pas du stock (R-REC-22).
    assert!(item.unquantified.is_empty() && !item.lines.iter().any(|l| l.name == "Poivre noir"));

    // Validation : 2 portions de reste au frigo.
    let fridge = proposal.leftover_location_id.unwrap();
    let declared =
        LeftoverInput { item_id: item.item_id, amount: Amount::from_raw(2_000), location_id: fridge, as_food: false };
    meal_validation::validate(&db, saturday.id, &ValidationInput { lines: None, leftovers: vec![declared] }, d(TODAY))
        .await
        .unwrap();
    let done = meals::get(&db, saturday.id, d(TODAY)).await.unwrap();
    assert_eq!(done.status, MealStatus::Done);
    assert_eq!(done.items[0].eaten_ratio, Some(Ratio::from_thousandths(667)), "4 portions mangées sur 6");
    assert!(stock::get(&db, zucchini_lot, d(TODAY)).await.unwrap().lot.depleted);
    let sunday_meal = meals::get(&db, sunday.id, d(TODAY)).await.unwrap();
    let leftover_lot = sunday_meal.items[0].lot_id.expect("le reste prévu est devenu un lot");
    let lot = stock::get(&db, leftover_lot, d(TODAY)).await.unwrap().lot;
    assert_eq!((lot.name.as_str(), lot.quantity), ("Restes : Gratin de courgettes", Some(Amount::from_raw(2_000))));
    assert_eq!(lot.expiry_date, Some(d("2026-10-26")), "conservation de 2 jours (R-STK-34)");

    // Dimanche midi mange les restes ; samedi ne s'annule plus avant dimanche.
    meal_validation::validate(&db, sunday.id, &ValidationInput::default(), d(TODAY)).await.unwrap();
    let blocked = meal_validation::unvalidate(&db, saturday.id, d(TODAY)).await;
    assert!(matches!(blocked, Err(AppError::Invalid { .. })), "{blocked:?}");
    meal_validation::unvalidate(&db, sunday.id, d(TODAY)).await.unwrap();
    meal_validation::unvalidate(&db, saturday.id, d(TODAY)).await.unwrap();
    let back = stock::get(&db, zucchini_lot, d(TODAY)).await.unwrap().lot;
    assert_eq!(back.quantity, Some(in_stock), "sorties annulées (R-PLA-34)");
    assert_eq!(meals::get(&db, saturday.id, d(TODAY)).await.unwrap().status, MealStatus::Planned);
    assert!(stock::get(&db, leftover_lot, d(TODAY)).await.unwrap().lot.depleted, "restes annulés");
}

/// Ajuster la validation : utilisé sans sortie de stock (R-PLA-32).
#[tokio::test]
async fn r_pla_32_used_without_stock() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let meal = create(&db, "2026-10-24", "Dîner").await;
    meals::update(&db, meal.id, &input(&meal, vec![cook(gratin, 2_600)])).await.unwrap();
    let proposal = meal_validation::proposal(&db, meal.id, d(TODAY)).await.unwrap();
    let item = &proposal.items[0];
    let zucchini =
        item.lines.iter().find(|l| matches!(l.target, LineTarget::Food(_)) && l.name == "Courgette").unwrap();
    let in_stock = stock::get(&db, zucchini.takes[0].lot_id, d(TODAY)).await.unwrap().lot.quantity;
    // Rien n'est pris dans les lots : tout est enregistré comme utilisé, sans sortie.
    let lines = item
        .lines
        .iter()
        .map(|l| meal_validation::LineInput {
            item_id: item.item_id,
            target: l.target,
            measure: l.measure,
            used: l.needed,
            takes: vec![],
        })
        .collect();
    meal_validation::validate(&db, meal.id, &ValidationInput { lines: Some(lines), leftovers: vec![] }, d(TODAY))
        .await
        .unwrap();
    let lot = stock::get(&db, zucchini.takes[0].lot_id, d(TODAY)).await.unwrap().lot;
    assert_eq!(lot.quantity, in_stock, "aucune sortie");
    // Le bilan réel compte quand même ce repas.
    let real =
        assessment::assess(&db, &AssessmentQuery { start: d("2026-10-24"), days: Some(1), actual: true }, d(TODAY))
            .await
            .unwrap();
    assert_eq!(real.days[0].meals, 1);
}

/// Bilan prévu (NUT-04) : l'objectif PNNS de la démonstration, rapporté à l'équivalent adulte ; un créneau
/// vide rend la journée incomplète (R-NUT-32).
#[tokio::test]
async fn nut_04_planned_assessment() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let meal = create(&db, "2026-10-27", "Dîner").await;
    meals::update(&db, meal.id, &input(&meal, vec![cook(gratin, 2_600)])).await.unwrap();
    let report =
        assessment::assess(&db, &AssessmentQuery { start: d("2026-10-27"), days: Some(1), actual: false }, d(TODAY))
            .await
            .unwrap();
    assert!(report.goal_id.is_some());
    let day = &report.days[0];
    assert_eq!(day.meals, 1);
    // Gratin pour 4 : 1 000 g de courgettes (160 kcal), 250 mL de lait (× 1,03 g/mL = 257,5 g, 118,45 kcal),
    // 100 g d'emmental (377 kcal), soit 655,45 kcal ; une portion adulte : 163,8625 kcal, arrondie au cal.
    assert_eq!(day.energy, Some(163_863));
    assert!(day.missing.iter().any(|m| m.starts_with("Petit-déjeuner du 27/10")), "{:?}", day.missing);
    let energy = report.criteria.iter().find(|c| c.criterion.nutrient_code.as_deref() == Some("ENERGY")).unwrap();
    assert_eq!(energy.status, Status::Incomplete);
    let real =
        assessment::assess(&db, &AssessmentQuery { start: d("2026-10-27"), days: Some(1), actual: true }, d(TODAY))
            .await
            .unwrap();
    assert_eq!(real.days[0].meals, 0, "rien de validé");
}

/// Copier une semaine (PLA-13) : les repas cuisinés reviennent, les créneaux déjà remplis sont laissés.
#[tokio::test]
async fn pla_13_copy_week() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let meal = create(&db, "2026-10-20", "Dîner").await;
    meals::update(&db, meal.id, &input(&meal, vec![cook(gratin, 2_600)])).await.unwrap();
    create(&db, "2026-10-21", "Dîner").await;
    create(&db, "2026-10-28", "Dîner").await;
    let result = meals::copy(&db, &CopyInput { from: d("2026-10-19"), to: d("2026-10-26"), days: None }).await.unwrap();
    assert_eq!((result.created, result.skipped), (1, 1));
    let plan = meals::plan(&db, &PlanQuery { start: d("2026-10-26"), days: Some(7) }, d(TODAY)).await.unwrap();
    let tuesday: Vec<&Meal> = plan.days[1].meals.iter().collect();
    assert_eq!(tuesday.len(), 1);
    assert_eq!(tuesday[0].items[0].recipe_id, Some(gratin));
    assert_eq!(tuesday[0].status, MealStatus::Planned);
}

/// Un repas validé ne se modifie pas, et un convive qui refuse un élément est signalé (R-PLA-31, R-PLA-50).
#[tokio::test]
async fn r_pla_31_validated_meal_is_locked_and_refusals_are_signalled() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let zucchini =
        foods::search(&db, &foods::FoodQuery { q: Some("courgette".into()), ..Default::default() }).await.unwrap()[0]
            .id;
    let input_member = MemberInput {
        name: "Camille".into(),
        adult_equivalent: Ratio::from_thousandths(600),
        active: true,
        absences: vec![],
        preferences: vec![household::PreferenceView {
            target: foodflow_domain::recipes::Target::Food(zucchini),
            target_name: String::new(),
            level: foodflow_domain::planning::household::PreferenceLevel::Refuses,
        }],
    };
    let camille = household::save_member(&db, None, &input_member).await.unwrap();
    let meal = create(&db, "2026-10-24", "Dîner").await;
    meals::update(&db, meal.id, &input(&meal, vec![cook(gratin, 1_000)])).await.unwrap();
    let meal = meals::get(&db, meal.id, d(TODAY)).await.unwrap();
    assert!(meal.signals.contains(&Signal::Refused { member: camille, item: "Gratin de courgettes".into() }));
    meal_validation::validate(&db, meal.id, &ValidationInput::default(), d(TODAY)).await.unwrap();
    let again = meals::update(&db, meal.id, &input(&meal, vec![])).await;
    assert!(matches!(again, Err(AppError::Domain(foodflow_domain::DomainError::MealValidated))));
    // Un membre déjà convive se désactive au lieu d'être supprimé.
    household::delete_member(&db, camille).await.unwrap();
    let members = household::members(&db).await.unwrap();
    assert!(members.iter().any(|m| m.id == camille && !m.active));
    let _: Vec<Diner> = meal.diners;
    let _: MealId = meal.id;
}

/// Supprimer le repas d'origine supprime le reste prévu qu'un autre repas devait manger (R-PLA-13).
#[tokio::test]
async fn r_pla_13_deleting_the_cooked_meal_drops_its_planned_leftover() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let saturday = create(&db, "2026-10-24", "Dîner").await;
    meals::update(&db, saturday.id, &input(&saturday, vec![cook(gratin, 6_000)])).await.unwrap();
    let saturday = meals::get(&db, saturday.id, d(TODAY)).await.unwrap();
    let sunday = create(&db, "2026-10-25", "Déjeuner").await;
    let leftover = MealItem {
        source: ItemSource::Lot,
        recipe_id: None,
        cooked_portions: None,
        leftover_of_item_id: saturday.items[0].id,
        amount: Some(Amount::from_raw(2_600)),
        ..cook(gratin, 1)
    };
    meals::update(&db, sunday.id, &input(&sunday, vec![leftover])).await.unwrap();
    // Un reste ne se mange pas avant d'avoir été cuisiné.
    let friday = create(&db, "2026-10-23", "Dîner").await;
    let early = MealItem {
        source: ItemSource::Lot,
        recipe_id: None,
        cooked_portions: None,
        leftover_of_item_id: saturday.items[0].id,
        amount: Some(Amount::from_raw(1_000)),
        ..cook(gratin, 1)
    };
    assert!(meals::update(&db, friday.id, &input(&friday, vec![early])).await.is_err());
    meals::delete(&db, saturday.id).await.unwrap();
    assert!(meals::get(&db, sunday.id, d(TODAY)).await.unwrap().items.is_empty());
}

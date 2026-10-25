//! Tests d'intégration des services, sur une vraie base SQLite (architecture.md § Stratégie de test).

use foodflow_domain::foods::TrackingMode;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, LocationId, LotId};
use foodflow_domain::stock::{ExpiryKind, ExpiryStatus, LocationKind, MovementKind, PresenceState, WasteReason};
use foodflow_domain::{Amount, Date, DomainError, Measure};

use super::foods::{self, FoodInput, FoodQuery};
use super::groups::{self, FoodGroupInput};
use super::stock::{
    self, AdjustInput, CheckInput, ConsumeInput, DateInput, InventoryEntry, InventoryInput, LotInput, LotOrigin,
    MoveInput, TrackingModeInput, WasteInput,
};
use super::{locations, seed};
use crate::db::groups::Settings;
use crate::db::{self, Db};
use crate::error::AppError;

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const TODAY: &str = "2026-10-25";

async fn setup() -> (Db, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    seed::run(&db).await.unwrap();
    (db, dir)
}

async fn group(db: &Db, name: &str, settings: Settings) -> FoodGroupId {
    groups::create(db, &FoodGroupInput { parent_id: None, name: name.into(), settings, reference_food_id: None })
        .await
        .unwrap()
}

async fn food(db: &Db, group: FoodGroupId, name: &str, settings: Settings) -> FoodId {
    let input = FoodInput {
        group_id: group,
        name: name.into(),
        aliases: vec![],
        nutrient_basis: None,
        density_mg_per_ml: None,
        piece_weight_mg: None,
        settings,
        minimum_stock: None,
        preferred_product_id: None,
        hidden: false,
    };
    foods::create(db, &input).await.unwrap()
}

async fn location(db: &Db, kind: LocationKind) -> LocationId {
    locations::list(db).await.unwrap().into_iter().find(|l| l.kind == kind).unwrap().id
}

fn lot_input(food: FoodId, location: LocationId, date: &str, amount: Option<i64>, expiry: Option<&str>) -> LotInput {
    LotInput {
        product_id: None,
        food_id: Some(food),
        location_id: location,
        date: Some(d(date)),
        amount: amount.map(Amount::from_raw),
        presence: None,
        expiry_kind: if expiry.is_some() { ExpiryKind::UseBy } else { ExpiryKind::None },
        expiry_date: expiry.map(d),
        expiry_missing: false,
        unit_price: None,
        count: None,
        origin: LotOrigin::Purchase,
        note: None,
    }
}

async fn lot(db: &Db, input: &LotInput) -> LotId {
    stock::create(db, input, d(TODAY)).await.unwrap()[0]
}

async fn quantity(db: &Db, lot: LotId) -> i64 {
    stock::get(db, lot, d(TODAY)).await.unwrap().lot.quantity.unwrap().raw()
}

fn consume(date: &str, amount: i64) -> ConsumeInput {
    ConsumeInput { date: Some(d(date)), amount: Some(Amount::from_raw(amount)), presence: None, note: None }
}

/// Critère d'acceptation de la spec 02 : 1 L − 250 mL − 200 mL = 550 mL ; inventaire à 500 mL, puis
/// −100 mL = 400 mL, écart de −50 mL conservé.
#[tokio::test]
async fn r_stk_2_and_r_stk_40_quantity_and_inventory() {
    let (db, _dir) = setup().await;
    let milks = group(&db, "Laits", Settings { stock_measure: Some(Measure::Volume), ..Settings::default() }).await;
    let milk = food(&db, milks, "Lait", Settings::default()).await;
    let fridge = location(&db, LocationKind::Fridge).await;
    let id = lot(&db, &lot_input(milk, fridge, "2026-10-20", Some(1_000_000), Some("2026-11-02"))).await;
    stock::consume(&db, id, &consume("2026-10-22", 250_000), d(TODAY)).await.unwrap();
    stock::consume(&db, id, &consume("2026-10-24", 200_000), d(TODAY)).await.unwrap();
    assert_eq!(quantity(&db, id).await, 550_000);
    let detail = stock::get(&db, id, d(TODAY)).await.unwrap();
    assert_eq!(detail.lot.measure, Measure::Volume, "mesure héritée de la famille (R-ALI-2)");

    let entry = InventoryEntry { lot_id: id, amount: Some(Amount::from_raw(500_000)), presence: None };
    stock::inventory(&db, &InventoryInput { date: Some(d(TODAY)), entries: vec![entry] }, d(TODAY)).await.unwrap();
    stock::consume(&db, id, &consume(TODAY, 100_000), d(TODAY)).await.unwrap();
    assert_eq!(quantity(&db, id).await, 400_000);
    let detail = stock::get(&db, id, d(TODAY)).await.unwrap();
    let inventory = detail.movements.iter().find(|m| m.kind == MovementKind::Inventory).unwrap();
    assert_eq!(inventory.shortfall, Some(Amount::from_raw(-50_000)));

    // Plus aucun mouvement avant l'inventaire (R-STK-14).
    let late = stock::consume(&db, id, &consume("2026-10-23", 10_000), d(TODAY)).await;
    assert!(matches!(late, Err(AppError::Domain(DomainError::MovementBeforeInventory(_)))));
}

/// Critère d'acceptation : sortir 300 g d'un lot de 200 g sort 200 g, épuise le lot et le signale.
#[tokio::test]
async fn r_stk_3_withdrawal_is_bounded_and_flagged() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Légumes", Settings::default()).await;
    let carrot = food(&db, g, "Carotte", Settings::default()).await;
    let pantry = location(&db, LocationKind::Pantry).await;
    let id = lot(&db, &lot_input(carrot, pantry, TODAY, Some(200_000), None)).await;
    stock::consume(&db, id, &consume(TODAY, 300_000), d(TODAY)).await.unwrap();
    let detail = stock::get(&db, id, d(TODAY)).await.unwrap();
    assert_eq!(detail.lot.quantity, Some(Amount::ZERO));
    assert!(detail.lot.depleted, "R-STK-4");
    assert!(detail.lot.needs_review);
    assert_eq!(detail.movements[0].shortfall, Some(Amount::from_raw(100_000)));
    // Un lot épuisé disparaît de la vue du stock.
    let query = stock::LotQuery { location_id: None, food_id: None, include_depleted: false };
    assert!(stock::list(&db, &query, d(TODAY)).await.unwrap().is_empty());
}

/// Critère d'acceptation : yaourt de DLC le 30/10 ouvert le 24/10, 2 jours après ouverture → 26/10.
#[tokio::test]
async fn r_stk_31_opening_shortens_expiry() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Yaourts", Settings { opened_shelf_life_days: Some(2), ..Settings::default() }).await;
    let yogurt = food(&db, g, "Yaourt", Settings::default()).await;
    let fridge = location(&db, LocationKind::Fridge).await;
    let id = lot(&db, &lot_input(yogurt, fridge, "2026-10-20", Some(500_000), Some("2026-10-30"))).await;
    stock::open(&db, id, &DateInput { date: Some(d("2026-10-24")) }, d(TODAY)).await.unwrap();
    let detail = stock::get(&db, id, d(TODAY)).await.unwrap();
    assert_eq!(detail.lot.effective_expiry.unwrap().date, d("2026-10-26"));
    assert_eq!(detail.lot.status, ExpiryStatus::Soon);
}

/// Congélation (R-STK-32), décongélation (R-STK-33), et annulation d'un déplacement partiel (R-STK-12, R-STK-15).
#[tokio::test]
async fn r_stk_32_33_and_15_freezing_and_partial_moves() {
    let (db, _dir) = setup().await;
    let g = group(
        &db,
        "Viandes",
        Settings { frozen_shelf_life_days: Some(90), thawed_shelf_life_days: Some(1), ..Settings::default() },
    )
    .await;
    let beef = food(&db, g, "Bœuf haché", Settings::default()).await;
    let fridge = location(&db, LocationKind::Fridge).await;
    let freezer = location(&db, LocationKind::Freezer).await;
    let id = lot(&db, &lot_input(beef, fridge, "2026-10-20", Some(600_000), Some("2026-10-23"))).await;

    // Congeler après la date limite est refusé.
    let late = MoveInput { date: Some(d(TODAY)), to_location_id: freezer, amount: Some(Amount::from_raw(300_000)) };
    assert!(matches!(
        stock::move_lot(&db, id, &late, d(TODAY)).await,
        Err(AppError::Domain(DomainError::FreezeAfterExpiry))
    ));

    // La moitié part au congélateur le 22 : nouveau lot, congelé, 90 jours.
    let half =
        MoveInput { date: Some(d("2026-10-22")), to_location_id: freezer, amount: Some(Amount::from_raw(300_000)) };
    let frozen = stock::move_lot(&db, id, &half, d(TODAY)).await.unwrap().unwrap();
    assert_eq!(quantity(&db, id).await, 300_000);
    let part = stock::get(&db, frozen, d(TODAY)).await.unwrap();
    assert_eq!(part.lot.quantity, Some(Amount::from_raw(300_000)));
    assert_eq!(part.lot.effective_expiry.unwrap().date, d("2027-01-20"));

    // Décongelé aujourd'hui : un jour ; il ne se recongèle pas.
    let out = MoveInput { date: Some(d(TODAY)), to_location_id: fridge, amount: None };
    stock::move_lot(&db, frozen, &out, d(TODAY)).await.unwrap();
    let thawed = stock::get(&db, frozen, d(TODAY)).await.unwrap();
    assert_eq!(thawed.lot.effective_expiry.unwrap().date, d("2026-10-26"));
    let again = MoveInput { date: Some(d(TODAY)), to_location_id: freezer, amount: None };
    assert!(matches!(
        stock::move_lot(&db, frozen, &again, d(TODAY)).await,
        Err(AppError::Domain(DomainError::RefreezeThawed))
    ));

    // Annuler la décongélation remet le lot au congélateur, sans date de décongélation.
    let thaw_move = thawed.movements.iter().find(|m| m.kind == MovementKind::Move && m.amount.is_none()).unwrap();
    stock::reverse(&db, thaw_move.id, d(TODAY)).await.unwrap();
    let back = stock::get(&db, frozen, d(TODAY)).await.unwrap();
    assert_eq!(back.lot.location_id, freezer);
    assert_eq!(back.lot.thawed_on, None);

    // Annuler le déplacement partiel annule ses deux mouvements ensemble (R-STK-12).
    let split = back.movements.iter().find(|m| m.kind == MovementKind::Move && m.amount.is_some()).unwrap();
    stock::reverse(&db, split.id, d(TODAY)).await.unwrap();
    assert_eq!(quantity(&db, id).await, 600_000);
    assert!(stock::get(&db, frozen, d(TODAY)).await.unwrap().lot.depleted);
    // Une deuxième annulation est refusée (R-STK-11).
    assert!(matches!(
        stock::reverse(&db, split.id, d(TODAY)).await,
        Err(AppError::Domain(DomainError::AlreadyReversed))
    ));
}

/// Date dépassée : jeter ou garder avec une nouvelle date limite (R-STK-37) ; produit jeté (R-STK-13).
#[tokio::test]
async fn r_stk_37_and_r_stk_13_expired_lots() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Yaourts", Settings::default()).await;
    let yogurt = food(&db, g, "Yaourt", Settings::default()).await;
    let fridge = location(&db, LocationKind::Fridge).await;
    let kept = lot(&db, &lot_input(yogurt, fridge, "2026-10-10", Some(125_000), Some("2026-10-24"))).await;
    let thrown = lot(&db, &lot_input(yogurt, fridge, "2026-10-10", Some(125_000), Some("2026-10-24"))).await;
    assert_eq!(stock::get(&db, kept, d(TODAY)).await.unwrap().lot.status, ExpiryStatus::Expired);

    stock::check(&db, kept, &CheckInput { date: None, until: None }, d(TODAY)).await.unwrap();
    let detail = stock::get(&db, kept, d(TODAY)).await.unwrap();
    assert_eq!(detail.lot.checked_until, Some(d("2026-10-26")));
    assert_eq!(detail.lot.status, ExpiryStatus::Soon);

    let waste = WasteInput { date: None, amount: None, reason: WasteReason::Expired, note: None };
    stock::waste(&db, thrown, &waste, d(TODAY)).await.unwrap();
    let detail = stock::get(&db, thrown, d(TODAY)).await.unwrap();
    assert!(detail.lot.depleted);
    assert_eq!(detail.movements[0].waste_reason, Some(WasteReason::Expired));
}

/// Changer le mode de suivi convertit les lots (R-STK-21).
#[tokio::test]
async fn r_stk_21_tracking_mode_change() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Condiments", Settings::default()).await;
    let mustard = food(&db, g, "Moutarde", Settings::default()).await;
    let fridge = location(&db, LocationKind::Fridge).await;
    let id = lot(&db, &lot_input(mustard, fridge, TODAY, Some(200_000), None)).await;

    // Le mode ne change pas par la fiche de l'aliment tant qu'il a des lots.
    let detail = foods::get(&db, mustard).await.unwrap();
    let mut input = FoodInput {
        group_id: g,
        name: detail.summary.name.clone(),
        aliases: vec![],
        nutrient_basis: None,
        density_mg_per_ml: None,
        piece_weight_mg: None,
        settings: Settings { tracking_mode: Some(TrackingMode::Presence), ..Settings::default() },
        minimum_stock: None,
        preferred_product_id: None,
        hidden: false,
    };
    assert!(foods::update(&db, mustard, &input).await.is_err());

    let to_presence = TrackingModeInput { mode: TrackingMode::Presence, inventory: vec![], date: None };
    stock::change_tracking_mode(&db, mustard, &to_presence, d(TODAY)).await.unwrap();
    let lot = stock::get(&db, id, d(TODAY)).await.unwrap().lot;
    assert_eq!((lot.tracking_mode, lot.presence), (TrackingMode::Presence, Some(PresenceState::Available)));
    // Cuisiner ne change pas l'état (R-STK-22) ; l'utilisateur le passe à « bientôt fini ».
    let cook = ConsumeInput { date: None, amount: None, presence: None, note: None };
    stock::consume(&db, id, &cook, d(TODAY)).await.unwrap();
    let low = AdjustInput { date: None, amount: None, presence: Some(PresenceState::Low), note: None };
    stock::adjust(&db, id, &low, d(TODAY)).await.unwrap();
    assert_eq!(stock::get(&db, id, d(TODAY)).await.unwrap().lot.presence, Some(PresenceState::Low));

    // Revenir en quantité exige la quantité de chaque lot.
    let without = TrackingModeInput { mode: TrackingMode::Quantity, inventory: vec![], date: None };
    assert!(stock::change_tracking_mode(&db, mustard, &without, d(TODAY)).await.is_err());
    let entry = InventoryEntry { lot_id: id, amount: Some(Amount::from_raw(80_000)), presence: None };
    let with = TrackingModeInput { mode: TrackingMode::Quantity, inventory: vec![entry], date: None };
    stock::change_tracking_mode(&db, mustard, &with, d(TODAY)).await.unwrap();
    assert_eq!(quantity(&db, id).await, 80_000);
    input.settings.tracking_mode = Some(TrackingMode::Quantity);
    foods::update(&db, mustard, &input).await.unwrap();
}

/// Recherche sans accents ni casse, par alias (R-ALI-10, ALI-06) ; noms uniques.
#[tokio::test]
async fn r_ali_10_search_and_unique_names() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Fromages", Settings::default()).await;
    let mut input = FoodInput {
        group_id: g,
        name: "Emmental râpé".into(),
        aliases: vec!["Gruyère râpé".into()],
        nutrient_basis: None,
        density_mg_per_ml: None,
        piece_weight_mg: None,
        settings: Settings::default(),
        minimum_stock: None,
        preferred_product_id: None,
        hidden: false,
    };
    foods::create(&db, &input).await.unwrap();
    let found = foods::search(&db, &FoodQuery { q: Some("gruyere".into()), group_id: None, include_hidden: false })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    input.name = "EMMENTAL RAPE".into();
    input.aliases = vec![];
    assert!(matches!(foods::create(&db, &input).await, Err(AppError::Domain(DomainError::FoodNameTaken))));
    input.name = "Comté".into();
    input.aliases = vec!["gruyère RÂPÉ".into()];
    assert!(matches!(foods::create(&db, &input).await, Err(AppError::Domain(DomainError::AliasTaken))));
}

/// Une famille ne peut pas devenir sa propre descendante (R-ALI-1) ; une famille utilisée ne se supprime pas
/// mais se fusionne (R-ALI-3).
#[tokio::test]
async fn r_ali_1_and_r_ali_3_groups() {
    let (db, _dir) = setup().await;
    let dairy = group(&db, "Produits laitiers", Settings::default()).await;
    let cheese = groups::create(
        &db,
        &FoodGroupInput {
            parent_id: Some(dairy),
            name: "Fromages".into(),
            settings: Settings::default(),
            reference_food_id: None,
        },
    )
    .await
    .unwrap();
    let cycle = FoodGroupInput {
        parent_id: Some(cheese),
        name: "Produits laitiers".into(),
        settings: Settings::default(),
        reference_food_id: None,
    };
    assert!(matches!(groups::update(&db, dairy, &cycle).await, Err(AppError::Domain(DomainError::FoodGroupCycle))));
    food(&db, cheese, "Comté", Settings::default()).await;
    assert!(matches!(groups::delete(&db, cheese).await, Err(AppError::Domain(DomainError::FoodGroupInUse))));
    let other = group(&db, "Crèmerie", Settings::default()).await;
    groups::merge(&db, cheese, other).await.unwrap();
    let list = groups::list(&db).await.unwrap();
    assert!(!list.iter().any(|g| g.id == cheese));
    assert_eq!(list.iter().find(|g| g.id == other).unwrap().food_count, 1);
}

//! Tests d'intégration des services, sur une vraie base SQLite (architecture.md § Stratégie de test).

use foodflow_domain::foods::TrackingMode;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, LocationId, LotId, ProductId};
use foodflow_domain::stock::{ExpiryKind, ExpiryStatus, LocationKind, MovementKind, PresenceState, WasteReason};
use foodflow_domain::{Amount, Date, DomainError, Measure};

use super::foods::{self, FoodInput, FoodQuery};
use super::groups::{self, FoodGroupInput};
use super::off::{self, OffSettings, OffStatus};
use super::products::{self, ProductInput};
use super::stock::{
    self, AdjustInput, CheckInput, ConsumeInput, DateInput, InventoryEntry, InventoryInput, LotBatchInput, LotInput,
    LotOrigin, MoveInput, TrackingModeInput, WasteInput,
};
use super::{locations, seed};
use crate::clients::off::StaticFacts;
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
    stock::inventory(&db, &InventoryInput { date: Some(d(TODAY)), entries: vec![entry], found: vec![] }, d(TODAY))
        .await
        .unwrap();
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

// ===================================================================== Scan (STK-12, STK-13)

async fn product(db: &Db, food: FoodId, name: &str, barcode: &str, package: i64) -> ProductId {
    let input = ProductInput {
        food_id: Some(food),
        group_id: None,
        name: name.into(),
        brand: None,
        barcode: Some(barcode.into()),
        stock_measure: None,
        package_amount: Some(Amount::from_raw(package)),
        package_units: None,
        nutrient_basis: None,
        off_payload: None,
    };
    products::create(db, &input).await.unwrap()
}

fn product_lot(product: ProductId, location: LocationId, amount: i64, expiry: Option<&str>) -> LotInput {
    LotInput {
        product_id: Some(product),
        food_id: None,
        expiry_missing: expiry.is_none(),
        expiry_kind: ExpiryKind::UseBy,
        ..lot_input(FoodId::new(), location, TODAY, Some(amount), expiry)
    }
}

/// Lecture d'un code : refusé si la clé est fausse, inconnu, ou connu avec les valeurs proposées pour ses
/// lots (R-STK-50, R-STK-52, R-STK-53).
#[tokio::test]
async fn r_stk_50_52_53_barcode_lookup() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Pâtes", Settings::default()).await;
    let pasta = food(&db, g, "Pâtes", Settings::default()).await;
    let pantry = location(&db, LocationKind::Pantry).await;
    let panzani = product(&db, pasta, "Panzani spaghetti", "3017620422003", 500_000).await;
    let barilla = product(&db, pasta, "Barilla penne", "96385074", 500_000).await;
    let facts = StaticFacts::default();

    let invalid = products::lookup_barcode(&db, &facts, "3017620422004").await;
    assert!(matches!(invalid, Err(AppError::Domain(DomainError::InvalidBarcode))), "R-STK-50");
    let unknown = products::lookup_barcode(&db, &facts, "036000291452").await.unwrap();
    assert!(unknown.product.is_none(), "R-STK-52");

    // Sans lot : aucun emplacement proposé, DLC par défaut.
    let first = products::lookup_barcode(&db, &facts, "3017 6204 2200 3").await.unwrap();
    assert_eq!(first.barcode, "3017620422003");
    assert_eq!(first.product.unwrap().id, panzani);
    assert_eq!(first.location_id, None);
    assert_eq!(first.expiry_kind, Some(ExpiryKind::UseBy));

    // Le dernier lot de l'aliment donne l'emplacement ; celui du produit donne aussi la nature.
    let bb =
        LotInput { expiry_kind: ExpiryKind::BestBefore, ..product_lot(panzani, pantry, 500_000, Some("2027-03-01")) };
    lot(&db, &bb).await;
    let other = products::lookup_barcode(&db, &facts, "96385074").await.unwrap();
    assert_eq!(other.product.unwrap().id, barilla);
    assert_eq!(other.location_id, Some(pantry), "emplacement du dernier lot de l'aliment");
    assert_eq!(other.expiry_kind, Some(ExpiryKind::UseBy));
    let again = products::lookup_barcode(&db, &facts, "3017620422003").await.unwrap();
    assert_eq!(again.location_id, Some(pantry));
    assert_eq!(again.expiry_kind, Some(ExpiryKind::BestBefore), "nature du dernier lot du produit");

    // Un aliment conservé sans date propose `NONE`.
    let fruits = group(&db, "Fruits", Settings { shelf_life_days: Some(7), ..Settings::default() }).await;
    let apple = food(&db, fruits, "Pomme", Settings::default()).await;
    product(&db, apple, "Pommes en filet", "40123455", 1_000_000).await;
    let apples = products::lookup_barcode(&db, &facts, "40123455").await.unwrap();
    assert_eq!(apples.expiry_kind, Some(ExpiryKind::None));
}

/// Critère d'acceptation : Panzani 500 g scanné trois fois → 3 lots de 500 g, retirés par une seule
/// annulation (R-STK-51, R-STK-53, R-STK-54).
#[tokio::test]
async fn r_stk_51_53_54_scan_entry_is_one_batch() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Pâtes", Settings::default()).await;
    let pasta = food(&db, g, "Pâtes", Settings::default()).await;
    let pantry = location(&db, LocationKind::Pantry).await;
    let panzani = product(&db, pasta, "Panzani spaghetti", "3017620422003", 500_000).await;
    let line = LotInput { count: Some(3), ..product_lot(panzani, pantry, 500_000, None) };
    let (batch, ids) = stock::create_batch(&db, &LotBatchInput { lines: vec![line] }, d(TODAY)).await.unwrap();
    assert_eq!(ids.len(), 3);
    for id in &ids {
        let lot = stock::get(&db, *id, d(TODAY)).await.unwrap().lot;
        assert_eq!(lot.quantity, Some(Amount::from_raw(500_000)));
        assert!(lot.expiry_missing, "date à compléter (R-COU-51)");
    }
    let query = stock::LotQuery { location_id: None, food_id: Some(pasta), include_depleted: false };
    assert_eq!(stock::list(&db, &query, d(TODAY)).await.unwrap().len(), 3);

    stock::reverse_whole_batch(&db, batch, d(TODAY)).await.unwrap();
    assert!(stock::list(&db, &query, d(TODAY)).await.unwrap().is_empty(), "R-STK-12");
}

/// Critère d'acceptation : inventaire du placard par scan, deux lots constatés, un lot trouvé, le riz
/// non scanné inchangé, le tout en un lot d'opérations (R-STK-55).
#[tokio::test]
async fn r_stk_55_scan_inventory() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Féculents", Settings::default()).await;
    let pasta = food(&db, g, "Pâtes", Settings::default()).await;
    let rice = food(&db, g, "Riz", Settings::default()).await;
    let pantry = location(&db, LocationKind::Pantry).await;
    let panzani = product(&db, pasta, "Panzani spaghetti", "3017620422003", 500_000).await;
    let soon = lot(&db, &product_lot(panzani, pantry, 500_000, Some("2026-11-10"))).await;
    stock::consume(&db, soon, &consume(TODAY, 200_000), d(TODAY)).await.unwrap();
    let later = lot(&db, &product_lot(panzani, pantry, 500_000, Some("2026-12-20"))).await;
    let rice_lot = lot(&db, &lot_input(rice, pantry, TODAY, Some(1_000_000), None)).await;

    let entry = |lot, amount| InventoryEntry { lot_id: lot, amount: Some(Amount::from_raw(amount)), presence: None };
    let input = InventoryInput {
        date: None,
        entries: vec![entry(soon, 300_000), entry(later, 500_000)],
        found: vec![product_lot(panzani, pantry, 500_000, None)],
    };
    let batch = stock::inventory(&db, &input, d(TODAY)).await.unwrap();

    assert_eq!(quantity(&db, soon).await, 300_000);
    assert_eq!(quantity(&db, later).await, 500_000);
    let query = stock::LotQuery { location_id: Some(pantry), food_id: Some(pasta), include_depleted: false };
    let lots = stock::list(&db, &query, d(TODAY)).await.unwrap();
    assert_eq!(lots.len(), 3);
    let found = lots.iter().find(|l| l.id != soon && l.id != later).unwrap();
    let detail = stock::get(&db, found.id, d(TODAY)).await.unwrap();
    assert_eq!(detail.movements[0].kind, MovementKind::Adjustment, "lot trouvé (R-STK-41)");
    assert_eq!(detail.movements[0].batch_id, Some(batch));
    let rice_detail = stock::get(&db, rice_lot, d(TODAY)).await.unwrap();
    assert_eq!(rice_detail.movements.len(), 1, "lot non scanné inchangé");
}

// ===================================================================== Open Food Facts (ALI-08)

const SPAGHETTI: &str = r#"{"product":{"product_name_fr":"Spaghetti","brands":"Panzani","quantity":"500 g",
    "categories_tags":["en:pastas","en:spaghetti"],"nutriments":{"energy-kcal_100g":359,"proteins_100g":12.5}}}"#;
const PENNE: &str = r#"{"product":{"product_name":"Penne rigate","brands":"Barilla","quantity":"1 kg",
    "categories_tags":["en:pastas","en:spaghetti"]}}"#;
const FUSILLI: &str = r#"{"product":{"product_name":"Fusilli n°34","quantity":"500 g",
    "categories_tags":["en:pastas","en:dry-pastas","en:fusilli"],
    "categories_tags_fr":["pâtes alimentaires","Pâtes sèches","Fusilli"]}}"#;

fn sheets(entries: &[(&str, &str)]) -> StaticFacts {
    StaticFacts {
        sheets: entries.iter().map(|(code, json)| ((*code).to_owned(), (*json).to_owned())).collect(),
        ..StaticFacts::default()
    }
}

/// Code inconnu : brouillon tiré d'Open Food Facts, rattachement proposé d'après le nom ; rien n'est créé
/// avant validation (R-ALI-60, ADR-0005).
#[tokio::test]
async fn r_ali_60_draft_from_open_food_facts() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Pâtes", Settings::default()).await;
    let spaghetti = food(&db, g, "Spaghetti", Settings::default()).await;
    let facts = sheets(&[("3017620422003", SPAGHETTI)]);

    let lookup = products::lookup_barcode(&db, &facts, "3017620422003").await.unwrap();
    assert!(lookup.product.is_none());
    assert_eq!(lookup.off_status, Some(OffStatus::Found));
    let draft = lookup.draft.unwrap();
    assert_eq!(draft.name.as_deref(), Some("Spaghetti"));
    assert_eq!(draft.brand.as_deref(), Some("Panzani"));
    assert_eq!(draft.package_measure, Some(Measure::Mass));
    assert_eq!(draft.package_amount, Some(Amount::from_raw(500_000)));
    assert_eq!(draft.proposed_food_id, Some(spaghetti));
    let query = products::ProductQuery { q: None, barcode: Some("3017620422003".into()) };
    assert!(products::search(&db, &query).await.unwrap().is_empty(), "aucun produit créé");

    // Absent, injoignable, désactivé.
    let absent = products::lookup_barcode(&db, &facts, "96385074").await.unwrap();
    assert_eq!(absent.off_status, Some(OffStatus::NotFound));
    let down = StaticFacts { fail: true, ..StaticFacts::default() };
    let unavailable = products::lookup_barcode(&db, &down, "96385074").await.unwrap();
    assert_eq!(unavailable.off_status, Some(OffStatus::Unavailable));
    off::update_settings(&db, &OffSettings { enabled: false }).await.unwrap();
    let silent = sheets(&[("3017620422003", SPAGHETTI)]);
    let disabled = products::lookup_barcode(&db, &silent, "3017620422003").await.unwrap();
    assert_eq!(disabled.off_status, Some(OffStatus::Disabled));
    assert!(silent.asked.lock().unwrap().is_empty(), "rien n'est envoyé (ADR-0007)");
}

/// Le brouillon validé garde sa fiche : le produit ne déclenche plus d'appel (R-ALI-61) ; sa catégorie
/// est mémorisée et proposée pour le produit suivant de la même catégorie (R-ALI-62).
#[tokio::test]
async fn r_ali_61_62_validated_draft() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Féculents", Settings::default()).await;
    let pasta = food(&db, g, "Pâtes sèches", Settings::default()).await;
    let facts = sheets(&[("3017620422003", SPAGHETTI), ("96385074", PENNE)]);

    let draft = products::lookup_barcode(&db, &facts, "3017620422003").await.unwrap().draft.unwrap();
    assert_eq!(draft.proposed_food_id, None, "« Spaghetti » ne ressemble à aucun aliment");
    let input = ProductInput {
        food_id: Some(pasta),
        group_id: None,
        name: draft.name.clone().unwrap(),
        brand: draft.brand.clone(),
        barcode: Some("3017620422003".into()),
        stock_measure: None,
        package_amount: draft.package_amount,
        package_units: None,
        nutrient_basis: Some(draft.nutrient_basis),
        off_payload: Some(draft.off_payload.clone()),
    };
    let id = products::create(&db, &input).await.unwrap();

    let known = products::lookup_barcode(&db, &facts, "3017620422003").await.unwrap();
    assert_eq!(known.product.unwrap().id, id);
    assert_eq!(facts.asked.lock().unwrap().len(), 1, "produit connu : plus d'appel (R-ALI-61)");
    let mappings = db::products::off_mappings(&db.read).await.unwrap();
    assert_eq!(mappings, vec![("en:spaghetti".to_owned(), Some(pasta), None)]);

    let next = products::lookup_barcode(&db, &facts, "96385074").await.unwrap().draft.unwrap();
    assert_eq!(next.proposed_food_id, Some(pasta), "R-ALI-62");
    assert_eq!(next.proposed_name.as_deref(), Some("Pâtes sèches"));
}

/// Sans catégorie mémorisée ni nom ressemblant, une catégorie en français propose l'aliment (R-ALI-60).
#[tokio::test]
async fn r_ali_60_proposal_from_french_categories() {
    let (db, _dir) = setup().await;
    let g = group(&db, "Féculents", Settings::default()).await;
    let pasta = food(&db, g, "Pâtes sèches", Settings::default()).await;
    let facts = sheets(&[("3017620422003", FUSILLI)]);
    let draft = products::lookup_barcode(&db, &facts, "3017620422003").await.unwrap().draft.unwrap();
    assert_eq!(draft.proposed_food_id, Some(pasta));
}

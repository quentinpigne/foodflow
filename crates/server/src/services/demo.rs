//! Jeu de données de démonstration, entièrement fictif.
//!
//! Généré de façon déterministe à partir de la date du jour, pour que la démonstration paraisse toujours à
//! jour (dates de péremption proches, lots à vérifier). Il passe par les services, donc par toutes les
//! règles. Il sert au développement local et aux tests de bout en bout, jamais à la production.

use anyhow::{Context, anyhow};
use chrono::Days;
use foodflow_domain::foods::{NutrientBasis, NutritionGroup, TrackingMode};
use foodflow_domain::recipes::{RecipeKind, Target, Yield};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, LocationId, ProductId, UnitId};
use foodflow_domain::stock::{ExpiryKind, LocationKind, PresenceState, WasteReason};
use foodflow_domain::{Amount, Date, Measure, Money};

use super::foods::{FoodInput, NutrientValueInput, NutrientsInput};
use super::goals::{self, NewVersionInput};
use super::groups::FoodGroupInput;
use super::products::ProductInput;
use super::recipes::{self, IngredientInput, RecipeInput, Step};
use super::stock::{self, ConsumeInput, DateInput, LotInput, LotOrigin, MoveInput, WasteInput};
use super::{auth, foods, groups, locations, products, seed};
use crate::db::Db;
use crate::db::groups::Settings;

/// Mot de passe du compte de démonstration.
pub const DEMO_PASSWORD: &str = "demo-foodflow";

fn days_before(today: Date, n: u64) -> anyhow::Result<Date> {
    today.checked_sub_days(Days::new(n)).context("date de démonstration")
}

fn days_after(today: Date, n: u64) -> anyhow::Result<Date> {
    today.checked_add_days(Days::new(n)).context("date de démonstration")
}

async fn group(db: &Db, parent: Option<FoodGroupId>, name: &str, settings: Settings) -> anyhow::Result<FoodGroupId> {
    let input = FoodGroupInput { parent_id: parent, name: name.into(), settings, reference_food_id: None };
    groups::create(db, &input).await.map_err(|e| anyhow!("famille {name} : {e}"))
}

/// Valeurs pour 100 g : énergie (kcal), protéines, glucides, lipides, fibres, sel (g).
async fn food(
    db: &Db,
    group: FoodGroupId,
    name: &str,
    aliases: &[&str],
    settings: Settings,
    values: Option<[i64; 6]>,
) -> anyhow::Result<FoodId> {
    let input = FoodInput {
        group_id: group,
        name: name.into(),
        aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
        nutrient_basis: Some(NutrientBasis::Per100G),
        density_mg_per_ml: None,
        piece_weight_mg: None,
        settings,
        minimum_stock: None,
        preferred_product_id: None,
        hidden: false,
    };
    let id = foods::create(db, &input).await.map_err(|e| anyhow!("aliment {name} : {e}"))?;
    if let Some([energy, protein, carbohydrate, fat, fiber, salt]) = values {
        let value =
            |code: &str, value: i64| NutrientValueInput { code: code.into(), value: Some(value), estimated: false };
        let input = NutrientsInput {
            values: vec![
                value("ENERGY", energy * 1_000),
                value("PROTEIN", protein),
                value("CARBOHYDRATE", carbohydrate),
                value("FAT", fat),
                value("FIBER", fiber),
                value("SALT", salt),
            ],
        };
        foods::set_nutrients(db, id, &input).await.map_err(|e| anyhow!("valeurs de {name} : {e}"))?;
    }
    Ok(id)
}

async fn product(
    db: &Db,
    food: FoodId,
    name: &str,
    brand: Option<&str>,
    barcode: Option<&str>,
    package: i64,
) -> anyhow::Result<ProductId> {
    let input = ProductInput {
        food_id: Some(food),
        group_id: None,
        name: name.into(),
        brand: brand.map(str::to_owned),
        barcode: barcode.map(str::to_owned),
        stock_measure: None,
        package_amount: Some(Amount::from_raw(package)),
        package_units: None,
        nutrient_basis: None,
    };
    products::create(db, &input).await.map_err(|e| anyhow!("produit {name} : {e}"))
}

struct Lot<'a> {
    product: Option<ProductId>,
    food: Option<FoodId>,
    location: LocationId,
    stocked_on: Date,
    amount: Option<i64>,
    presence: Option<PresenceState>,
    expiry: (ExpiryKind, Option<Date>),
    price: Option<i64>,
    note: Option<&'a str>,
}

async fn lot(db: &Db, lot: Lot<'_>, today: Date) -> anyhow::Result<foodflow_domain::shared::ids::LotId> {
    let input = LotInput {
        product_id: lot.product,
        food_id: lot.food,
        location_id: lot.location,
        date: Some(lot.stocked_on),
        amount: lot.amount.map(Amount::from_raw),
        presence: lot.presence,
        expiry_kind: lot.expiry.0,
        expiry_date: lot.expiry.1,
        expiry_missing: false,
        unit_price: lot.price.map(Money::from_cents),
        count: None,
        origin: LotOrigin::Purchase,
        note: lot.note.map(str::to_owned),
    };
    let ids = stock::create(db, &input, today).await.map_err(|e| anyhow!("lot : {e}"))?;
    ids.first().copied().context("lot créé")
}

/// Réglages d'une famille : ceux de `a`, complétés par ceux de `b`.
fn merge(a: Settings, b: Settings) -> Settings {
    Settings {
        nutrition_group: a.nutrition_group.or(b.nutrition_group),
        stock_measure: a.stock_measure.or(b.stock_measure),
        tracking_mode: a.tracking_mode.or(b.tracking_mode),
        shelf_life_days: a.shelf_life_days.or(b.shelf_life_days),
        opened_shelf_life_days: a.opened_shelf_life_days.or(b.opened_shelf_life_days),
        frozen_shelf_life_days: a.frozen_shelf_life_days.or(b.frozen_shelf_life_days),
        thawed_shelf_life_days: a.thawed_shelf_life_days.or(b.thawed_shelf_life_days),
        portion_measure: a.portion_measure.or(b.portion_measure),
        portion_amount: a.portion_amount.or(b.portion_amount),
    }
}

/// Densité et poids d'une pièce d'un aliment (R-ALI-31).
async fn set_conversions(db: &Db, id: FoodId, density: Option<i64>, piece: Option<i64>) -> anyhow::Result<()> {
    let detail = foods::get(db, id).await.map_err(|e| anyhow!("{e}"))?;
    let input = FoodInput {
        group_id: detail.summary.group_id,
        name: detail.summary.name.clone(),
        aliases: detail.aliases.clone(),
        nutrient_basis: Some(detail.nutrient_basis),
        density_mg_per_ml: density,
        piece_weight_mg: piece,
        settings: detail.settings,
        minimum_stock: None,
        preferred_product_id: None,
        hidden: false,
    };
    foods::update(db, id, &input).await.map_err(|e| anyhow!("{e}"))
}

/// Un ingrédient de démonstration : quantité et unité générale, ou à volonté.
fn ing(target: Target, quantity: Option<&str>, unit: Option<UnitId>) -> IngredientInput {
    IngredientInput {
        section: None,
        target,
        quantity: quantity.map(str::to_owned),
        unit_id: unit,
        unit_label: None,
        to_taste: quantity.is_none(),
        optional: false,
        fixed: false,
        note: None,
        source_line: None,
        import_name: None,
    }
}

fn step(text: &str, timer: Option<u32>) -> Step {
    Step { section: None, text: text.into(), timer_minutes: timer }
}

struct Draft<'a> {
    name: &'a str,
    kind: RecipeKind,
    yield_: Yield,
    times: (u32, u32),
    ingredients: Vec<IngredientInput>,
    steps: Vec<Step>,
    tags: &'a [&'a str],
}

fn recipe(d: Draft<'_>) -> RecipeInput {
    RecipeInput {
        name: d.name.into(),
        kind: d.kind,
        description: None,
        notes: None,
        yield_: d.yield_,
        active_minutes: d.times.0,
        passive_minutes: d.times.1,
        fridge_days: None,
        freezable: false,
        frozen_days: None,
        finishing_minutes: None,
        leftover_food_id: None,
        source_url: None,
        tags: d.tags.iter().map(|t| (*t).to_owned()).collect(),
        ingredients: d.ingredients,
        steps: d.steps,
    }
}

/// Remplit une base vide avec le jeu de démonstration.
pub async fn load(db: &Db, today: Date) -> anyhow::Result<()> {
    seed::run(db).await?;
    auth::set_password(db, "Démo", DEMO_PASSWORD).await?;
    let places = locations::list(db).await.map_err(|e| anyhow!("{e}"))?;
    let place = |kind: LocationKind| places.iter().find(|l| l.kind == kind).map(|l| l.id).context("emplacement");
    let (pantry, fridge, freezer) =
        (place(LocationKind::Pantry)?, place(LocationKind::Fridge)?, place(LocationKind::Freezer)?);

    let none = Settings::default();
    let volume = Settings { stock_measure: Some(Measure::Volume), opened_shelf_life_days: Some(3), ..none };
    let count = Settings { stock_measure: Some(Measure::Count), ..none };
    let presence = Settings { tracking_mode: Some(TrackingMode::Presence), ..none };
    let fresh = Settings { shelf_life_days: Some(7), frozen_shelf_life_days: Some(240), ..none };
    let meat = Settings {
        shelf_life_days: Some(3),
        frozen_shelf_life_days: Some(180),
        thawed_shelf_life_days: Some(1),
        ..none
    };

    let dairy = group(db, None, "Produits laitiers", none).await?;
    let portion = |group, measure, amount| Settings {
        nutrition_group: Some(group),
        portion_measure: Some(measure),
        portion_amount: Some(Amount::from_raw(amount)),
        ..none
    };
    let milks =
        group(db, Some(dairy), "Laits", merge(volume, portion(NutritionGroup::Dairy, Measure::Volume, 150_000)))
            .await?;
    let cheeses = group(db, Some(dairy), "Fromages", portion(NutritionGroup::Dairy, Measure::Mass, 30_000)).await?;
    let yogurts = group(
        db,
        Some(dairy),
        "Yaourts et desserts lactés",
        merge(
            Settings { opened_shelf_life_days: Some(2), ..count },
            portion(NutritionGroup::Dairy, Measure::Count, 1_000),
        ),
    )
    .await?;
    let produce = group(db, None, "Fruits et légumes", fresh).await?;
    let vegetables =
        group(db, Some(produce), "Légumes", portion(NutritionGroup::Vegetables, Measure::Mass, 80_000)).await?;
    let fruits =
        group(db, Some(produce), "Fruits", merge(count, portion(NutritionGroup::Fruits, Measure::Count, 1_000)))
            .await?;
    let eggs_group =
        group(db, None, "Œufs", merge(count, portion(NutritionGroup::Eggs, Measure::Count, 2_000))).await?;
    let grocery = group(db, None, "Épicerie", none).await?;
    let starches = group(
        db,
        Some(grocery),
        "Pâtes, riz et céréales",
        portion(NutritionGroup::RefinedStarches, Measure::Mass, 60_000),
    )
    .await?;
    let condiments = group(db, Some(grocery), "Condiments et épices", presence).await?;
    let oils = group(
        db,
        Some(grocery),
        "Huiles",
        merge(
            Settings { stock_measure: Some(Measure::Volume), ..presence },
            portion(NutritionGroup::AddedFats, Measure::Volume, 10_000),
        ),
    )
    .await?;
    let meats =
        group(db, None, "Viandes et poissons", merge(meat, portion(NutritionGroup::RedMeat, Measure::Mass, 100_000)))
            .await?;

    let milk = food(db, milks, "Lait demi-écrémé", &["lait"], none, Some([46, 3_200, 4_800, 1_600, 0, 100])).await?;
    let emmental =
        food(db, cheeses, "Emmental râpé", &["gruyère râpé"], none, Some([377, 28_000, 0, 29_000, 0, 700])).await?;
    let yogurt = food(db, yogurts, "Yaourt nature", &[], none, Some([67, 4_200, 5_600, 2_900, 0, 130])).await?;
    let zucchini = food(db, vegetables, "Courgette", &[], none, Some([16, 1_200, 1_800, 300, 1_000, 0])).await?;
    let carrot = food(db, vegetables, "Carotte", &[], none, Some([36, 600, 7_600, 300, 2_700, 100])).await?;
    let apple = food(db, fruits, "Pomme", &[], none, Some([53, 300, 11_400, 300, 1_900, 0])).await?;
    let pasta =
        food(db, starches, "Pâtes sèches", &["spaghetti", "penne"], none, Some([355, 12_000, 71_000, 1_500, 3_000, 0]))
            .await?;
    let rice = food(db, starches, "Riz basmati", &[], none, None).await?;
    let mustard = food(db, condiments, "Moutarde de Dijon", &[], none, None).await?;
    let pepper = food(db, condiments, "Poivre noir", &[], none, None).await?;
    let olive_oil = food(db, oils, "Huile d'olive", &[], none, Some([900, 0, 0, 100_000, 0, 0])).await?;
    let minced =
        food(db, meats, "Bœuf haché 5 %", &["steak haché"], none, Some([125, 21_000, 0, 5_000, 0, 200])).await?;
    let tomato = food(db, vegetables, "Tomate", &[], none, Some([18, 900, 3_900, 200, 1_200, 0])).await?;
    let flour =
        food(db, starches, "Farine de blé", &["farine"], none, Some([340, 10_000, 72_000, 1_200, 4_000, 0])).await?;
    let egg = food(db, eggs_group, "Œuf", &["oeuf"], none, Some([140, 12_500, 300, 9_600, 0, 300])).await?;
    set_conversions(db, flour, Some(550), None).await?;
    set_conversions(db, egg, None, Some(60_000)).await?;
    set_conversions(db, milk, Some(1_030), None).await?;
    set_conversions(db, olive_oil, Some(910), None).await?;

    let lactel = product(db, milk, "Lait demi-écrémé 1 L", Some("Lactel"), Some("2000000000015"), 1_000_000).await?;
    let yogurts_pack = product(db, yogurt, "Yaourt nature × 4", Some("Danone"), None, 4_000).await?;
    let barilla = product(db, pasta, "Spaghetti n° 5 500 g", Some("Barilla"), None, 500_000).await?;

    let d = |n| days_before(today, n);
    let a = |n| days_after(today, n);
    let milk_lot = lot(
        db,
        Lot {
            product: Some(lactel),
            food: None,
            location: fridge,
            stocked_on: d(5)?,
            amount: Some(1_000_000),
            presence: None,
            expiry: (ExpiryKind::UseBy, Some(a(6)?)),
            price: Some(115),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: Some(lactel),
            food: None,
            location: pantry,
            stocked_on: d(5)?,
            amount: Some(1_000_000),
            presence: None,
            expiry: (ExpiryKind::UseBy, Some(a(40)?)),
            price: Some(115),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: Some(yogurts_pack),
            food: None,
            location: fridge,
            stocked_on: d(12)?,
            amount: Some(2_000),
            presence: None,
            expiry: (ExpiryKind::UseBy, Some(d(1)?)),
            price: Some(180),
            note: Some("Fond de pack"),
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(emmental),
            location: fridge,
            stocked_on: d(8)?,
            amount: Some(120_000),
            presence: None,
            expiry: (ExpiryKind::BestBefore, Some(a(1)?)),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    let zucchini_lot = lot(
        db,
        Lot {
            product: None,
            food: Some(zucchini),
            location: fridge,
            stocked_on: d(5)?,
            amount: Some(800_000),
            presence: None,
            expiry: (ExpiryKind::None, None),
            price: Some(239),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(carrot),
            location: fridge,
            stocked_on: d(2)?,
            amount: Some(1_000_000),
            presence: None,
            expiry: (ExpiryKind::None, None),
            price: Some(149),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(apple),
            location: pantry,
            stocked_on: d(3)?,
            amount: Some(6_000),
            presence: None,
            expiry: (ExpiryKind::None, None),
            price: Some(299),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: Some(barilla),
            food: None,
            location: pantry,
            stocked_on: d(20)?,
            amount: Some(500_000),
            presence: None,
            expiry: (ExpiryKind::BestBefore, Some(a(300)?)),
            price: Some(129),
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(rice),
            location: pantry,
            stocked_on: d(30)?,
            amount: Some(1_000_000),
            presence: None,
            expiry: (ExpiryKind::BestBefore, Some(a(200)?)),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(mustard),
            location: fridge,
            stocked_on: d(40)?,
            amount: None,
            presence: Some(PresenceState::Low),
            expiry: (ExpiryKind::BestBefore, Some(a(90)?)),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(pepper),
            location: pantry,
            stocked_on: d(60)?,
            amount: None,
            presence: None,
            expiry: (ExpiryKind::None, None),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    lot(
        db,
        Lot {
            product: None,
            food: Some(olive_oil),
            location: pantry,
            stocked_on: d(50)?,
            amount: None,
            presence: None,
            expiry: (ExpiryKind::BestBefore, Some(a(400)?)),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    let minced_lot = lot(
        db,
        Lot {
            product: None,
            food: Some(minced),
            location: fridge,
            stocked_on: d(12)?,
            amount: Some(500_000),
            presence: None,
            expiry: (ExpiryKind::UseBy, Some(d(10)?)),
            price: Some(649),
            note: None,
        },
        today,
    )
    .await?;

    // Le lait a été ouvert il y a deux jours et entamé ; le bœuf haché a été congelé le lendemain de l'achat.
    let fail = |e: crate::error::AppError| anyhow!("{e}");
    stock::open(db, milk_lot, &DateInput { date: Some(d(2)?) }, today).await.map_err(fail)?;
    let consume = |date, amount| ConsumeInput {
        date: Some(date),
        amount: Some(Amount::from_raw(amount)),
        presence: None,
        note: None,
    };
    stock::consume(db, milk_lot, &consume(d(2)?, 250_000), today).await.map_err(fail)?;
    stock::consume(db, milk_lot, &consume(d(1)?, 200_000), today).await.map_err(fail)?;
    stock::move_lot(db, minced_lot, &MoveInput { date: Some(d(11)?), to_location_id: freezer, amount: None }, today)
        .await
        .map_err(fail)?;
    stock::consume(db, zucchini_lot, &consume(d(1)?, 300_000), today).await.map_err(fail)?;
    // Une sortie trop grande, qui laisse un écart à vérifier (R-STK-3).
    let spare = lot(
        db,
        Lot {
            product: None,
            food: Some(carrot),
            location: pantry,
            stocked_on: d(4)?,
            amount: Some(200_000),
            presence: None,
            expiry: (ExpiryKind::None, None),
            price: None,
            note: None,
        },
        today,
    )
    .await?;
    stock::consume(db, spare, &consume(d(1)?, 300_000), today).await.map_err(fail)?;
    // Un yaourt jeté la semaine dernière.
    let old = lot(
        db,
        Lot {
            product: Some(yogurts_pack),
            food: None,
            location: fridge,
            stocked_on: d(20)?,
            amount: Some(1_000),
            presence: None,
            expiry: (ExpiryKind::UseBy, Some(d(8)?)),
            price: Some(45),
            note: None,
        },
        today,
    )
    .await?;
    stock::waste(
        db,
        old,
        &WasteInput { date: Some(d(7)?), amount: None, reason: WasteReason::Expired, note: None },
        today,
    )
    .await
    .map_err(fail)?;

    // Recettes : une sauce de base, utilisée comme sous-recette (R-REC-31), et trois plats.
    let g = crate::db::units::find_general(&db.read, "g").await?.context("g")?;
    let cl = crate::db::units::find_general(&db.read, "cL").await?.context("cL")?;
    let tbsp = crate::db::units::find_general(&db.read, "cuillère à soupe").await?.context("cuillère à soupe")?;
    let portions = |n: i64| Yield::Portions { portions: Amount::from_raw(n * 1_000) };
    let mut sauce_input = recipe(Draft {
        name: "Sauce tomate maison",
        kind: RecipeKind::Base,
        yield_: Yield::Amount { measure: Measure::Mass, amount: Amount::from_raw(600_000) },
        times: (10, 30),
        ingredients: vec![
            ing(Target::Food(tomato), Some("800"), Some(g)),
            ing(Target::Food(olive_oil), Some("2"), Some(tbsp)),
            ing(Target::Food(pepper), None, None),
        ],
        steps: vec![
            step("Couper les tomates en morceaux.", None),
            step("Cuire à feu doux avec l'huile d'olive.", Some(30)),
        ],
        tags: &["sauce"],
    });
    (sauce_input.fridge_days, sauce_input.freezable, sauce_input.frozen_days) = (Some(4), true, Some(90));
    let sauce = recipes::create(db, &sauce_input).await.map_err(fail)?;
    let pasta_dish = recipe(Draft {
        name: "Pâtes à la sauce tomate",
        kind: RecipeKind::Main,
        yield_: portions(4),
        times: (5, 10),
        ingredients: vec![
            ing(Target::Food(pasta), Some("400"), Some(g)),
            ing(Target::Recipe(sauce), Some("300"), Some(g)),
            ing(Target::Food(emmental), Some("60"), Some(g)),
        ],
        steps: vec![step("Cuire les pâtes.", Some(10)), step("Réchauffer la sauce, servir avec le fromage.", None)],
        tags: &["rapide"],
    });
    recipes::create(db, &pasta_dish).await.map_err(fail)?;
    let mut gratin = recipe(Draft {
        name: "Gratin de courgettes",
        kind: RecipeKind::Main,
        yield_: portions(4),
        times: (20, 45),
        ingredients: vec![
            ing(Target::Food(zucchini), Some("1000"), Some(g)),
            ing(Target::Food(milk), Some("25"), Some(cl)),
            ing(Target::Food(emmental), Some("100"), Some(g)),
            ing(Target::Food(pepper), None, None),
        ],
        steps: vec![
            step("Couper les courgettes en rondelles.", None),
            step("Ajouter le lait et le fromage, enfourner.", Some(45)),
        ],
        tags: &["légumes"],
    });
    gratin.fridge_days = Some(2);
    recipes::create(db, &gratin).await.map_err(fail)?;
    let crepes = recipe(Draft {
        name: "Crêpes",
        kind: RecipeKind::Dessert,
        yield_: portions(6),
        times: (20, 60),
        ingredients: vec![
            ing(Target::Food(flour), Some("250"), Some(g)),
            ing(Target::Food(egg), Some("4"), None),
            ing(Target::Food(milk), Some("50"), Some(cl)),
            ing(Target::Food(olive_oil), Some("1"), Some(tbsp)),
        ],
        steps: vec![
            step("Mélanger la farine et les œufs, puis le lait.", None),
            step("Laisser reposer une heure.", Some(60)),
            step("Cuire les crêpes.", None),
        ],
        tags: &["chandeleur"],
    });
    recipes::create(db, &crepes).await.map_err(fail)?;

    // Objectif du foyer : le modèle PNNS, en vigueur depuis un mois (R-NUT-4).
    let pnns = goals::templates(db).await.map_err(fail)?.into_iter().find(|t| t.builtin).context("modèle PNNS")?;
    let version = NewVersionInput { valid_from: d(30)?, note: None, template_id: Some(pnns.id), copy_from: None };
    goals::create_version(db, &version, today).await.map_err(fail)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::dashboard;

    #[tokio::test]
    async fn demo_loads_and_shows_priorities() {
        let (db, _dir) = crate::db::test_db().await;
        let today: Date = "2026-10-25".parse().unwrap();
        load(&db, today).await.unwrap();
        let board = dashboard::get(&db, today).await.unwrap();
        assert!(board.expired_count >= 1, "yaourts à date dépassée");
        assert!(board.soon_count >= 1);
        assert!(board.priority.iter().any(|l| l.needs_review), "écart à vérifier");
    }
}

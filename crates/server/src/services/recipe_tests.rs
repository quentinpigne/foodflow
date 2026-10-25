//! Tests d'intégration des recettes, sur le jeu de démonstration (spec 03).

use foodflow_domain::Amount;
use foodflow_domain::Measure;
use foodflow_domain::recipes::Target;
use foodflow_domain::recipes::matching::Confidence;

use super::recipe_import::{self, StartImport};
use super::recipes::{self, RecipeQuery, RecipeSummary};
use super::{demo, foods};
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::AppError;

async fn setup() -> (Db, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    demo::load(&db, "2026-10-25".parse().unwrap()).await.unwrap();
    (db, dir)
}

async fn find(db: &Db, name: &str) -> RecipeSummary {
    recipes::search(db, &RecipeQuery { include_archived: true, ..Default::default() })
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name == name)
        .unwrap()
}

/// Sous-recette au prorata, valeurs par portion, temps avec la sous-recette (R-REC-12, R-REC-41, R-REC-50).
#[tokio::test]
async fn r_rec_32_41_50_sub_recipe_values_and_times() {
    let (db, _dir) = setup().await;
    let dish = find(&db, "Pâtes à la sauce tomate").await;
    let detail = recipes::get(&db, dish.id, None).await.unwrap();
    // Temps : sauce (10 + 30), puis pâtes (5 + 10) ; actif 15 min.
    assert_eq!((detail.summary.times.total_active_minutes, detail.summary.times.total_minutes), (15, 55));
    // Énergie par portion : 400 g de pâtes (1 420 kcal) + la moitié de la sauce (800 g de tomate → 144 kcal,
    // 2 c. à s. d'huile → 30 mL × 0,91 g/mL × 9 kcal/g = 245,7 kcal, soit 194,85 kcal) + 60 g d'emmental
    // (226,2 kcal) = 1 841,05 kcal, ÷ 4 = 460,2625 kcal, arrondie au plus proche (ADR-0001).
    let energy = detail.nutrients.iter().find(|n| n.code == "ENERGY").unwrap();
    assert!(energy.missing.is_empty(), "{:?}", energy.missing);
    assert_eq!(energy.value, 460_263);
    // Les fibres du fromage et de l'huile sont connues ici ; le sel du poivre à volonté ne compte pas.
    assert!(detail.nutrients.iter().find(|n| n.code == "SALT").unwrap().missing.is_empty());
    // Groupes : 100 g de pâtes par portion = 1,667 portion de féculents ; 100 g de tomates = 1,25 portion de légumes.
    let starch = detail
        .group_portions
        .iter()
        .find(|g| g.group == foodflow_domain::foods::NutritionGroup::RefinedStarches)
        .unwrap();
    assert_eq!(starch.portions, 1_667);
    let vegetables =
        detail.group_portions.iter().find(|g| g.group == foodflow_domain::foods::NutritionGroup::Vegetables).unwrap();
    assert_eq!(vegetables.portions, 1_250);
}

/// Critère d'acceptation : servir les crêpes (6 portions, 4 œufs) à 2,6 équivalents adulte (R-REC-12, R-REC-14).
#[tokio::test]
async fn r_rec_12_14_scaling_and_half_pieces() {
    let (db, _dir) = setup().await;
    let crepes = find(&db, "Crêpes").await;
    let detail = recipes::get(&db, crepes.id, Some(Amount::from_raw(2_600))).await.unwrap();
    let eggs = detail.ingredients.iter().find(|i| i.target_name == "Œuf").unwrap();
    // 4 × 2,6 ÷ 6 = 1,733… œuf ; affichage arrondi à la demi-pièce : 1,5.
    assert_eq!(eggs.measure, Some(Measure::Count));
    assert_eq!(eggs.amount, Some(Amount::from_raw(1_733)));
    assert_eq!(eggs.display_amount, Some(Amount::from_raw(1_500)));
    assert_eq!(eggs.quantity.as_deref(), Some("1,73"));
    let milk = detail.ingredients.iter().find(|i| i.target_name == "Lait demi-écrémé").unwrap();
    assert_eq!(milk.amount, Some(Amount::from_raw(216_667)), "50 cL × 2,6 ÷ 6, en µL");
}

/// Une recette ne peut pas être son propre ingrédient (R-REC-30) ; une sous-recette utilisée ne se supprime pas.
#[tokio::test]
async fn r_rec_30_cycles_and_delete() {
    let (db, _dir) = setup().await;
    let sauce = find(&db, "Sauce tomate maison").await;
    let dish = find(&db, "Pâtes à la sauce tomate").await;
    let detail = recipes::get(&db, sauce.id, None).await.unwrap();
    assert_eq!(detail.used_by[0].name, "Pâtes à la sauce tomate");
    let exported = recipes::json_ld(&db, sauce.id).await.unwrap();
    assert_eq!(exported["recipeYield"], "600 g");
    // Ajouter les pâtes à la sauce : cycle.
    let mut input = super::recipes::RecipeInput {
        name: "Sauce tomate maison".into(),
        kind: foodflow_domain::recipes::RecipeKind::Base,
        description: None,
        notes: None,
        yield_: foodflow_domain::recipes::Yield::Amount { measure: Measure::Mass, amount: Amount::from_raw(600_000) },
        active_minutes: 10,
        passive_minutes: 30,
        fridge_days: None,
        freezable: false,
        frozen_days: None,
        finishing_minutes: None,
        leftover_food_id: None,
        source_url: None,
        tags: vec![],
        ingredients: vec![],
        steps: vec![],
    };
    input.ingredients.push(super::recipes::IngredientInput {
        section: None,
        target: Target::Recipe(dish.id),
        quantity: Some("1".into()),
        unit_id: None,
        unit_label: None,
        to_taste: false,
        optional: false,
        fixed: false,
        note: None,
        source_line: None,
        import_name: None,
    });
    assert!(matches!(
        recipes::update(&db, sauce.id, &input).await,
        Err(AppError::Domain(foodflow_domain::DomainError::RecipeCycle))
    ));
    assert!(recipes::delete(&db, sauce.id).await.is_err(), "la sauce est utilisée par les pâtes");
    recipes::set_archived(&db, sauce.id, true).await.unwrap();
    assert!(recipes::search(&db, &RecipeQuery::default()).await.unwrap().iter().all(|r| r.id != sauce.id), "R-REC-2");
}

/// « 2 verres de pâtes » sans densité : la quantité est gardée mais rend les totaux incomplets (R-REC-21).
#[tokio::test]
async fn r_rec_21_unconvertible_quantity() {
    let (db, _dir) = setup().await;
    let pasta =
        foods::search(&db, &foods::FoodQuery { q: Some("pâtes sèches".into()), group_id: None, include_hidden: false })
            .await
            .unwrap()[0]
            .id;
    let glass = db::units::find_general(&db.read, "verre").await.unwrap().unwrap();
    let input = super::recipes::RecipeInput {
        name: "Pâtes nature".into(),
        kind: foodflow_domain::recipes::RecipeKind::Main,
        description: None,
        notes: None,
        yield_: foodflow_domain::recipes::Yield::Portions { portions: Amount::from_raw(2_000) },
        active_minutes: 0,
        passive_minutes: 10,
        fridge_days: None,
        freezable: false,
        frozen_days: None,
        finishing_minutes: None,
        leftover_food_id: None,
        source_url: None,
        tags: vec![],
        ingredients: vec![super::recipes::IngredientInput {
            section: None,
            target: Target::Food(pasta),
            quantity: Some("2".into()),
            unit_id: Some(glass),
            unit_label: None,
            to_taste: false,
            optional: false,
            fixed: false,
            note: None,
            source_line: None,
            import_name: None,
        }],
        steps: vec![],
    };
    let id = recipes::create(&db, &input).await.unwrap();
    let detail = recipes::get(&db, id, None).await.unwrap();
    assert!(detail.ingredients[0].unconvertible);
    assert_eq!(detail.ingredients[0].text, "2 verre Pâtes sèches");
    let energy = detail.nutrients.iter().find(|n| n.code == "ENERGY").unwrap();
    assert_eq!(energy.missing, vec!["2 verre Pâtes sèches".to_owned()]);
    assert_eq!(detail.summary.energy, None, "énergie incomplète : pas de chiffre présenté comme exact");
}

/// « Que cuisiner avec ce que j'ai ? » : les recettes les mieux couvertes par le stock d'abord (REC-09).
#[tokio::test]
async fn rec_09_in_stock_and_filters() {
    let (db, _dir) = setup().await;
    let list = recipes::search(&db, &RecipeQuery { in_stock: true, ..Default::default() }).await.unwrap();
    let gratin = list.iter().find(|r| r.name == "Gratin de courgettes").unwrap();
    // Courgettes, lait et emmental sont en stock ; le poivre à volonté ne compte pas.
    assert_eq!(gratin.coverage.map(|c| (c.covered, c.total)), Some((3, 3)));
    assert_eq!(list[0].coverage.map(|c| c.covered == c.total), Some(true));
    let quick = recipes::search(&db, &RecipeQuery { max_active: Some(15), ..Default::default() }).await.unwrap();
    assert!(quick.iter().all(|r| r.times.total_active_minutes <= 15));
    let by_ingredient =
        recipes::search(&db, &RecipeQuery { q: Some("courgette".into()), ..Default::default() }).await.unwrap();
    assert_eq!(by_ingredient.len(), 1);
    let make_ahead = recipes::search(&db, &RecipeQuery { make_ahead: true, ..Default::default() }).await.unwrap();
    assert_eq!(make_ahead.len(), 2, "sauce et gratin");
}

/// Export puis réimport d'une recette : les rattachements mémorisés reviennent (R-REC-70).
#[tokio::test]
async fn r_rec_70_export_and_reimport() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    let gratin = find(&db, "Gratin de courgettes").await;
    let exported = recipes::json_ld(&db, gratin.id).await.unwrap();
    assert_eq!(exported["recipeIngredient"][0], "1000 g Courgette");
    let fetcher = crate::clients::web::StaticFetcher { pages: Default::default() };
    let draft =
        recipe_import::start(&db, &data, &fetcher, &StartImport { url: None, json_ld: Some(exported.to_string()) })
            .await
            .unwrap();
    let zucchini = draft.payload.lines.iter().find(|l| l.name == "Courgette").unwrap();
    assert_eq!(zucchini.confidence, Confidence::High, "nom identique");
    assert_eq!(zucchini.quantity.as_deref(), Some("1000"));
    assert_eq!(draft.payload.steps.len(), 2);
    assert_eq!(draft.payload.portions.as_deref(), Some("4"));
}

//! Import de la table Ciqual (spec 01, ALI-03, R-ALI-50 à R-ALI-52).
//!
//! Idempotent : réimporter la même table ne change rien ; une version plus récente met à jour les aliments
//! et les familles par code Ciqual, sans toucher à ce que l'utilisateur a modifié.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;
use foodflow_domain::foods::nutrients::{NutrientValue, parse_ciqual};
use foodflow_domain::foods::{NutrientBasis, ValueSource};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId};
use foodflow_domain::text::normalize;
use serde::Deserialize;

use super::seed::NutrientSeed;
use crate::db::foods::FoodFields;
use crate::db::groups::{GroupFields, Settings};
use crate::db::{self, Db};
use crate::readers::ciqual::{self, CiqualFiles};

const SELECTION: &str = include_str!("../../seed/ciqual_selection.json");
const GROUP_DEFAULTS: &str = include_str!("../../seed/ciqual_groups.json");

#[derive(Deserialize)]
struct Selection {
    codes: Vec<String>,
}

#[derive(Deserialize)]
struct GroupDefaults {
    groups: HashMap<String, Settings>,
    /// Débuts de noms d'aliments rangés en poisson gras (spec 04) : le groupe Ciqual ne les distingue pas.
    #[serde(default)]
    fatty_fish: Vec<String>,
}

/// Complète les réglages vides d'une famille par ceux par défaut (R-ALI-2) ; ce qui est renseigné reste.
fn fill(current: Settings, defaults: Settings) -> Settings {
    Settings {
        nutrition_group: current.nutrition_group.or(defaults.nutrition_group),
        stock_measure: current.stock_measure.or(defaults.stock_measure),
        tracking_mode: current.tracking_mode.or(defaults.tracking_mode),
        shelf_life_days: current.shelf_life_days.or(defaults.shelf_life_days),
        opened_shelf_life_days: current.opened_shelf_life_days.or(defaults.opened_shelf_life_days),
        frozen_shelf_life_days: current.frozen_shelf_life_days.or(defaults.frozen_shelf_life_days),
        thawed_shelf_life_days: current.thawed_shelf_life_days.or(defaults.thawed_shelf_life_days),
        portion_measure: current.portion_measure.or(defaults.portion_measure),
        portion_amount: current.portion_amount.or(defaults.portion_amount),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub groups_created: usize,
    pub groups_updated: usize,
    pub foods_created: usize,
    pub foods_updated: usize,
    pub values_set: usize,
}

/// Importe la table Ciqual dont les fichiers XML sont dans `dir`.
pub async fn import(db: &Db, dir: &Path) -> anyhow::Result<ImportReport> {
    let files = CiqualFiles::find(dir)?;
    let mut groups = ciqual::parse_groups(&ciqual::read(&files.groups)?).context("groupes Ciqual")?;
    let foods = ciqual::parse_foods(&ciqual::read(&files.foods)?).context("aliments Ciqual")?;
    let nutrients: Vec<NutrientSeed> = super::seed::nutrients()?;
    let constituents: Vec<&str> = nutrients.iter().filter_map(|n| n.ciqual_code.as_deref()).collect();
    let values = ciqual::parse_compositions(&ciqual::read(&files.compositions)?, &constituents)
        .context("compositions Ciqual")?;

    if foods.iter().any(|f| f.group_codes.iter().any(|c| c == ciqual::UNCLASSIFIED)) {
        groups.push(ciqual::CiqualGroup {
            code: ciqual::UNCLASSIFIED.to_owned(),
            name: "Non classés".to_owned(),
            parent_code: None,
        });
    }
    let selection: HashSet<String> = serde_json::from_str::<Selection>(SELECTION)?.codes.into_iter().collect();
    let config = serde_json::from_str::<GroupDefaults>(GROUP_DEFAULTS)?;
    let (defaults, fatty_fish) = (config.groups, config.fatty_fish);
    let mut report = ImportReport::default();
    let mut tx = db.write.begin().await?;

    // Familles, parents d'abord (R-ALI-51).
    let mut group_ids: HashMap<String, FoodGroupId> = HashMap::new();
    let existing: HashMap<FoodGroupId, db::groups::GroupRow> =
        db::groups::list(&mut *tx).await?.into_iter().map(|g| (g.id, g)).collect();
    for group in &groups {
        let parent = group.parent_code.as_ref().and_then(|p| group_ids.get(p)).copied();
        let key = normalize(&group.name);
        match db::groups::find_by_ciqual(&mut *tx, &group.code).await? {
            Some(id) => {
                group_ids.insert(group.code.clone(), id);
                let Some(row) = existing.get(&id) else { continue };
                let settings = match defaults.get(&group.code) {
                    Some(d) if !row.user_modified => fill(row.settings, *d),
                    _ => row.settings,
                };
                if !row.user_modified && (row.name != group.name || row.parent_id != parent || row.settings != settings)
                {
                    let fields = GroupFields {
                        parent_id: parent,
                        name: &group.name,
                        name_key: &key,
                        ciqual_code: Some(&group.code),
                        settings,
                        reference_food_id: row.reference_food_id,
                        user_modified: false,
                    };
                    db::groups::update(&mut *tx, id, &fields).await?;
                    report.groups_updated += 1;
                }
            }
            None => {
                let id = FoodGroupId::new();
                let fields = GroupFields {
                    parent_id: parent,
                    name: &group.name,
                    name_key: &key,
                    ciqual_code: Some(&group.code),
                    settings: defaults.get(&group.code).copied().unwrap_or_default(),
                    reference_food_id: None,
                    user_modified: false,
                };
                db::groups::insert(&mut *tx, id, &fields).await?;
                group_ids.insert(group.code.clone(), id);
                report.groups_created += 1;
            }
        }
    }

    // Aliments (R-ALI-50, R-ALI-52).
    let mut food_ids: HashMap<String, FoodId> = HashMap::new();
    for food in &foods {
        let group = food
            .group_codes
            .iter()
            .find_map(|code| group_ids.get(code))
            .copied()
            .with_context(|| format!("aliment {} : aucun de ses groupes n'existe", food.code))?;
        match db::foods::find_by_ciqual(&mut *tx, &food.code).await? {
            Some(row) => {
                food_ids.insert(food.code.clone(), row.id);
                if !row.user_modified && (row.name != food.name || row.group_id != group) {
                    let key = unique_key(&mut tx, &food.name, &food.code, Some(row.id)).await?;
                    let fields = FoodFields { group_id: group, name: &food.name, name_key: &key, ..fields_of(&row) };
                    db::foods::update(&mut *tx, row.id, &fields).await?;
                    report.foods_updated += 1;
                }
            }
            None => {
                let id = FoodId::new();
                let key = unique_key(&mut tx, &food.name, &food.code, None).await?;
                let lower = normalize(&food.name);
                let fatty = fatty_fish.iter().any(|f| lower.starts_with(&normalize(f)));
                let settings = Settings {
                    nutrition_group: fatty.then_some(foodflow_domain::foods::NutritionGroup::FattyFish),
                    ..Settings::default()
                };
                let fields = FoodFields {
                    group_id: group,
                    name: &food.name,
                    name_key: &key,
                    ciqual_code: Some(&food.code),
                    hidden: !selection.contains(&food.code),
                    nutrient_basis: NutrientBasis::Per100G,
                    density_mg_per_ml: None,
                    piece_weight_mg: None,
                    settings,
                    minimum_stock: None,
                    preferred_product_id: None,
                    user_modified: false,
                };
                db::foods::insert(&mut *tx, id, &fields).await?;
                food_ids.insert(food.code.clone(), id);
                report.foods_created += 1;
            }
        }
    }

    // Valeurs nutritionnelles : une saisie manuelle est conservée, la valeur Ciqual devient sa référence
    // (R-ALI-11) ; une valeur inconnue reste absente (R-ALI-20).
    let by_constituent: HashMap<&str, &NutrientSeed> =
        nutrients.iter().filter_map(|n| n.ciqual_code.as_deref().map(|c| (c, n))).collect();
    let mut current: HashMap<FoodId, HashMap<String, db::nutrients::FoodNutrientRow>> = HashMap::new();
    for value in &values {
        let (Some(food), Some(nutrient)) =
            (food_ids.get(&value.food_code), by_constituent.get(value.constituent_code.as_str()))
        else {
            continue;
        };
        if !current.contains_key(food) {
            let rows = db::nutrients::food_values(&mut *tx, *food).await?;
            current.insert(*food, rows.into_iter().map(|r| (r.code.clone(), r)).collect());
        }
        let before = current.get(food).and_then(|m| m.get(&nutrient.code));
        let parsed = parse_ciqual(&value.raw, nutrient.scale);
        match (before.map(|b| b.value.source), parsed) {
            (Some(ValueSource::Manual), _) => {
                let Some(before) = before else { continue };
                let reference = parsed.map(|(v, _)| v);
                if before.reference_value != reference {
                    db::nutrients::set_food_value(&mut *tx, *food, &nutrient.code, before.value, reference).await?;
                }
            }
            (_, Some((v, estimated))) => {
                let new = NutrientValue { value: v, estimated, source: ValueSource::Ciqual };
                if before.map(|b| b.value) != Some(new) {
                    db::nutrients::set_food_value(&mut *tx, *food, &nutrient.code, new, None).await?;
                    report.values_set += 1;
                }
            }
            (Some(_), None) => db::nutrients::delete_food_value(&mut *tx, *food, &nutrient.code).await?,
            (None, None) => {}
        }
    }
    tx.commit().await?;
    Ok(report)
}

/// Clé unique d'un nom : Ciqual a quelques noms qui ne diffèrent que par les accents ; on les distingue
/// par leur code (R-ALI-10).
async fn unique_key(
    conn: &mut sqlx::SqliteConnection,
    name: &str,
    code: &str,
    except: Option<FoodId>,
) -> anyhow::Result<String> {
    let key = normalize(name);
    if db::foods::name_taken(&mut *conn, &key, except).await? { Ok(format!("{key} ({code})")) } else { Ok(key) }
}

fn fields_of(row: &db::foods::FoodRow) -> FoodFields<'_> {
    FoodFields {
        group_id: row.group_id,
        name: &row.name,
        name_key: &row.name_key,
        ciqual_code: row.ciqual_code.as_deref(),
        hidden: row.hidden,
        nutrient_basis: row.nutrient_basis,
        density_mg_per_ml: row.density_mg_per_ml,
        piece_weight_mg: row.piece_weight_mg,
        settings: row.settings,
        minimum_stock: row.minimum_stock,
        preferred_product_id: row.preferred_product_id,
        user_modified: row.user_modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::foods::{self, NutrientValueInput, NutrientsInput};
    use crate::services::{groups, seed};

    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ciqual")
    }

    async fn food_by_code(db: &Db, code: &str) -> db::foods::FoodRow {
        db::foods::find_by_ciqual(&db.read, code).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn r_ali_50_import_is_idempotent() {
        let (db, _dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        let first = import(&db, &fixtures()).await.unwrap();
        assert_eq!(first.foods_created, 6);
        assert!(first.groups_created >= 10);
        assert!(first.values_set > 30);
        let second = import(&db, &fixtures()).await.unwrap();
        assert_eq!(second, ImportReport::default(), "réimporter la même table ne change rien");

        let milk = food_by_code(&db, "19041").await;
        assert!(!milk.hidden, "aliment courant affiché (R-ALI-52)");
        let values = db::nutrients::food_values(&db.read, milk.id).await.unwrap();
        let energy = values.iter().find(|v| v.code == "ENERGY").unwrap();
        assert_eq!(energy.value.source, ValueSource::Ciqual);
        // Les familles Ciqual reçoivent leurs réglages par défaut : les laits se comptent en volume.
        let all = groups::list(&db).await.unwrap();
        let laits = all.iter().find(|g| g.ciqual_code.as_deref() == Some("0501")).unwrap();
        assert_eq!(laits.effective.stock_measure, foodflow_domain::Measure::Volume);
    }

    /// Critère d'acceptation de la spec 01 : une valeur saisie et une famille renommée survivent au réimport.
    #[tokio::test]
    async fn r_ali_11_and_r_ali_51_user_changes_survive_reimport() {
        let (db, _dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        import(&db, &fixtures()).await.unwrap();
        let milk = food_by_code(&db, "19041").await;
        let input = NutrientsInput {
            values: vec![NutrientValueInput { code: "PROTEIN".into(), value: Some(3_500), estimated: false }],
        };
        foods::set_nutrients(&db, milk.id, &input).await.unwrap();
        let laits =
            groups::list(&db).await.unwrap().into_iter().find(|g| g.ciqual_code.as_deref() == Some("0501")).unwrap();
        let rename = groups::FoodGroupInput {
            parent_id: laits.parent_id,
            name: "Laits et boissons lactées".into(),
            settings: laits.settings,
            reference_food_id: None,
        };
        groups::update(&db, laits.id, &rename).await.unwrap();

        import(&db, &fixtures()).await.unwrap();

        let protein = db::nutrients::food_values(&db.read, milk.id)
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.code == "PROTEIN")
            .unwrap();
        assert_eq!(protein.value.value, 3_500);
        assert_eq!(protein.value.source, ValueSource::Manual);
        assert!(protein.reference_value.is_some(), "valeur Ciqual gardée pour être rétablie");
        let laits = groups::list(&db).await.unwrap().into_iter().find(|g| g.id == laits.id).unwrap();
        assert_eq!(laits.name, "Laits et boissons lactées");

        foods::restore_ciqual(&db, milk.id, "PROTEIN").await.unwrap();
        let restored = db::nutrients::food_values(&db.read, milk.id)
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.code == "PROTEIN")
            .unwrap();
        assert_eq!(restored.value.source, ValueSource::Ciqual);
        assert_eq!(Some(restored.value.value), protein.reference_value);
    }

    #[tokio::test]
    async fn r_ali_20_unknown_values_stay_absent() {
        let (db, _dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        import(&db, &fixtures()).await.unwrap();
        // Dans l'extrait, le sel de l'huile d'olive est inconnu (« - ») : aucune ligne, jamais zéro.
        let oil = food_by_code(&db, "17270").await;
        let values = db::nutrients::food_values(&db.read, oil.id).await.unwrap();
        let unknown = values.iter().filter(|v| v.code == "SALT").count();
        let raw = ciqual::parse_compositions(&ciqual::read(&fixtures().join("compo_extrait.xml")).unwrap(), &["10004"])
            .unwrap()
            .into_iter()
            .find(|v| v.food_code == "17270")
            .unwrap();
        assert_eq!(unknown, usize::from(raw.raw != "-"));
    }
}

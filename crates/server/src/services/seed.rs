//! Données initiales (architecture.md § Migrations) : distinctes des migrations, idempotentes, sans
//! aucune donnée personnelle.
//!
//! - Nutriments et unités générales : données de référence, complétées à chaque démarrage.
//! - Modèles d'objectif fournis (« Repères PNNS 2019 ») : rechargés à chaque démarrage, non modifiables.
//! - Emplacements, créneaux de repas et semaine type : proposés **au premier lancement** seulement, pour
//!   ne jamais recréer ce que l'utilisateur a supprimé ou renommé.

use foodflow_domain::Ratio;
use foodflow_domain::foods::NutritionGroup;
use foodflow_domain::foods::nutrients::NutrientUnit;
use foodflow_domain::nutrition::Criterion;
use foodflow_domain::recipes::RecipeKind;
use foodflow_domain::shared::ids::{GoalTemplateId, LocationId, MealSlotId, SlotRoleId, UnitId};
use foodflow_domain::stock::LocationKind;
use foodflow_domain::text::normalize;
use foodflow_domain::{Amount, Measure};
use serde::Deserialize;

use crate::db::goals::{Owner, SlotRow, TemplateRow};
use crate::db::locations::LocationFields;
use crate::db::nutrients::NutrientRow;
use crate::db::planning::{RoleRow, ScheduleRow};
use crate::db::units::UnitFields;
use crate::db::{self, Db};

const NUTRIENTS: &str = include_str!("../../seed/nutrients.json");
const UNITS: &str = include_str!("../../seed/units.json");
const LOCATIONS: &str = include_str!("../../seed/locations.json");
const TEMPLATES: &str = include_str!("../../seed/goal_templates.json");
const SLOTS: &str = include_str!("../../seed/meal_slots.json");

#[derive(Deserialize)]
struct TemplateFile {
    templates: Vec<TemplateSeed>,
}

#[derive(Deserialize)]
struct TemplateSeed {
    name: String,
    criteria: Vec<Criterion>,
}

#[derive(Deserialize)]
struct SlotFile {
    slots: Vec<SlotSeed>,
}

#[derive(Deserialize)]
struct SlotSeed {
    name: String,
    energy_share: i64,
    weekday: TimesSeed,
    weekend: TimesSeed,
    #[serde(default)]
    roles: Vec<RoleSeed>,
}

#[derive(Deserialize)]
struct TimesSeed {
    minutes: u32,
    earliest_start: String,
    ready_by: String,
}

#[derive(Deserialize)]
struct RoleSeed {
    name: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    recipe_kinds: Vec<RecipeKind>,
    #[serde(default)]
    nutrition_groups: Vec<NutritionGroup>,
}

/// Horaires par défaut d'un créneau qui n'est pas dans le fichier (créé par l'utilisateur).
const FALLBACK_TIMES: (u32, &str, &str) = (20, "12:00", "13:00");

#[derive(Debug, Deserialize)]
pub struct NutrientSeed {
    pub code: String,
    pub name: String,
    pub storage_unit: NutrientUnit,
    pub ciqual_code: Option<String>,
    /// Facteur entre l'unité de la table Ciqual et l'unité de stockage (g → mg : 1 000).
    pub scale: i64,
}

#[derive(Deserialize)]
struct NutrientFile {
    nutrients: Vec<NutrientSeed>,
}

#[derive(Deserialize)]
struct UnitSeed {
    name: String,
    plural: Option<String>,
    abbreviation: Option<String>,
    measure: Measure,
    amount: i64,
}

#[derive(Deserialize)]
struct UnitFile {
    units: Vec<UnitSeed>,
}

#[derive(Deserialize)]
struct LocationSeed {
    name: String,
    kind: LocationKind,
    soon_days: Option<u32>,
}

#[derive(Deserialize)]
struct LocationFile {
    locations: Vec<LocationSeed>,
}

/// Liste de référence des nutriments (modèle de données, `Nutrient`).
pub fn nutrients() -> anyhow::Result<Vec<NutrientSeed>> {
    Ok(serde_json::from_str::<NutrientFile>(NUTRIENTS)?.nutrients)
}

pub async fn run(db: &Db) -> anyhow::Result<()> {
    let mut tx = db.write.begin().await?;

    for (position, n) in nutrients()?.into_iter().enumerate() {
        let row = NutrientRow {
            code: n.code,
            name: n.name,
            storage_unit: n.storage_unit,
            ciqual_code: n.ciqual_code,
            position: i64::try_from(position)?,
        };
        db::nutrients::upsert(&mut *tx, &row).await?;
    }

    let units: UnitFile = serde_json::from_str(UNITS)?;
    for (position, unit) in units.units.iter().enumerate() {
        if db::units::find_general(&mut *tx, &unit.name).await?.is_none() {
            let fields = UnitFields {
                food_id: None,
                name: &unit.name,
                plural: unit.plural.as_deref(),
                abbreviation: unit.abbreviation.as_deref(),
                measure: unit.measure,
                amount: Amount::from_raw(unit.amount),
                position: i64::try_from(position)?,
            };
            db::units::insert(&mut *tx, UnitId::new(), &fields).await?;
        }
    }

    if db::settings::get::<bool>(&mut *tx, "seed.locations").await?.is_none() {
        if db::locations::count(&mut *tx).await? == 0 {
            let seed: LocationFile = serde_json::from_str(LOCATIONS)?;
            for (position, location) in seed.locations.iter().enumerate() {
                let key = normalize(&location.name);
                let fields = LocationFields {
                    name: &location.name,
                    name_key: &key,
                    kind: location.kind,
                    position: i64::try_from(position)?,
                    soon_days: location.soon_days,
                };
                db::locations::insert(&mut *tx, LocationId::new(), &fields).await?;
            }
        }
        db::settings::set(&mut *tx, "seed.locations", &true).await?;
    }

    let templates: TemplateFile = serde_json::from_str(TEMPLATES)?;
    let existing = db::goals::templates(&mut *tx).await?;
    for template in templates.templates {
        for criterion in &template.criteria {
            criterion.check().map_err(|e| anyhow::anyhow!("modèle {} : {e}", template.name))?;
        }
        let id = match existing.iter().find(|t| t.name == template.name) {
            Some(t) => t.id,
            None => {
                let row = TemplateRow { id: GoalTemplateId::new(), name: template.name.clone(), builtin: true };
                db::goals::insert_template(&mut *tx, &row).await?;
                row.id
            }
        };
        db::goals::replace_criteria(&mut tx, Owner::Template(id), &template.criteria).await?;
    }

    if db::settings::get::<bool>(&mut *tx, "seed.meal_slots").await?.is_none() {
        if db::goals::slots(&mut *tx).await?.is_empty() {
            let seed: SlotFile = serde_json::from_str(SLOTS)?;
            for (position, slot) in seed.slots.into_iter().enumerate() {
                let row = SlotRow {
                    id: MealSlotId::new(),
                    name: slot.name,
                    position: i64::try_from(position)?,
                    energy_share: Ratio::from_thousandths(slot.energy_share),
                    active: true,
                };
                db::goals::upsert_slot(&mut *tx, &row).await?;
            }
        }
        db::settings::set(&mut *tx, "seed.meal_slots", &true).await?;
    }

    // Semaine type et composition type par défaut des créneaux qui n'en ont pas (ADR-0011, spec 05).
    if db::settings::get::<bool>(&mut *tx, "seed.week_template").await?.is_none() {
        let seed: SlotFile = serde_json::from_str(SLOTS)?;
        let schedules = db::planning::schedules(&mut *tx).await?;
        let roles = db::planning::roles(&mut *tx).await?;
        for slot in db::goals::slots(&mut *tx).await? {
            let found = seed.slots.iter().find(|s| s.name == slot.name);
            if !schedules.iter().any(|s| s.slot_id == slot.id) {
                for weekday in 1..=7 {
                    let (minutes, start, ready) = match found {
                        Some(s) => {
                            let times = if weekday >= 6 { &s.weekend } else { &s.weekday };
                            (times.minutes, times.earliest_start.as_str(), times.ready_by.as_str())
                        }
                        None => FALLBACK_TIMES,
                    };
                    let row = ScheduleRow {
                        slot_id: slot.id,
                        weekday,
                        available_active_minutes: minutes,
                        earliest_start: db::planning::time_from_db(start)?,
                        ready_by: db::planning::time_from_db(ready)?,
                        out_of_home: false,
                        habitual_meal_id: None,
                    };
                    db::planning::upsert_schedule(&mut *tx, &row).await?;
                }
            }
            if let Some(found) = found
                && !roles.iter().any(|r| r.slot_id == slot.id)
            {
                let rows: Vec<RoleRow> = found
                    .roles
                    .iter()
                    .enumerate()
                    .map(|(position, r)| {
                        Ok(RoleRow {
                            id: SlotRoleId::new(),
                            slot_id: slot.id,
                            position: i64::try_from(position)?,
                            name: r.name.clone(),
                            required: r.required,
                            recipe_kinds: r.recipe_kinds.clone(),
                            nutrition_groups: r.nutrition_groups.clone(),
                        })
                    })
                    .collect::<anyhow::Result<_>>()?;
                db::planning::replace_roles(&mut tx, slot.id, &rows).await?;
            }
        }
        db::settings::set(&mut *tx, "seed.week_template", &true).await?;
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_is_idempotent_and_runs_locations_once() {
        let (db, _dir) = db::test_db().await;
        run(&db).await.unwrap();
        assert_eq!(db::nutrients::list(&db.read).await.unwrap().len(), 8);
        let units = db::units::list(&db.read, None).await.unwrap();
        assert!(units.iter().any(|u| u.name == "cuillère à soupe" && u.amount == Amount::from_raw(15_000)));
        let locations = db::locations::list(&db.read).await.unwrap();
        assert_eq!(locations.len(), 3);

        // Un emplacement supprimé n'est pas recréé au démarrage suivant.
        db::locations::delete(&db.write, locations[0].id).await.unwrap();
        run(&db).await.unwrap();
        assert_eq!(db::locations::list(&db.read).await.unwrap().len(), 2);
        assert_eq!(db::units::list(&db.read, None).await.unwrap().len(), units.len());
        // Le modèle PNNS est rechargé sans doublon ; les créneaux totalisent 100 % (R-NUT-30).
        assert_eq!(db::goals::templates(&db.read).await.unwrap().len(), 1);
        let criteria = db::goals::criteria(&db.read).await.unwrap();
        assert_eq!(criteria.len(), 18);
        let shares: Vec<_> = db::goals::slots(&db.read).await.unwrap().iter().map(|s| s.energy_share).collect();
        assert!(foodflow_domain::nutrition::check_energy_shares(&shares).is_ok());
    }
}

//! Données initiales (architecture.md § Migrations) : distinctes des migrations, idempotentes, sans
//! aucune donnée personnelle.
//!
//! - Nutriments et unités générales : données de référence, complétées à chaque démarrage.
//! - Emplacements : proposés **au premier lancement** seulement, pour ne jamais recréer ce que l'utilisateur
//!   a supprimé ou renommé.

use foodflow_domain::foods::nutrients::NutrientUnit;
use foodflow_domain::shared::ids::{LocationId, UnitId};
use foodflow_domain::stock::LocationKind;
use foodflow_domain::text::normalize;
use foodflow_domain::{Amount, Measure};
use serde::Deserialize;

use crate::db::locations::LocationFields;
use crate::db::nutrients::NutrientRow;
use crate::db::units::UnitFields;
use crate::db::{self, Db};

const NUTRIENTS: &str = include_str!("../../seed/nutrients.json");
const UNITS: &str = include_str!("../../seed/units.json");
const LOCATIONS: &str = include_str!("../../seed/locations.json");

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
    }
}

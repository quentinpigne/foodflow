//! Emplacements (spec 02, STK-01).

use foodflow_domain::DomainError;
use foodflow_domain::shared::ids::LocationId;
use foodflow_domain::stock::LocationKind;
use foodflow_domain::text::normalize;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::required;
use crate::db::locations::LocationFields;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct Location {
    pub id: LocationId,
    pub name: String,
    pub kind: LocationKind,
    pub position: i64,
    /// Seuil « bientôt » propre, en jours ; `null` : réglage général (R-GAS-2).
    pub soon_days: Option<u32>,
    /// Lots non épuisés.
    pub lot_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LocationInput {
    pub name: String,
    pub kind: LocationKind,
    pub position: Option<i64>,
    pub soon_days: Option<u32>,
}

pub async fn list(db: &Db) -> AppResult<Vec<Location>> {
    Ok(db::locations::list(&db.read)
        .await?
        .into_iter()
        .map(|l| Location {
            id: l.id,
            name: l.name,
            kind: l.kind,
            position: l.position,
            soon_days: l.soon_days,
            lot_count: l.lot_count,
        })
        .collect())
}

pub async fn create(db: &Db, input: &LocationInput) -> AppResult<LocationId> {
    let name = required(&input.name, "name", 60)?;
    let key = normalize(&name);
    let mut tx = db.write.begin().await?;
    if db::locations::name_taken(&mut *tx, &key, None).await? {
        return Err(DomainError::LocationNameTaken.into());
    }
    let position = match input.position {
        Some(p) => p,
        None => db::locations::count(&mut *tx).await?,
    };
    let id = LocationId::new();
    let fields = LocationFields { name: &name, name_key: &key, kind: input.kind, position, soon_days: input.soon_days };
    db::locations::insert(&mut *tx, id, &fields).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: LocationId, input: &LocationInput) -> AppResult<()> {
    let name = required(&input.name, "name", 60)?;
    let key = normalize(&name);
    let mut tx = db.write.begin().await?;
    let current = db::locations::list(&mut *tx).await?.into_iter().find(|l| l.id == id);
    let current = current.ok_or(AppError::NotFound("emplacement"))?;
    if db::locations::name_taken(&mut *tx, &key, Some(id)).await? {
        return Err(DomainError::LocationNameTaken.into());
    }
    // Changer la nature d'un emplacement qui contient des lots fausserait leur congélation (R-STK-32).
    if current.kind != input.kind && current.lot_count > 0 {
        return Err(AppError::invalid_field(
            "LOCATION_KIND_LOCKED",
            "kind",
            "un emplacement qui contient des lots ne change pas de nature",
        ));
    }
    let fields = LocationFields {
        name: &name,
        name_key: &key,
        kind: input.kind,
        position: input.position.unwrap_or(current.position),
        soon_days: input.soon_days,
    };
    db::locations::update(&mut *tx, id, &fields).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete(db: &Db, id: LocationId) -> AppResult<()> {
    if !db::locations::delete(&db.write, id).await? {
        return Err(DomainError::LocationInUse.into());
    }
    Ok(())
}

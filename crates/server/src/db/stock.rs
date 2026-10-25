//! Lots et mouvements de stock (spec 02, ADR-0002).
//!
//! La quantité d'un lot n'est pas une colonne : elle se calcule depuis le journal (domaine). `depleted`,
//! `opened_on`, `frozen_on`, `thawed_on` et `checked_until` sont des copies de lecture, tenues à jour par
//! le service à chaque mouvement (modèle de données).

use foodflow_domain::Date;
use foodflow_domain::foods::TrackingMode;
use foodflow_domain::shared::ids::{BatchId, FoodId, LocationId, LotId, MovementId, ProductId, RecipeId};
use foodflow_domain::stock::journal::Movement;
use foodflow_domain::stock::{ExpiryKind, MovementKind, PresenceState, WasteReason};
use foodflow_domain::{Amount, Measure, Money};
use sqlx::SqliteExecutor;

use super::codec::{
    date_from_db, enum_from_db, enum_to_db, id_from_db, opt_date_from_db, opt_enum_from_db, opt_enum_to_db,
    opt_id_from_db,
};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct LotRow {
    pub id: LotId,
    pub product_id: Option<ProductId>,
    pub food_id: Option<FoodId>,
    /// Lot de préparation ou de restes (R-STK-1, ADR-0012) : en portions, ou dans la mesure du rendement.
    pub recipe_id: Option<RecipeId>,
    pub location_id: LocationId,
    pub tracking_mode: TrackingMode,
    pub measure: Measure,
    pub expiry_kind: ExpiryKind,
    pub expiry_date: Option<Date>,
    pub expiry_missing: bool,
    pub stocked_on: Date,
    pub opened_on: Option<Date>,
    pub frozen_on: Option<Date>,
    pub thawed_on: Option<Date>,
    pub checked_until: Option<Date>,
    pub unit_price: Option<Money>,
    pub needs_review: bool,
    pub depleted: bool,
    pub note: Option<String>,
}

macro_rules! lot_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(LotRow {
            id: id_from_db(&r.id)?,
            product_id: opt_id_from_db(r.product_id.as_deref())?,
            food_id: opt_id_from_db(r.food_id.as_deref())?,
            recipe_id: opt_id_from_db(r.recipe_id.as_deref())?,
            location_id: id_from_db(&r.location_id)?,
            tracking_mode: enum_from_db(&r.tracking_mode)?,
            measure: enum_from_db(&r.measure)?,
            expiry_kind: enum_from_db(&r.expiry_kind)?,
            expiry_date: opt_date_from_db(r.expiry_date.as_deref())?,
            expiry_missing: r.expiry_missing,
            stocked_on: date_from_db(&r.stocked_on)?,
            opened_on: opt_date_from_db(r.opened_on.as_deref())?,
            frozen_on: opt_date_from_db(r.frozen_on.as_deref())?,
            thawed_on: opt_date_from_db(r.thawed_on.as_deref())?,
            checked_until: opt_date_from_db(r.checked_until.as_deref())?,
            unit_price: r.unit_price.map(Money::from_cents),
            needs_review: r.needs_review,
            depleted: r.depleted,
            note: r.note,
        })
    }};
}

/// Lots, épuisés compris ou non. Les filtres facultatifs se combinent.
pub async fn list(
    db: impl SqliteExecutor<'_>,
    location: Option<LocationId>,
    food: Option<FoodId>,
    include_depleted: bool,
) -> anyhow::Result<Vec<LotRow>> {
    let location = location.map(|l| l.to_string());
    let food = food.map(|f| f.to_string());
    let rows = sqlx::query!(
        r#"SELECT s.id, s.product_id, s.food_id, s.recipe_id, s.location_id, s.tracking_mode, s.measure, s.expiry_kind,
                  s.expiry_date, s.expiry_missing AS "expiry_missing: bool", s.stocked_on, s.opened_on, s.frozen_on,
                  s.thawed_on, s.checked_until, s.unit_price, s.needs_review AS "needs_review: bool",
                  s.depleted AS "depleted: bool", s.note
           FROM stock_lot s LEFT JOIN product p ON p.id = s.product_id
           WHERE (?1 IS NULL OR s.location_id = ?1)
             AND (?2 IS NULL OR s.food_id = ?2 OR p.food_id = ?2)
             AND (?3 OR s.depleted = 0)
           ORDER BY s.created_at"#,
        location,
        food,
        include_depleted
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| lot_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: LotId) -> anyhow::Result<Option<LotRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, product_id, food_id, recipe_id, location_id, tracking_mode, measure, expiry_kind, expiry_date,
                  expiry_missing AS "expiry_missing: bool", stocked_on, opened_on, frozen_on, thawed_on,
                  checked_until, unit_price, needs_review AS "needs_review: bool", depleted AS "depleted: bool", note
           FROM stock_lot WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| lot_row!(r)).transpose()
}

pub async fn insert(db: impl SqliteExecutor<'_>, lot: &LotRow) -> anyhow::Result<()> {
    let id = lot.id.to_string();
    let product = lot.product_id.map(|x| x.to_string());
    let food = lot.food_id.map(|x| x.to_string());
    let recipe = lot.recipe_id.map(|x| x.to_string());
    let location = lot.location_id.to_string();
    let mode = enum_to_db(&lot.tracking_mode);
    let measure = enum_to_db(&lot.measure);
    let kind = enum_to_db(&lot.expiry_kind);
    let expiry = lot.expiry_date.map(|d| d.to_string());
    let stocked = lot.stocked_on.to_string();
    let opened = lot.opened_on.map(|d| d.to_string());
    let frozen = lot.frozen_on.map(|d| d.to_string());
    let thawed = lot.thawed_on.map(|d| d.to_string());
    let checked = lot.checked_until.map(|d| d.to_string());
    let price = lot.unit_price.map(Money::cents);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO stock_lot (id, product_id, food_id, recipe_id, location_id, tracking_mode, measure, expiry_kind,
             expiry_date, expiry_missing, stocked_on, opened_on, frozen_on, thawed_on, checked_until, unit_price,
             needs_review, depleted, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        product,
        food,
        recipe,
        location,
        mode,
        measure,
        kind,
        expiry,
        lot.expiry_missing,
        stocked,
        opened,
        frozen,
        thawed,
        checked,
        price,
        lot.needs_review,
        lot.depleted,
        lot.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Met à jour les champs modifiables d'un lot et ses copies de lecture.
pub async fn update(db: impl SqliteExecutor<'_>, lot: &LotRow) -> anyhow::Result<()> {
    let id = lot.id.to_string();
    let location = lot.location_id.to_string();
    let mode = enum_to_db(&lot.tracking_mode);
    let kind = enum_to_db(&lot.expiry_kind);
    let expiry = lot.expiry_date.map(|d| d.to_string());
    let opened = lot.opened_on.map(|d| d.to_string());
    let frozen = lot.frozen_on.map(|d| d.to_string());
    let thawed = lot.thawed_on.map(|d| d.to_string());
    let checked = lot.checked_until.map(|d| d.to_string());
    let price = lot.unit_price.map(Money::cents);
    let now = timestamp();
    sqlx::query!(
        "UPDATE stock_lot SET location_id = ?, tracking_mode = ?, expiry_kind = ?, expiry_date = ?, expiry_missing = ?,
             opened_on = ?, frozen_on = ?, thawed_on = ?, checked_until = ?, unit_price = ?, needs_review = ?,
             depleted = ?, note = ?, updated_at = ?
         WHERE id = ?",
        location,
        mode,
        kind,
        expiry,
        lot.expiry_missing,
        opened,
        frozen,
        thawed,
        checked,
        price,
        lot.needs_review,
        lot.depleted,
        lot.note,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct MovementRow {
    pub id: MovementId,
    pub lot_id: LotId,
    pub date: Date,
    pub kind: MovementKind,
    pub amount: Option<Amount>,
    pub presence: Option<PresenceState>,
    pub shortfall: Option<Amount>,
    pub waste_reason: Option<WasteReason>,
    pub from_location_id: Option<LocationId>,
    pub to_location_id: Option<LocationId>,
    pub checked_until: Option<Date>,
    pub reverses_id: Option<MovementId>,
    pub batch_id: Option<BatchId>,
    pub note: Option<String>,
    pub created_at: String,
}

impl MovementRow {
    pub fn to_domain(&self) -> Movement {
        Movement {
            id: self.id,
            date: self.date,
            kind: self.kind,
            amount: self.amount,
            presence: self.presence,
            reverses: self.reverses_id,
        }
    }
}

macro_rules! movement_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(MovementRow {
            id: id_from_db(&r.id)?,
            lot_id: id_from_db(&r.lot_id)?,
            date: date_from_db(&r.date)?,
            kind: enum_from_db(&r.kind)?,
            amount: r.amount.map(Amount::from_raw),
            presence: opt_enum_from_db(r.presence.as_deref())?,
            shortfall: r.shortfall.map(Amount::from_raw),
            waste_reason: opt_enum_from_db(r.waste_reason.as_deref())?,
            from_location_id: opt_id_from_db(r.from_location_id.as_deref())?,
            to_location_id: opt_id_from_db(r.to_location_id.as_deref())?,
            checked_until: opt_date_from_db(r.checked_until.as_deref())?,
            reverses_id: opt_id_from_db(r.reverses_id.as_deref())?,
            batch_id: opt_id_from_db(r.batch_id.as_deref())?,
            note: r.note,
            created_at: r.created_at,
        })
    }};
}

/// Mouvements de tous les lots non épuisés (ou de tous les lots) : un seul aller-retour pour la vue du stock.
pub async fn movements_of_active_lots(
    db: impl SqliteExecutor<'_>,
    include_depleted: bool,
) -> anyhow::Result<Vec<MovementRow>> {
    let rows = sqlx::query!(
        "SELECT m.id, m.lot_id, m.date, m.kind, m.amount, m.presence, m.shortfall, m.waste_reason, m.from_location_id,
                m.to_location_id, m.checked_until, m.reverses_id, m.batch_id, m.note, m.created_at
         FROM stock_movement m JOIN stock_lot s ON s.id = m.lot_id
         WHERE ? OR s.depleted = 0
         ORDER BY m.date, m.id",
        include_depleted
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| movement_row!(r)).collect()
}

pub async fn movements_of_lot(db: impl SqliteExecutor<'_>, lot: LotId) -> anyhow::Result<Vec<MovementRow>> {
    let lot = lot.to_string();
    let rows = sqlx::query!(
        "SELECT id, lot_id, date, kind, amount, presence, shortfall, waste_reason, from_location_id, to_location_id,
                checked_until, reverses_id, batch_id, note, created_at
         FROM stock_movement WHERE lot_id = ? ORDER BY date, id",
        lot
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| movement_row!(r)).collect()
}

pub async fn find_movement(db: impl SqliteExecutor<'_>, id: MovementId) -> anyhow::Result<Option<MovementRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        "SELECT id, lot_id, date, kind, amount, presence, shortfall, waste_reason, from_location_id, to_location_id,
                checked_until, reverses_id, batch_id, note, created_at
         FROM stock_movement WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| movement_row!(r)).transpose()
}

pub async fn movements_of_batch(db: impl SqliteExecutor<'_>, batch: BatchId) -> anyhow::Result<Vec<MovementRow>> {
    let batch = batch.to_string();
    let rows = sqlx::query!(
        "SELECT id, lot_id, date, kind, amount, presence, shortfall, waste_reason, from_location_id, to_location_id,
                checked_until, reverses_id, batch_id, note, created_at
         FROM stock_movement WHERE batch_id = ? ORDER BY date, id",
        batch
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| movement_row!(r)).collect()
}

pub async fn insert_movement(db: impl SqliteExecutor<'_>, m: &MovementRow) -> anyhow::Result<()> {
    let id = m.id.to_string();
    let lot = m.lot_id.to_string();
    let date = m.date.to_string();
    let kind = enum_to_db(&m.kind);
    let amount = m.amount.map(Amount::raw);
    let presence = opt_enum_to_db(&m.presence);
    let shortfall = m.shortfall.map(Amount::raw);
    let reason = opt_enum_to_db(&m.waste_reason);
    let from = m.from_location_id.map(|x| x.to_string());
    let to = m.to_location_id.map(|x| x.to_string());
    let checked = m.checked_until.map(|d| d.to_string());
    let reverses = m.reverses_id.map(|x| x.to_string());
    let batch = m.batch_id.map(|x| x.to_string());
    sqlx::query!(
        "INSERT INTO stock_movement (id, lot_id, date, kind, amount, presence, shortfall, waste_reason,
             from_location_id, to_location_id, checked_until, reverses_id, batch_id, note, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        lot,
        date,
        kind,
        amount,
        presence,
        shortfall,
        reason,
        from,
        to,
        checked,
        reverses,
        batch,
        m.note,
        m.created_at
    )
    .execute(db)
    .await?;
    Ok(())
}

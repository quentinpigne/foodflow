//! Stock : lots, mouvements, inventaire (spec 02, STK-02 à STK-05, ADR-0002).
//!
//! Chaque cas d'usage charge le lot et son journal, fait vérifier et calculer le domaine, écrit les
//! mouvements, puis remet à jour les copies de lecture du lot (`depleted`, dates d'ouverture…).

use std::collections::HashMap;
use std::sync::Arc;

use foodflow_domain::foods::TrackingMode;
use foodflow_domain::shared::ids::{BatchId, FoodId, LocationId, LotId, MovementId, ProductId, RecipeId};
use foodflow_domain::stock::expiry::{self, EffectiveExpiry, ExpiryInput};
use foodflow_domain::stock::journal::{self, LotState};
use foodflow_domain::stock::{ExpiryKind, ExpiryStatus, LocationKind, MovementKind, PresenceState, WasteReason};
use foodflow_domain::{Amount, Date, DomainError, Measure, Money};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use utoipa::{IntoParams, ToSchema};

use super::catalog::Catalog;
use super::clean;
use crate::clock::timestamp;
use crate::db::locations::LocationRow;
use crate::db::stock::{LotRow, MovementRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

// ===================================================================== Vues

#[derive(Debug, Serialize, ToSchema)]
pub struct Lot {
    pub id: LotId,
    pub product_id: Option<ProductId>,
    /// Aliment du lot : le sien, ou celui de son produit.
    pub food_id: Option<FoodId>,
    /// Lot de préparation ou de restes (STK-08) : la recette, en portions.
    pub recipe_id: Option<RecipeId>,
    /// Nom affiché : le produit (et sa marque), ou l'aliment.
    pub name: String,
    pub brand: Option<String>,
    /// Aliment ou famille de rattachement d'un produit.
    pub food_name: Option<String>,
    pub location_id: LocationId,
    pub location_name: String,
    pub location_kind: LocationKind,
    pub tracking_mode: TrackingMode,
    pub measure: Measure,
    /// Quantité calculée (lot en `QUANTITY`, R-STK-2).
    pub quantity: Option<Amount>,
    /// État (lot en `PRESENCE`).
    pub presence: Option<PresenceState>,
    pub expiry_kind: ExpiryKind,
    pub expiry_date: Option<Date>,
    /// Date de péremption à compléter (R-COU-51).
    pub expiry_missing: bool,
    pub effective_expiry: Option<EffectiveExpiry>,
    pub status: ExpiryStatus,
    pub stocked_on: Date,
    pub opened_on: Option<Date>,
    pub frozen_on: Option<Date>,
    pub thawed_on: Option<Date>,
    pub checked_until: Option<Date>,
    pub unit_price: Option<Money>,
    /// Écart à vérifier (R-STK-3).
    pub needs_review: bool,
    pub depleted: bool,
    pub note: Option<String>,
    /// Date du dernier inventaire : aucun mouvement ne la précède (R-STK-14).
    pub last_inventory: Option<Date>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StockMovement {
    pub id: MovementId,
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
    /// Annulé par un mouvement inverse (R-STK-11).
    pub reversed: bool,
    pub batch_id: Option<BatchId>,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LotDetail {
    #[serde(flatten)]
    pub lot: Lot,
    pub movements: Vec<StockMovement>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct LotQuery {
    pub location_id: Option<LocationId>,
    pub food_id: Option<FoodId>,
    #[serde(default)]
    pub include_depleted: bool,
}

/// Ce qu'il faut pour présenter des lots : référentiel, emplacements, réglages.
pub struct Context {
    catalog: Arc<Catalog>,
    recipes: HashMap<RecipeId, String>,
    locations: HashMap<LocationId, LocationRow>,
    soon_days: u32,
    today: Date,
}

impl Context {
    pub async fn load(conn: &mut SqliteConnection, today: Date) -> anyhow::Result<Self> {
        let catalog = Catalog::load(&mut *conn).await?;
        let locations = db::locations::list(&mut *conn).await?.into_iter().map(|l| (l.id, l)).collect();
        let soon_days = super::settings::get(&mut *conn).await?.soon_days;
        let recipes = db::recipes::list(&mut *conn).await?.into_iter().map(|r| (r.id, r.name)).collect();
        Ok(Self { catalog, recipes, locations, soon_days, today })
    }

    fn settings(&self, lot: &LotRow) -> Option<super::catalog::EffectiveSettings> {
        match (lot.product_id.and_then(|p| self.catalog.products.get(&p)), lot.food_id) {
            (Some(product), _) => Some(self.catalog.product_settings(product)),
            (None, Some(food)) => self.catalog.foods.get(&food).map(|f| self.catalog.food_settings(f)),
            (None, None) => None,
        }
    }

    pub fn expiry_input(&self, lot: &LotRow) -> ExpiryInput {
        ExpiryInput {
            kind: lot.expiry_kind,
            date: lot.expiry_date,
            stocked_on: lot.stocked_on,
            opened_on: lot.opened_on,
            frozen_on: lot.frozen_on,
            thawed_on: lot.thawed_on,
            checked_until: lot.checked_until,
            shelf: self.settings(lot).map(|s| s.shelf()).unwrap_or_default(),
        }
    }

    fn soon_days(&self, lot: &LotRow) -> u32 {
        self.locations.get(&lot.location_id).and_then(|l| l.soon_days).unwrap_or(self.soon_days)
    }

    pub fn view(&self, lot: &LotRow, state: LotState) -> Lot {
        let effective = self.expiry_input(lot).effective();
        let product = lot.product_id.and_then(|p| self.catalog.products.get(&p));
        let food_id = lot.food_id.or(product.and_then(|p| p.food_id));
        let food = food_id.and_then(|f| self.catalog.foods.get(&f));
        let recipe = lot.recipe_id.and_then(|r| self.recipes.get(&r));
        let name = product
            .map(|p| p.name.clone())
            .or(food.map(|f| f.name.clone()))
            .or(recipe.map(|r| format!("Restes : {r}")))
            .unwrap_or_default();
        let food_name = match product {
            Some(p) => food.map(|f| f.name.clone()).or(p.group_id.map(|g| self.catalog.group_path(g))),
            None => None,
        };
        let location = self.locations.get(&lot.location_id);
        Lot {
            id: lot.id,
            product_id: lot.product_id,
            food_id,
            recipe_id: lot.recipe_id,
            name,
            brand: product.and_then(|p| p.brand.clone()),
            food_name,
            location_id: lot.location_id,
            location_name: location.map(|l| l.name.clone()).unwrap_or_default(),
            location_kind: location.map_or(LocationKind::Pantry, |l| l.kind),
            tracking_mode: lot.tracking_mode,
            measure: lot.measure,
            quantity: state.quantity,
            presence: state.presence,
            expiry_kind: lot.expiry_kind,
            expiry_date: lot.expiry_date,
            expiry_missing: lot.expiry_missing,
            effective_expiry: effective,
            status: expiry::status(effective, self.today, self.soon_days(lot)),
            stocked_on: lot.stocked_on,
            opened_on: lot.opened_on,
            frozen_on: lot.frozen_on,
            thawed_on: lot.thawed_on,
            checked_until: lot.checked_until,
            unit_price: lot.unit_price,
            needs_review: lot.needs_review,
            depleted: lot.depleted,
            note: lot.note.clone(),
            last_inventory: state.last_inventory,
        }
    }
}

fn movement_view(m: &MovementRow, all: &[MovementRow]) -> StockMovement {
    StockMovement {
        id: m.id,
        date: m.date,
        kind: m.kind,
        amount: m.amount,
        presence: m.presence,
        shortfall: m.shortfall,
        waste_reason: m.waste_reason,
        from_location_id: m.from_location_id,
        to_location_id: m.to_location_id,
        checked_until: m.checked_until,
        reverses_id: m.reverses_id,
        reversed: all.iter().any(|x| x.reverses_id == Some(m.id)),
        batch_id: m.batch_id,
        note: m.note.clone(),
        created_at: m.created_at.clone(),
    }
}

fn state_of(lot: &LotRow, movements: &[MovementRow]) -> LotState {
    let domain: Vec<_> = movements.iter().map(MovementRow::to_domain).collect();
    journal::state(lot.tracking_mode, &domain)
}

/// Lots avec leur quantité calculée et leur état de péremption (STK-04).
pub async fn list(db: &Db, query: &LotQuery, today: Date) -> AppResult<Vec<Lot>> {
    let mut conn = db.read.acquire().await?;
    list_in(&mut conn, query, today).await
}

/// Comme `list`, dans une connexion ouverte.
pub(crate) async fn list_in(conn: &mut SqliteConnection, query: &LotQuery, today: Date) -> AppResult<Vec<Lot>> {
    let context = Context::load(&mut *conn, today).await?;
    let lots = db::stock::list(&mut *conn, query.location_id, query.food_id, query.include_depleted).await?;
    let mut movements: HashMap<LotId, Vec<MovementRow>> = HashMap::new();
    for m in db::stock::movements_of_active_lots(&mut *conn, query.include_depleted).await? {
        movements.entry(m.lot_id).or_default().push(m);
    }
    let mut views: Vec<Lot> = lots
        .iter()
        .map(|lot| context.view(lot, state_of(lot, movements.get(&lot.id).map_or(&[][..], Vec::as_slice))))
        .collect();
    sort_by_expiry(&mut views);
    Ok(views)
}

/// Tri par date limite effective, les lots sans date à la fin, puis par nom.
pub fn sort_by_expiry(lots: &mut [Lot]) {
    lots.sort_by(|a, b| {
        let date = |l: &Lot| l.effective_expiry.map(|e| e.date);
        match (date(a), date(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub async fn get(db: &Db, id: LotId, today: Date) -> AppResult<LotDetail> {
    let mut conn = db.read.acquire().await?;
    let context = Context::load(&mut conn, today).await?;
    let lot = db::stock::find(&mut *conn, id).await?.ok_or(AppError::NotFound("lot"))?;
    let movements = db::stock::movements_of_lot(&mut *conn, id).await?;
    Ok(LotDetail {
        lot: context.view(&lot, state_of(&lot, &movements)),
        movements: movements.iter().rev().map(|m| movement_view(m, &movements)).collect(),
    })
}

pub async fn has_active_lots(conn: &mut SqliteConnection, food: FoodId) -> anyhow::Result<bool> {
    Ok(!db::stock::list(&mut *conn, None, Some(food), false).await?.is_empty())
}

// ===================================================================== Écritures

/// Lot chargé avec son journal, pour une écriture.
struct Loaded {
    lot: LotRow,
    movements: Vec<MovementRow>,
}

impl Loaded {
    async fn load(conn: &mut SqliteConnection, id: LotId) -> AppResult<Self> {
        let lot = db::stock::find(&mut *conn, id).await?.ok_or(AppError::NotFound("lot"))?;
        let movements = db::stock::movements_of_lot(&mut *conn, id).await?;
        Ok(Self { lot, movements })
    }

    fn state(&self) -> LotState {
        state_of(&self.lot, &self.movements)
    }

    /// Vérifie la date d'un nouveau mouvement (R-STK-14).
    fn check_date(&self, date: Date, today: Date) -> AppResult<()> {
        Ok(journal::check_date(date, today, self.state().last_inventory)?)
    }

    /// Enregistre un mouvement sur ce lot.
    async fn record(&mut self, conn: &mut SqliteConnection, movement: MovementRow) -> anyhow::Result<()> {
        db::stock::insert_movement(&mut *conn, &movement).await?;
        self.movements.push(movement);
        Ok(())
    }

    /// Remet à jour les copies de lecture du lot (épuisé) et l'écrit.
    async fn save(&mut self, conn: &mut SqliteConnection) -> anyhow::Result<()> {
        self.lot.depleted = self.state().is_depleted();
        db::stock::update(&mut *conn, &self.lot).await
    }
}

fn movement(lot: LotId, date: Date, kind: MovementKind) -> MovementRow {
    MovementRow {
        id: MovementId::new(),
        lot_id: lot,
        date,
        kind,
        amount: None,
        presence: None,
        shortfall: None,
        waste_reason: None,
        from_location_id: None,
        to_location_id: None,
        checked_until: None,
        reverses_id: None,
        batch_id: None,
        note: None,
        created_at: timestamp(),
    }
}

/// Origine d'un lot saisi à la main.
#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LotOrigin {
    /// Acheté (STK-02).
    #[default]
    Purchase,
    /// Trouvé lors d'un inventaire (R-STK-41).
    Adjustment,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct LotInput {
    pub product_id: Option<ProductId>,
    pub food_id: Option<FoodId>,
    pub location_id: LocationId,
    /// Date d'entrée en stock ; aujourd'hui par défaut.
    pub date: Option<Date>,
    /// Quantité par lot, dans la mesure de stock (lot en `QUANTITY`).
    pub amount: Option<Amount>,
    /// État initial (lot en `PRESENCE`) ; disponible par défaut.
    pub presence: Option<PresenceState>,
    pub expiry_kind: ExpiryKind,
    pub expiry_date: Option<Date>,
    /// Date de péremption à saisir plus tard (R-COU-51).
    #[serde(default)]
    pub expiry_missing: bool,
    /// Prix payé pour un lot.
    pub unit_price: Option<Money>,
    /// Nombre de lots identiques à créer (pack de yaourts, R-STK-6) ; 1 par défaut.
    pub count: Option<u32>,
    #[serde(default)]
    pub origin: LotOrigin,
    pub note: Option<String>,
}

/// Ajoute un ou plusieurs lots (STK-02, R-STK-1, R-STK-6).
pub async fn create(db: &Db, input: &LotInput, today: Date) -> AppResult<Vec<LotId>> {
    let mut tx = db.write.begin().await?;
    let context = Context::load(&mut tx, today).await?;
    let ids = insert_lots(&mut tx, &context, input, BatchId::new()).await?;
    tx.commit().await?;
    Ok(ids)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LotBatchInput {
    pub lines: Vec<LotInput>,
}

/// Ajoute les lots d'une mise en stock par scan en un seul lot d'opérations, annulable d'un geste
/// (STK-12, R-STK-54).
pub async fn create_batch(db: &Db, input: &LotBatchInput, today: Date) -> AppResult<(BatchId, Vec<LotId>)> {
    let batch = BatchId::new();
    let mut tx = db.write.begin().await?;
    let context = Context::load(&mut tx, today).await?;
    let mut ids = Vec::new();
    for line in &input.lines {
        ids.extend(insert_lots(&mut tx, &context, line, batch).await?);
    }
    tx.commit().await?;
    Ok((batch, ids))
}

/// Crée les lots d'une saisie et leur mouvement d'entrée, dans le lot d'opérations donné.
async fn insert_lots(
    conn: &mut SqliteConnection,
    context: &Context,
    input: &LotInput,
    batch: BatchId,
) -> AppResult<Vec<LotId>> {
    let today = context.today;
    let date = input.date.unwrap_or(today);
    if date > today {
        return Err(DomainError::MovementInFuture.into());
    }
    if !context.locations.contains_key(&input.location_id) {
        return Err(AppError::NotFound("emplacement"));
    }
    let settings = match (input.product_id, input.food_id) {
        (Some(product), None) => {
            let row = context.catalog.products.get(&product).ok_or(AppError::NotFound("produit"))?;
            context.catalog.product_settings(row)
        }
        (None, Some(food)) => {
            let row = context.catalog.foods.get(&food).ok_or(AppError::NotFound("aliment"))?;
            context.catalog.food_settings(row)
        }
        _ => return Err(DomainError::LotContent.into()),
    };
    if input.expiry_kind != ExpiryKind::None && input.expiry_date.is_none() && !input.expiry_missing {
        return Err(DomainError::ExpiryDateRequired.into());
    }
    let (amount, presence) = match settings.tracking_mode {
        TrackingMode::Quantity => {
            let amount = input.amount.filter(|a| a.is_positive()).ok_or(DomainError::NotPositive)?;
            (Some(amount), None)
        }
        TrackingMode::Presence => (None, Some(input.presence.unwrap_or(PresenceState::Available))),
    };
    let location_kind = context.locations.get(&input.location_id).map(|l| l.kind);
    let count = input.count.unwrap_or(1).clamp(1, 100);
    let mut ids = Vec::new();
    for _ in 0..count {
        let lot = LotRow {
            id: LotId::new(),
            product_id: input.product_id,
            food_id: input.food_id,
            recipe_id: None,
            location_id: input.location_id,
            tracking_mode: settings.tracking_mode,
            measure: settings.stock_measure,
            expiry_kind: input.expiry_kind,
            expiry_date: input.expiry_date.filter(|_| input.expiry_kind != ExpiryKind::None),
            expiry_missing: input.expiry_missing
                && input.expiry_date.is_none()
                && input.expiry_kind != ExpiryKind::None,
            stocked_on: date,
            opened_on: None,
            // Un lot rangé directement au congélateur est congelé dès son entrée (R-STK-32).
            frozen_on: (location_kind == Some(LocationKind::Freezer)).then_some(date),
            thawed_on: None,
            checked_until: None,
            unit_price: input.unit_price,
            needs_review: false,
            depleted: false,
            note: clean(input.note.as_deref()),
        };
        db::stock::insert(&mut *conn, &lot).await?;
        let kind = match input.origin {
            LotOrigin::Purchase => MovementKind::Purchase,
            LotOrigin::Adjustment => MovementKind::Adjustment,
        };
        let entry = MovementRow { amount, presence, batch_id: Some(batch), ..movement(lot.id, date, kind) };
        db::stock::insert_movement(&mut *conn, &entry).await?;
        ids.push(lot.id);
    }
    if let Some(food) = input.food_id {
        db::foods::unhide(&mut *conn, food).await?;
    }
    Ok(ids)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LotUpdate {
    pub expiry_kind: ExpiryKind,
    pub expiry_date: Option<Date>,
    #[serde(default)]
    pub expiry_missing: bool,
    pub unit_price: Option<Money>,
    pub note: Option<String>,
}

/// Corrige la péremption, le prix ou la note d'un lot (pas sa quantité, qui passe par un mouvement).
pub async fn update(db: &Db, id: LotId, input: &LotUpdate) -> AppResult<()> {
    if input.expiry_kind != ExpiryKind::None && input.expiry_date.is_none() && !input.expiry_missing {
        return Err(DomainError::ExpiryDateRequired.into());
    }
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.lot.expiry_kind = input.expiry_kind;
    loaded.lot.expiry_date = input.expiry_date.filter(|_| input.expiry_kind != ExpiryKind::None);
    loaded.lot.expiry_missing = input.expiry_kind != ExpiryKind::None && loaded.lot.expiry_date.is_none();
    loaded.lot.unit_price = input.unit_price;
    loaded.lot.note = clean(input.note.as_deref());
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConsumeInput {
    pub date: Option<Date>,
    /// Quantité sortie (lot en `QUANTITY`).
    pub amount: Option<Amount>,
    /// Nouvel état (lot en `PRESENCE`) : cuisiner ne le change pas d'office (R-STK-22).
    pub presence: Option<PresenceState>,
    pub note: Option<String>,
}

/// Sortie d'un lot consommé (R-STK-3 : bornée à la quantité, l'écart est signalé).
pub async fn consume(db: &Db, id: LotId, input: &ConsumeInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    let mut row = movement(id, date, MovementKind::Cooked);
    row.note = clean(input.note.as_deref());
    match loaded.lot.tracking_mode {
        TrackingMode::Quantity => {
            let requested = input.amount.filter(|a| a.is_positive()).ok_or(DomainError::NotPositive)?;
            let available = loaded.state().quantity.unwrap_or_default();
            let (taken, shortfall) = journal::bounded_withdrawal(available, requested);
            row.amount = Some(-taken);
            if shortfall.is_positive() {
                row.shortfall = Some(shortfall);
                loaded.lot.needs_review = true;
            }
        }
        TrackingMode::Presence => row.presence = input.presence,
    }
    loaded.record(&mut tx, row).await?;
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct WasteInput {
    pub date: Option<Date>,
    /// Quantité jetée ; tout le lot si absente.
    pub amount: Option<Amount>,
    pub reason: WasteReason,
    pub note: Option<String>,
}

/// Produit jeté, avec son motif (R-STK-13).
pub async fn waste(db: &Db, id: LotId, input: &WasteInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    let mut row = movement(id, date, MovementKind::Waste);
    row.waste_reason = Some(input.reason);
    row.note = clean(input.note.as_deref());
    match loaded.lot.tracking_mode {
        TrackingMode::Quantity => {
            let available = loaded.state().quantity.unwrap_or_default();
            let requested = input.amount.unwrap_or(available);
            if !requested.is_positive() {
                return Err(DomainError::NotPositive.into());
            }
            let (taken, shortfall) = journal::bounded_withdrawal(available, requested);
            row.amount = Some(-taken);
            row.shortfall = shortfall.is_positive().then_some(shortfall);
        }
        TrackingMode::Presence => row.presence = Some(PresenceState::Out),
    }
    loaded.record(&mut tx, row).await?;
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdjustInput {
    pub date: Option<Date>,
    /// Écart signé (lot en `QUANTITY`).
    pub amount: Option<Amount>,
    /// Nouvel état (lot en `PRESENCE`).
    pub presence: Option<PresenceState>,
    pub note: Option<String>,
}

/// Correction manuelle (R-STK-10) : la quantité qui en résulte ne peut pas être négative.
pub async fn adjust(db: &Db, id: LotId, input: &AdjustInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    let mut row = movement(id, date, MovementKind::Adjustment);
    row.note = clean(input.note.as_deref());
    match loaded.lot.tracking_mode {
        TrackingMode::Quantity => {
            let delta = input.amount.filter(|a| !a.is_zero()).ok_or(DomainError::TrackingModeMismatch)?;
            if (loaded.state().quantity.unwrap_or_default() + delta).is_negative() {
                return Err(DomainError::NegativeQuantity.into());
            }
            row.amount = Some(delta);
        }
        TrackingMode::Presence => row.presence = Some(input.presence.ok_or(DomainError::TrackingModeMismatch)?),
    }
    loaded.record(&mut tx, row).await?;
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DateInput {
    pub date: Option<Date>,
}

/// Ouverture d'un lot : elle raccourcit sa conservation (R-STK-31).
pub async fn open(db: &Db, id: LotId, input: &DateInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    if loaded.lot.depleted {
        return Err(DomainError::LotDepleted.into());
    }
    if loaded.lot.opened_on.is_some() {
        return Err(AppError::invalid("ALREADY_OPENED", "ce lot est déjà ouvert"));
    }
    loaded.record(&mut tx, movement(id, date, MovementKind::Opened)).await?;
    loaded.lot.opened_on = Some(date);
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CheckInput {
    pub date: Option<Date>,
    /// Nouvelle date limite ; le lendemain par défaut (R-STK-37).
    pub until: Option<Date>,
}

/// « Encore bon » : l'utilisateur garde un lot dont la date est dépassée (R-STK-37).
pub async fn check(db: &Db, id: LotId, input: &CheckInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let until = match input.until {
        Some(until) => until,
        None => date.succ_opt().ok_or(DomainError::CheckedUntilBeforeDate)?,
    };
    expiry::check_checked_until(date, until)?;
    let mut tx = db.write.begin().await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    if loaded.lot.depleted {
        return Err(DomainError::LotDepleted.into());
    }
    loaded
        .record(&mut tx, MovementRow { checked_until: Some(until), ..movement(id, date, MovementKind::Checked) })
        .await?;
    loaded.lot.checked_until = Some(until);
    loaded.save(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveInput {
    pub date: Option<Date>,
    pub to_location_id: LocationId,
    /// Quantité déplacée ; tout le lot si absente (R-STK-15).
    pub amount: Option<Amount>,
}

/// Change les dates de congélation d'un lot qui passe d'un emplacement à un autre (R-STK-32, R-STK-33).
fn apply_freezing(context: &Context, lot: &mut LotRow, to: LocationKind, date: Date) -> AppResult<()> {
    let from = context.locations.get(&lot.location_id).map_or(LocationKind::Pantry, |l| l.kind);
    match (from == LocationKind::Freezer, to == LocationKind::Freezer) {
        (false, true) => {
            context.expiry_input(lot).check_freeze(date)?;
            lot.frozen_on = Some(date);
        }
        (true, false) => lot.thawed_on = Some(date),
        _ => {}
    }
    Ok(())
}

/// Déplace tout ou partie d'un lot (R-STK-15) ; un passage au congélateur le congèle, une sortie le décongèle.
pub async fn move_lot(db: &Db, id: LotId, input: &MoveInput, today: Date) -> AppResult<Option<LotId>> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let context = Context::load(&mut tx, today).await?;
    let mut loaded = Loaded::load(&mut tx, id).await?;
    loaded.check_date(date, today)?;
    if loaded.lot.depleted {
        return Err(DomainError::LotDepleted.into());
    }
    let to = context.locations.get(&input.to_location_id).ok_or(AppError::NotFound("emplacement"))?.kind;
    if input.to_location_id == loaded.lot.location_id {
        return Err(DomainError::SameLocation.into());
    }
    let from_location = loaded.lot.location_id;
    let available = loaded.state().quantity;
    let partial = match (input.amount, available) {
        (Some(amount), Some(available)) if amount < available => {
            if !amount.is_positive() {
                return Err(DomainError::NotPositive.into());
            }
            Some(amount)
        }
        (Some(amount), Some(available)) if amount > available => {
            return Err(DomainError::MoveExceedsLot(available).into());
        }
        _ => None,
    };
    let created = match partial {
        None => {
            apply_freezing(&context, &mut loaded.lot, to, date)?;
            let row = MovementRow {
                from_location_id: Some(from_location),
                to_location_id: Some(input.to_location_id),
                ..movement(id, date, MovementKind::Move)
            };
            loaded.record(&mut tx, row).await?;
            loaded.lot.location_id = input.to_location_id;
            loaded.save(&mut tx).await?;
            None
        }
        Some(amount) => {
            // Le nouveau lot hérite de la péremption et de l'état du lot d'origine.
            let mut part = LotRow { id: LotId::new(), needs_review: false, depleted: false, ..loaded.lot.clone() };
            apply_freezing(&context, &mut part, to, date)?;
            part.location_id = input.to_location_id;
            db::stock::insert(&mut *tx, &part).await?;
            let batch = BatchId::new();
            let out = MovementRow {
                amount: Some(-amount),
                from_location_id: Some(from_location),
                to_location_id: Some(input.to_location_id),
                batch_id: Some(batch),
                ..movement(id, date, MovementKind::Move)
            };
            loaded.record(&mut tx, out).await?;
            let into = MovementRow {
                amount: Some(amount),
                from_location_id: Some(from_location),
                to_location_id: Some(input.to_location_id),
                batch_id: Some(batch),
                ..movement(part.id, date, MovementKind::Move)
            };
            db::stock::insert_movement(&mut *tx, &into).await?;
            loaded.save(&mut tx).await?;
            Some(part.id)
        }
    };
    tx.commit().await?;
    Ok(created)
}

/// Annule un mouvement, ou tout son lot d'opérations s'il en fait partie (R-STK-11, R-STK-12).
pub async fn reverse(db: &Db, movement_id: MovementId, today: Date) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let target = db::stock::find_movement(&mut *tx, movement_id).await?.ok_or(AppError::NotFound("mouvement"))?;
    let targets = match target.batch_id {
        Some(batch) => db::stock::movements_of_batch(&mut *tx, batch).await?,
        None => vec![target],
    };
    reverse_movements(&mut tx, targets, today).await?;
    tx.commit().await?;
    Ok(())
}

async fn reverse_movements(conn: &mut SqliteConnection, targets: Vec<MovementRow>, today: Date) -> AppResult<()> {
    let tx = conn;
    let context = Context::load(&mut *tx, today).await?;
    for original in targets.iter().filter(|m| m.reverses_id.is_none()) {
        let mut loaded = Loaded::load(&mut *tx, original.lot_id).await?;
        let domain: Vec<_> = loaded.movements.iter().map(MovementRow::to_domain).collect();
        let amount = journal::reversal_of(&original.to_domain(), &domain)?;
        // L'annulation est datée comme le mouvement annulé : un inventaire postérieur l'a déjà constaté, on
        // ne revient plus dessus (R-STK-14).
        let reversed = |m: &MovementRow| loaded.movements.iter().any(|x| x.reverses_id == Some(m.id));
        let later_inventory = loaded.movements.iter().find(|m| {
            m.kind == MovementKind::Inventory
                && m.reverses_id.is_none()
                && !reversed(m)
                && (m.date, m.id) > (original.date, original.id)
        });
        if let Some(inventory) = later_inventory {
            return Err(DomainError::MovementBeforeInventory(inventory.date).into());
        }
        let row = MovementRow {
            amount,
            reverses_id: Some(original.id),
            from_location_id: original.to_location_id,
            to_location_id: original.from_location_id,
            batch_id: original.batch_id,
            ..movement(original.lot_id, original.date, original.kind)
        };
        loaded.record(&mut *tx, row).await?;
        // Copies de lecture touchées par le mouvement annulé.
        match original.kind {
            MovementKind::Opened => loaded.lot.opened_on = None,
            MovementKind::Checked => {
                loaded.lot.checked_until = loaded
                    .movements
                    .iter()
                    .filter(|m| m.kind == MovementKind::Checked && m.reverses_id.is_none() && m.id != original.id)
                    .filter(|m| !loaded.movements.iter().any(|x| x.reverses_id == Some(m.id)))
                    .filter_map(|m| m.checked_until)
                    .max();
            }
            MovementKind::Move if original.amount.is_none() => {
                let back = original.from_location_id.unwrap_or(loaded.lot.location_id);
                let to = context.locations.get(&original.to_location_id.unwrap_or(back)).map(|l| l.kind);
                let from = context.locations.get(&back).map(|l| l.kind);
                if to == Some(LocationKind::Freezer) && from != Some(LocationKind::Freezer) {
                    loaded.lot.frozen_on = None;
                }
                if from == Some(LocationKind::Freezer) && to != Some(LocationKind::Freezer) {
                    loaded.lot.thawed_on = None;
                }
                loaded.lot.location_id = back;
            }
            _ => {}
        }
        loaded.save(&mut *tx).await?;
    }
    Ok(())
}

// ===================================================================== Pour la validation des repas

/// Lots non épuisés des aliments donnés (produits rattachés compris) ou des recettes données, avec leur
/// quantité et leur état de péremption : les candidats d'une sortie de repas (R-STK-36).
pub(crate) async fn available_lots(
    conn: &mut SqliteConnection,
    today: Date,
    foods: &std::collections::HashSet<FoodId>,
    recipes: &std::collections::HashSet<RecipeId>,
) -> AppResult<Vec<Lot>> {
    let context = Context::load(&mut *conn, today).await?;
    let lots = db::stock::list(&mut *conn, None, None, false).await?;
    let mut movements: HashMap<LotId, Vec<MovementRow>> = HashMap::new();
    for m in db::stock::movements_of_active_lots(&mut *conn, false).await? {
        movements.entry(m.lot_id).or_default().push(m);
    }
    let mut views: Vec<Lot> = lots
        .iter()
        .map(|lot| context.view(lot, state_of(lot, movements.get(&lot.id).map_or(&[][..], Vec::as_slice))))
        .filter(|l| l.food_id.is_some_and(|f| foods.contains(&f)) || l.recipe_id.is_some_and(|r| recipes.contains(&r)))
        .collect();
    sort_by_expiry(&mut views);
    Ok(views)
}

/// Un lot, avec sa quantité et son état, dans une transaction ouverte.
pub(crate) async fn get_in(conn: &mut SqliteConnection, id: LotId, today: Date) -> AppResult<Lot> {
    let context = Context::load(&mut *conn, today).await?;
    let lot = db::stock::find(&mut *conn, id).await?.ok_or(AppError::NotFound("lot"))?;
    let movements = db::stock::movements_of_lot(&mut *conn, id).await?;
    Ok(context.view(&lot, state_of(&lot, &movements)))
}

/// Sortie d'un lot pour un repas validé (R-PLA-30) : datée, bornée à la quantité (R-STK-3), rattachée au lot
/// d'opérations de la validation (R-STK-12). Renvoie le mouvement et la quantité réellement sortie.
pub(crate) async fn withdraw(
    conn: &mut SqliteConnection,
    lot: LotId,
    amount: Amount,
    date: Date,
    today: Date,
    batch: BatchId,
) -> AppResult<(MovementId, Amount)> {
    let mut loaded = Loaded::load(&mut *conn, lot).await?;
    loaded.check_date(date, today)?;
    let mut row = MovementRow { batch_id: Some(batch), ..movement(lot, date, MovementKind::Cooked) };
    let taken = match loaded.lot.tracking_mode {
        TrackingMode::Quantity => {
            let available = loaded.state().quantity.unwrap_or_default();
            let (taken, shortfall) = journal::bounded_withdrawal(available, amount);
            row.amount = Some(-taken);
            if shortfall.is_positive() {
                row.shortfall = Some(shortfall);
                loaded.lot.needs_review = true;
            }
            taken
        }
        // Un lot suivi en présence ne se compte pas : la sortie l'indique sans quantité.
        TrackingMode::Presence => {
            row.presence = Some(PresenceState::Available);
            Amount::ZERO
        }
    };
    let id = row.id;
    loaded.record(&mut *conn, row).await?;
    loaded.save(&mut *conn).await?;
    Ok((id, taken))
}

/// Crée un lot de restes ou de préparation (STK-08, ADR-0012), avec son mouvement d'entrée dans le lot
/// d'opérations de la validation.
pub(crate) async fn create_leftover(
    conn: &mut SqliteConnection,
    lot: &LotRow,
    amount: Amount,
    kind: MovementKind,
    batch: BatchId,
) -> AppResult<()> {
    if !amount.is_positive() {
        return Err(DomainError::NotPositive.into());
    }
    db::stock::insert(&mut *conn, lot).await?;
    let entry = MovementRow { amount: Some(amount), batch_id: Some(batch), ..movement(lot.id, lot.stocked_on, kind) };
    db::stock::insert_movement(&mut *conn, &entry).await?;
    Ok(())
}

/// Annule tout un lot d'opérations : une mise en stock par scan, un inventaire (R-STK-12, R-STK-54).
pub async fn reverse_whole_batch(db: &Db, batch: BatchId, today: Date) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let movements = db::stock::movements_of_batch(&mut *tx, batch).await?;
    if movements.is_empty() {
        return Err(AppError::NotFound("lot d'opérations"));
    }
    reverse_movements(&mut tx, movements, today).await?;
    tx.commit().await?;
    Ok(())
}

/// Annule un lot d'opérations dans une transaction ouverte (R-STK-12) : voir `reverse`.
pub(crate) async fn reverse_batch(conn: &mut SqliteConnection, batch: BatchId, today: Date) -> AppResult<()> {
    let movements = db::stock::movements_of_batch(&mut *conn, batch).await?;
    if movements.is_empty() {
        return Ok(());
    }
    reverse_movements(conn, movements, today).await
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InventoryEntry {
    pub lot_id: LotId,
    /// Quantité constatée (lot en `QUANTITY`) ; 0 pour un lot absent.
    pub amount: Option<Amount>,
    /// État constaté (lot en `PRESENCE`).
    pub presence: Option<PresenceState>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InventoryInput {
    pub date: Option<Date>,
    pub entries: Vec<InventoryEntry>,
    /// Lots trouvés (R-STK-41), créés dans le même lot d'opérations (R-STK-55) ; leur origine est
    /// forcée à `ADJUSTMENT` et leur date à celle de l'inventaire.
    #[serde(default)]
    pub found: Vec<LotInput>,
}

/// Inventaire : constate la quantité réelle de chaque lot et conserve l'écart (R-STK-40, R-STK-41).
pub async fn inventory(db: &Db, input: &InventoryInput, today: Date) -> AppResult<BatchId> {
    let date = input.date.unwrap_or(today);
    let batch = BatchId::new();
    let mut tx = db.write.begin().await?;
    for entry in &input.entries {
        let mut loaded = Loaded::load(&mut tx, entry.lot_id).await?;
        loaded.check_date(date, today)?;
        let mut row = MovementRow { batch_id: Some(batch), ..movement(entry.lot_id, date, MovementKind::Inventory) };
        match loaded.lot.tracking_mode {
            TrackingMode::Quantity => {
                let observed = entry.amount.ok_or(DomainError::TrackingModeMismatch)?;
                if observed.is_negative() {
                    return Err(DomainError::NegativeQuantity.into());
                }
                let computed = loaded.state().quantity.unwrap_or_default();
                row.amount = Some(observed);
                row.shortfall = Some(observed - computed);
            }
            TrackingMode::Presence => row.presence = Some(entry.presence.ok_or(DomainError::TrackingModeMismatch)?),
        }
        loaded.record(&mut tx, row).await?;
        loaded.lot.needs_review = false;
        loaded.save(&mut tx).await?;
    }
    if !input.found.is_empty() {
        let context = Context::load(&mut tx, today).await?;
        for found in &input.found {
            let found = LotInput { date: Some(date), origin: LotOrigin::Adjustment, ..found.clone() };
            insert_lots(&mut tx, &context, &found, batch).await?;
        }
    }
    tx.commit().await?;
    Ok(batch)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TrackingModeInput {
    pub mode: TrackingMode,
    /// Pour passer en `QUANTITY` : la quantité constatée de chaque lot non épuisé (R-STK-21).
    #[serde(default)]
    pub inventory: Vec<InventoryEntry>,
    pub date: Option<Date>,
}

/// Change le mode de suivi d'un aliment et convertit ses lots (R-STK-21).
pub async fn change_tracking_mode(db: &Db, food: FoodId, input: &TrackingModeInput, today: Date) -> AppResult<()> {
    let date = input.date.unwrap_or(today);
    let mut tx = db.write.begin().await?;
    let mut row = db::foods::find(&mut *tx, food).await?.ok_or(AppError::NotFound("aliment"))?;
    row.settings.tracking_mode = Some(input.mode);
    let lots = db::stock::list(&mut *tx, None, Some(food), false).await?;
    let batch = BatchId::new();
    for lot in lots.iter().filter(|l| l.tracking_mode != input.mode) {
        let mut loaded = Loaded::load(&mut tx, lot.id).await?;
        loaded.check_date(date, today)?;
        let mut inventory = MovementRow { batch_id: Some(batch), ..movement(lot.id, date, MovementKind::Inventory) };
        match input.mode {
            TrackingMode::Presence => inventory.presence = Some(PresenceState::Available),
            TrackingMode::Quantity => {
                let observed = input.inventory.iter().find(|e| e.lot_id == lot.id).and_then(|e| e.amount);
                let observed = observed.ok_or_else(|| {
                    AppError::invalid_field(
                        "INVENTORY_REQUIRED",
                        "inventory",
                        "passer en quantité demande la quantité de chaque lot en stock (R-STK-21)",
                    )
                })?;
                if !observed.is_positive() {
                    return Err(DomainError::NotPositive.into());
                }
                inventory.amount = Some(observed);
            }
        }
        loaded.lot.tracking_mode = input.mode;
        loaded.record(&mut tx, inventory).await?;
        loaded.save(&mut tx).await?;
    }
    let fields = db::foods::FoodFields {
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
    };
    db::foods::update(&mut *tx, food, &fields).await?;
    tx.commit().await?;
    Ok(())
}

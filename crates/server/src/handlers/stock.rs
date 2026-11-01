use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::{BatchId, LotId, MovementId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::stock::{
    self, AdjustInput, CheckInput, ConsumeInput, DateInput, InventoryInput, Lot, LotBatchInput, LotDetail, LotInput,
    LotQuery, LotUpdate, MoveInput, WasteInput,
};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedLots {
    pub ids: Vec<LotId>,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedLotBatch {
    /// Lot d'opérations de la mise en stock, pour l'annuler d'un geste (R-STK-54).
    pub batch_id: BatchId,
    pub ids: Vec<LotId>,
}

#[derive(Serialize, ToSchema)]
pub struct MoveResult {
    /// Nouveau lot créé par un déplacement partiel (R-STK-15).
    pub created_lot_id: Option<LotId>,
}

#[derive(Serialize, ToSchema)]
pub struct InventoryResult {
    pub batch_id: BatchId,
}

/// Lots avec leur quantité calculée et leur état de péremption (STK-04).
#[utoipa::path(get, operation_id = "listLots", path = "/lots", tag = "stock", params(LotQuery), responses((status = 200, body = Vec<Lot>)))]
pub async fn list(State(state): State<AppState>, Query(query): Query<LotQuery>) -> AppResult<Json<Vec<Lot>>> {
    Ok(Json(stock::list(&state.db, &query, state.clock.today()).await?))
}

/// Ajoute un ou plusieurs lots (STK-02, R-STK-6).
#[utoipa::path(
    post, operation_id = "createLots", path = "/lots", tag = "stock", request_body = LotInput,
    responses((status = 201, body = CreatedLots), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<LotInput>,
) -> AppResult<(StatusCode, Json<CreatedLots>)> {
    let ids = stock::create(&state.db, &input, state.clock.today()).await?;
    Ok((StatusCode::CREATED, Json(CreatedLots { ids })))
}

/// Mise en stock par scan : tous les lots en un seul lot d'opérations (STK-12, R-STK-54).
#[utoipa::path(
    post, operation_id = "createLotBatch", path = "/lot-batches", tag = "stock", request_body = LotBatchInput,
    responses((status = 201, body = CreatedLotBatch), (status = 422, body = Problem))
)]
pub async fn create_batch(
    State(state): State<AppState>,
    Json(input): Json<LotBatchInput>,
) -> AppResult<(StatusCode, Json<CreatedLotBatch>)> {
    let (batch_id, ids) = stock::create_batch(&state.db, &input, state.clock.today()).await?;
    Ok((StatusCode::CREATED, Json(CreatedLotBatch { batch_id, ids })))
}

/// Annule tout un lot d'opérations : mise en stock par scan, inventaire (R-STK-12).
#[utoipa::path(
    post, operation_id = "reverseBatch", path = "/batches/{id}/reverse", tag = "stock", params(("id" = BatchId, Path)),
    responses((status = 204), (status = 404, body = Problem), (status = 422, body = Problem))
)]
pub async fn reverse_batch(State(state): State<AppState>, Path(id): Path<BatchId>) -> AppResult<StatusCode> {
    stock::reverse_whole_batch(&state.db, id, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Un lot et son journal, du plus récent au plus ancien (STK-03).
#[utoipa::path(get, operation_id = "getLot", path = "/lots/{id}", tag = "stock", params(("id" = LotId, Path)), responses((status = 200, body = LotDetail), (status = 404, body = Problem)))]
pub async fn get(State(state): State<AppState>, Path(id): Path<LotId>) -> AppResult<Json<LotDetail>> {
    Ok(Json(stock::get(&state.db, id, state.clock.today()).await?))
}

/// Corrige la péremption, le prix ou la note d'un lot.
#[utoipa::path(
    put, operation_id = "updateLot", path = "/lots/{id}", tag = "stock", params(("id" = LotId, Path)), request_body = LotUpdate,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<LotUpdate>,
) -> AppResult<StatusCode> {
    stock::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Sortie d'un lot consommé (R-STK-3).
#[utoipa::path(
    post, operation_id = "consumeLot", path = "/lots/{id}/consume", tag = "stock", params(("id" = LotId, Path)),
    request_body = ConsumeInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn consume(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<ConsumeInput>,
) -> AppResult<StatusCode> {
    stock::consume(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Produit jeté, avec son motif (R-STK-13).
#[utoipa::path(
    post, operation_id = "wasteLot", path = "/lots/{id}/waste", tag = "stock", params(("id" = LotId, Path)),
    request_body = WasteInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn waste(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<WasteInput>,
) -> AppResult<StatusCode> {
    stock::waste(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Correction manuelle de la quantité ou de l'état.
#[utoipa::path(
    post, operation_id = "adjustLot", path = "/lots/{id}/adjust", tag = "stock", params(("id" = LotId, Path)),
    request_body = AdjustInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn adjust(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<AdjustInput>,
) -> AppResult<StatusCode> {
    stock::adjust(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ouverture d'un lot (R-STK-31).
#[utoipa::path(
    post, operation_id = "openLot", path = "/lots/{id}/open", tag = "stock", params(("id" = LotId, Path)),
    request_body = DateInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn open(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<DateInput>,
) -> AppResult<StatusCode> {
    stock::open(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// « Encore bon » : garde un lot à date dépassée jusqu'à une nouvelle date limite (R-STK-37).
#[utoipa::path(
    post, operation_id = "checkLot", path = "/lots/{id}/check", tag = "stock", params(("id" = LotId, Path)),
    request_body = CheckInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn check(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<CheckInput>,
) -> AppResult<StatusCode> {
    stock::check(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Déplace tout ou partie d'un lot ; congèle ou décongèle selon l'emplacement (R-STK-15, R-STK-32, R-STK-33).
#[utoipa::path(
    post, operation_id = "moveLot", path = "/lots/{id}/move", tag = "stock", params(("id" = LotId, Path)),
    request_body = MoveInput, responses((status = 200, body = MoveResult), (status = 422, body = Problem))
)]
pub async fn move_lot(
    State(state): State<AppState>,
    Path(id): Path<LotId>,
    Json(input): Json<MoveInput>,
) -> AppResult<Json<MoveResult>> {
    let created_lot_id = stock::move_lot(&state.db, id, &input, state.clock.today()).await?;
    Ok(Json(MoveResult { created_lot_id }))
}

/// Annule un mouvement, ou tout son lot d'opérations (R-STK-11, R-STK-12).
#[utoipa::path(
    post, operation_id = "reverseMovement", path = "/movements/{id}/reverse", tag = "stock", params(("id" = MovementId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn reverse(State(state): State<AppState>, Path(id): Path<MovementId>) -> AppResult<StatusCode> {
    stock::reverse(&state.db, id, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Inventaire : quantité constatée de chaque lot, écarts conservés (R-STK-40).
#[utoipa::path(
    post, operation_id = "recordInventory", path = "/inventories", tag = "stock", request_body = InventoryInput,
    responses((status = 201, body = InventoryResult), (status = 422, body = Problem))
)]
pub async fn inventory(
    State(state): State<AppState>,
    Json(input): Json<InventoryInput>,
) -> AppResult<(StatusCode, Json<InventoryResult>)> {
    let batch_id = stock::inventory(&state.db, &input, state.clock.today()).await?;
    Ok((StatusCode::CREATED, Json(InventoryResult { batch_id })))
}

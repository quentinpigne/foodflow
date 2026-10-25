use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::{FoodId, UnitId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::foods::{self, FoodDetail, FoodInput, FoodQuery, FoodSummary, NutrientsInput};
use crate::services::stock::{self, TrackingModeInput};
use crate::services::units::UnitInput;
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedFood {
    pub id: FoodId,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedUnit {
    pub id: UnitId,
}

/// Recherche d'aliments, sans accents ni casse, par nom et alias (ALI-06).
#[utoipa::path(get, operation_id = "searchFoods", path = "/foods", tag = "foods", params(FoodQuery), responses((status = 200, body = Vec<FoodSummary>)))]
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<FoodQuery>,
) -> AppResult<Json<Vec<FoodSummary>>> {
    Ok(Json(foods::search(&state.db, &query).await?))
}

#[utoipa::path(
    post, operation_id = "createFood", path = "/foods", tag = "foods", request_body = FoodInput,
    responses((status = 201, body = CreatedFood), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<FoodInput>,
) -> AppResult<(StatusCode, Json<CreatedFood>)> {
    let id = foods::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedFood { id })))
}

/// Fiche d'un aliment : réglages, valeurs nutritionnelles, unités propres, produits.
#[utoipa::path(get, operation_id = "getFood", path = "/foods/{id}", tag = "foods", params(("id" = FoodId, Path)), responses((status = 200, body = FoodDetail), (status = 404, body = Problem)))]
pub async fn get(State(state): State<AppState>, Path(id): Path<FoodId>) -> AppResult<Json<FoodDetail>> {
    Ok(Json(foods::get(&state.db, id).await?))
}

#[utoipa::path(
    put, operation_id = "updateFood", path = "/foods/{id}", tag = "foods", params(("id" = FoodId, Path)), request_body = FoodInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<FoodId>,
    Json(input): Json<FoodInput>,
) -> AppResult<StatusCode> {
    foods::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un aliment inutilisé.
#[utoipa::path(delete, operation_id = "deleteFood", path = "/foods/{id}", tag = "foods", params(("id" = FoodId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete(State(state): State<AppState>, Path(id): Path<FoodId>) -> AppResult<StatusCode> {
    foods::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Saisit des valeurs nutritionnelles ; `null` rend une valeur inconnue (R-ALI-11, R-ALI-20).
#[utoipa::path(
    put, operation_id = "setFoodNutrients", path = "/foods/{id}/nutrients", tag = "foods", params(("id" = FoodId, Path)),
    request_body = NutrientsInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn set_nutrients(
    State(state): State<AppState>,
    Path(id): Path<FoodId>,
    Json(input): Json<NutrientsInput>,
) -> AppResult<StatusCode> {
    foods::set_nutrients(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Rétablit la valeur Ciqual d'un nutriment (R-ALI-11).
#[utoipa::path(
    post, operation_id = "restoreCiqualNutrient", path = "/foods/{id}/nutrients/{code}/restore", tag = "foods",
    params(("id" = FoodId, Path), ("code" = String, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn restore_ciqual(
    State(state): State<AppState>,
    Path((id, code)): Path<(FoodId, String)>,
) -> AppResult<StatusCode> {
    foods::restore_ciqual(&state.db, id, &code).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Change le mode de suivi de l'aliment et convertit ses lots (R-STK-21).
#[utoipa::path(
    post, operation_id = "changeTrackingMode", path = "/foods/{id}/tracking-mode", tag = "foods", params(("id" = FoodId, Path)),
    request_body = TrackingModeInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn change_tracking_mode(
    State(state): State<AppState>,
    Path(id): Path<FoodId>,
    Json(input): Json<TrackingModeInput>,
) -> AppResult<StatusCode> {
    stock::change_tracking_mode(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ajoute une unité propre à l'aliment (R-ALI-32).
#[utoipa::path(
    post, operation_id = "addFoodUnit", path = "/foods/{id}/units", tag = "foods", params(("id" = FoodId, Path)),
    request_body = UnitInput, responses((status = 201, body = CreatedUnit), (status = 422, body = Problem))
)]
pub async fn add_unit(
    State(state): State<AppState>,
    Path(id): Path<FoodId>,
    Json(input): Json<UnitInput>,
) -> AppResult<(StatusCode, Json<CreatedUnit>)> {
    let id = foods::add_unit(&state.db, id, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedUnit { id })))
}

#[utoipa::path(
    put, operation_id = "updateFoodUnit", path = "/foods/{id}/units/{unit}", tag = "foods",
    params(("id" = FoodId, Path), ("unit" = UnitId, Path)), request_body = UnitInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_unit(
    State(state): State<AppState>,
    Path((id, unit)): Path<(FoodId, UnitId)>,
    Json(input): Json<UnitInput>,
) -> AppResult<StatusCode> {
    foods::update_unit(&state.db, id, unit, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete, operation_id = "deleteFoodUnit", path = "/foods/{id}/units/{unit}", tag = "foods",
    params(("id" = FoodId, Path), ("unit" = UnitId, Path)), responses((status = 204), (status = 404, body = Problem))
)]
pub async fn delete_unit(
    State(state): State<AppState>,
    Path((id, unit)): Path<(FoodId, UnitId)>,
) -> AppResult<StatusCode> {
    foods::delete_unit(&state.db, id, unit).await?;
    Ok(StatusCode::NO_CONTENT)
}

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::FoodGroupId;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::groups::{self, FoodGroup, FoodGroupInput, MergeGroupInput};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedGroup {
    pub id: FoodGroupId,
}

/// Toutes les familles, avec leurs réglages propres et effectifs (ALI-01).
#[utoipa::path(get, operation_id = "listFoodGroups", path = "/food-groups", tag = "foods", responses((status = 200, body = Vec<FoodGroup>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<FoodGroup>>> {
    Ok(Json(groups::list(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createFoodGroup", path = "/food-groups", tag = "foods", request_body = FoodGroupInput,
    responses((status = 201, body = CreatedGroup), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<FoodGroupInput>,
) -> AppResult<(StatusCode, Json<CreatedGroup>)> {
    let id = groups::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedGroup { id })))
}

/// Renomme, déplace ou règle une famille (R-ALI-1, R-ALI-2, R-ALI-4).
#[utoipa::path(
    put, operation_id = "updateFoodGroup", path = "/food-groups/{id}", tag = "foods", params(("id" = FoodGroupId, Path)),
    request_body = FoodGroupInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<FoodGroupId>,
    Json(input): Json<FoodGroupInput>,
) -> AppResult<StatusCode> {
    groups::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime une famille vide (R-ALI-3).
#[utoipa::path(
    delete, operation_id = "deleteFoodGroup", path = "/food-groups/{id}", tag = "foods", params(("id" = FoodGroupId, Path)),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn delete(State(state): State<AppState>, Path(id): Path<FoodGroupId>) -> AppResult<StatusCode> {
    groups::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fusionne la famille dans une autre (R-ALI-3).
#[utoipa::path(
    post, operation_id = "mergeFoodGroup", path = "/food-groups/{id}/merge", tag = "foods", params(("id" = FoodGroupId, Path)),
    request_body = MergeGroupInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn merge(
    State(state): State<AppState>,
    Path(id): Path<FoodGroupId>,
    Json(input): Json<MergeGroupInput>,
) -> AppResult<StatusCode> {
    groups::merge(&state.db, id, input.into).await?;
    Ok(StatusCode::NO_CONTENT)
}

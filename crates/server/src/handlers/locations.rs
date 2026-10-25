use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::LocationId;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::locations::{self, Location, LocationInput};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedLocation {
    pub id: LocationId,
}

/// Emplacements, dans l'ordre d'affichage (STK-01).
#[utoipa::path(get, operation_id = "listLocations", path = "/locations", tag = "stock", responses((status = 200, body = Vec<Location>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Location>>> {
    Ok(Json(locations::list(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createLocation", path = "/locations", tag = "stock", request_body = LocationInput,
    responses((status = 201, body = CreatedLocation), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<LocationInput>,
) -> AppResult<(StatusCode, Json<CreatedLocation>)> {
    let id = locations::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedLocation { id })))
}

#[utoipa::path(
    put, operation_id = "updateLocation", path = "/locations/{id}", tag = "stock", params(("id" = LocationId, Path)),
    request_body = LocationInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<LocationId>,
    Json(input): Json<LocationInput>,
) -> AppResult<StatusCode> {
    locations::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un emplacement qui n'a jamais contenu de lot.
#[utoipa::path(delete, operation_id = "deleteLocation", path = "/locations/{id}", tag = "stock", params(("id" = LocationId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete(State(state): State<AppState>, Path(id): Path<LocationId>) -> AppResult<StatusCode> {
    locations::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;

use crate::error::{AppResult, Problem};
use crate::services::settings::{self, AppSettings};
use crate::state::AppState;

/// Réglages généraux.
#[utoipa::path(get, operation_id = "getSettings", path = "/settings", tag = "system", responses((status = 200, body = AppSettings)))]
pub async fn get(State(state): State<AppState>) -> AppResult<Json<AppSettings>> {
    Ok(Json(settings::get(&state.db.read).await?))
}

#[utoipa::path(put, operation_id = "updateSettings", path = "/settings", tag = "system", request_body = AppSettings, responses((status = 204), (status = 422, body = Problem)))]
pub async fn update(State(state): State<AppState>, Json(input): Json<AppSettings>) -> AppResult<StatusCode> {
    settings::update(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

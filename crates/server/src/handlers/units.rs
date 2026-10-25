use axum::Json;
use axum::extract::State;

use crate::error::AppResult;
use crate::services::units::{self, Unit};
use crate::state::AppState;

/// Unités générales de saisie (ALI-04).
#[utoipa::path(get, operation_id = "listUnits", path = "/units", tag = "foods", responses((status = 200, body = Vec<Unit>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<Unit>>> {
    Ok(Json(units::list(&state.db, None).await?))
}

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::RecipeDraftId;

use super::recipes::CreatedRecipe;
use crate::error::{AppResult, Problem};
use crate::services::recipe_import::{self, DraftPayload, RecipeDraft, StartImport};
use crate::services::recipes::RecipeInput;
use crate::state::AppState;

/// Importe une page de recette ou un fichier JSON-LD : crée un brouillon à revoir (REC-07, R-REC-60, R-REC-61).
#[utoipa::path(
    post, operation_id = "startRecipeImport", path = "/recipe-drafts", tag = "recipes", request_body = StartImport,
    responses((status = 201, body = RecipeDraft), (status = 422, body = Problem))
)]
pub async fn start(
    State(state): State<AppState>,
    Json(input): Json<StartImport>,
) -> AppResult<(StatusCode, Json<RecipeDraft>)> {
    let draft =
        recipe_import::start(&state.db, &state.data, state.fetcher.as_ref(), state.photos.as_ref(), &input).await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

/// Brouillons en attente de revue.
#[utoipa::path(get, operation_id = "listRecipeDrafts", path = "/recipe-drafts", tag = "recipes", responses((status = 200, body = Vec<RecipeDraft>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<RecipeDraft>>> {
    Ok(Json(recipe_import::list(&state.db).await?))
}

#[utoipa::path(get, operation_id = "getRecipeDraft", path = "/recipe-drafts/{id}", tag = "recipes", params(("id" = RecipeDraftId, Path)), responses((status = 200, body = RecipeDraft), (status = 404, body = Problem)))]
pub async fn get(State(state): State<AppState>, Path(id): Path<RecipeDraftId>) -> AppResult<Json<RecipeDraft>> {
    Ok(Json(recipe_import::get(&state.db, id).await?))
}

/// Enregistre la revue en cours.
#[utoipa::path(
    put, operation_id = "saveRecipeDraft", path = "/recipe-drafts/{id}", tag = "recipes", params(("id" = RecipeDraftId, Path)),
    request_body = DraftPayload, responses((status = 204), (status = 404, body = Problem))
)]
pub async fn save(
    State(state): State<AppState>,
    Path(id): Path<RecipeDraftId>,
    Json(payload): Json<DraftPayload>,
) -> AppResult<StatusCode> {
    recipe_import::save(&state.db, id, &payload).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Valide le brouillon : crée la recette et mémorise les rattachements (R-REC-65, R-REC-66).
#[utoipa::path(
    post, operation_id = "commitRecipeDraft", path = "/recipe-drafts/{id}/commit", tag = "recipes", params(("id" = RecipeDraftId, Path)),
    request_body = RecipeInput, responses((status = 201, body = CreatedRecipe), (status = 422, body = Problem))
)]
pub async fn commit(
    State(state): State<AppState>,
    Path(id): Path<RecipeDraftId>,
    Json(input): Json<RecipeInput>,
) -> AppResult<(StatusCode, Json<CreatedRecipe>)> {
    let id = recipe_import::commit(&state.db, &state.data, id, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedRecipe { id })))
}

/// Abandonne le brouillon.
#[utoipa::path(delete, operation_id = "discardRecipeDraft", path = "/recipe-drafts/{id}", tag = "recipes", params(("id" = RecipeDraftId, Path)), responses((status = 204)))]
pub async fn discard(State(state): State<AppState>, Path(id): Path<RecipeDraftId>) -> AppResult<StatusCode> {
    recipe_import::discard(&state.db, &state.data, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

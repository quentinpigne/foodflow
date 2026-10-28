use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use foodflow_domain::shared::ids::RecipeDraftId;

use super::recipes::{CreatedRecipe, image_field};
use crate::error::{AppResult, Problem};
use crate::services::images;
use crate::services::photos::{self, ImageUrlInput};
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

/// Image d'un brouillon (R-REC-68, R-REC-73).
#[utoipa::path(
    get, operation_id = "getRecipeDraftImage", path = "/recipe-drafts/{id}/image", tag = "recipes",
    params(("id" = RecipeDraftId, Path)),
    responses((status = 200, description = "Image", content_type = "image/*"), (status = 404, body = Problem))
)]
pub async fn image(State(state): State<AppState>, Path(id): Path<RecipeDraftId>) -> AppResult<impl IntoResponse> {
    let (bytes, content_type) = images::read_draft(&state.db, &state.data, id).await?;
    Ok(([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "private, max-age=3600")], Bytes::from(bytes)))
}

/// Remplace l'image d'un brouillon (formulaire `multipart`, champ `image`).
#[utoipa::path(
    put, operation_id = "uploadRecipeDraftImage", path = "/recipe-drafts/{id}/image", tag = "recipes",
    params(("id" = RecipeDraftId, Path)),
    request_body(content_type = "multipart/form-data", description = "Champ `image` : JPEG, PNG ou WebP"),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn upload_image(
    State(state): State<AppState>,
    Path(id): Path<RecipeDraftId>,
    form: Multipart,
) -> AppResult<StatusCode> {
    let (content_type, bytes) = image_field(form).await?;
    images::upload_draft(&state.db, &state.data, id, content_type.as_deref(), &bytes).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Retient pour un brouillon l'image d'une adresse : photo proposée, ou adresse saisie (R-REC-73).
#[utoipa::path(
    put, operation_id = "chooseRecipeDraftPhoto", path = "/recipe-drafts/{id}/photo", tag = "recipes",
    params(("id" = RecipeDraftId, Path)), request_body = ImageUrlInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn choose_photo(
    State(state): State<AppState>,
    Path(id): Path<RecipeDraftId>,
    Json(input): Json<ImageUrlInput>,
) -> AppResult<StatusCode> {
    photos::choose_for_draft(&state.db, &state.data, state.fetcher.as_ref(), id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

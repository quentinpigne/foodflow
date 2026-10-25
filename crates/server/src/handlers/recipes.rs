use axum::Json;
use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use foodflow_domain::Amount;
use foodflow_domain::shared::ids::RecipeId;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::{AppError, AppResult, Problem};
use crate::services::images;
use crate::services::recipes::{self, RecipeDetail, RecipeInput, RecipeQuery, RecipeSummary};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedRecipe {
    pub id: RecipeId,
}

#[derive(Deserialize, IntoParams)]
pub struct ServingsQuery {
    /// Équivalents adulte servis, en millièmes : met la fiche à l'échelle (R-REC-12).
    pub servings: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
pub struct ArchiveInput {
    pub archived: bool,
}

/// Recherche et filtres (REC-09).
#[utoipa::path(get, operation_id = "searchRecipes", path = "/recipes", tag = "recipes", params(RecipeQuery), responses((status = 200, body = Vec<RecipeSummary>)))]
pub async fn search(
    State(state): State<AppState>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<Vec<RecipeSummary>>> {
    Ok(Json(recipes::search(&state.db, &query).await?))
}

/// Étiquettes utilisées.
#[utoipa::path(get, operation_id = "listRecipeTags", path = "/recipes/tags", tag = "recipes", responses((status = 200, body = Vec<String>)))]
pub async fn tags(State(state): State<AppState>) -> AppResult<Json<Vec<String>>> {
    Ok(Json(recipes::tags(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createRecipe", path = "/recipes", tag = "recipes", request_body = RecipeInput,
    responses((status = 201, body = CreatedRecipe), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<RecipeInput>,
) -> AppResult<(StatusCode, Json<CreatedRecipe>)> {
    let id = recipes::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedRecipe { id })))
}

/// Fiche : ingrédients mis à l'échelle, valeurs par portion, temps, portions des groupes (REC-02, REC-03).
#[utoipa::path(
    get, operation_id = "getRecipe", path = "/recipes/{id}", tag = "recipes", params(("id" = RecipeId, Path), ServingsQuery),
    responses((status = 200, body = RecipeDetail), (status = 404, body = Problem))
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<RecipeId>,
    Query(q): Query<ServingsQuery>,
) -> AppResult<Json<RecipeDetail>> {
    let servings = q.servings.filter(|s| *s > 0).map(Amount::from_raw);
    Ok(Json(recipes::get(&state.db, id, servings).await?))
}

#[utoipa::path(
    put, operation_id = "updateRecipe", path = "/recipes/{id}", tag = "recipes", params(("id" = RecipeId, Path)), request_body = RecipeInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<RecipeId>,
    Json(input): Json<RecipeInput>,
) -> AppResult<StatusCode> {
    recipes::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id = "deleteRecipe", path = "/recipes/{id}", tag = "recipes", params(("id" = RecipeId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete(State(state): State<AppState>, Path(id): Path<RecipeId>) -> AppResult<StatusCode> {
    recipes::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Archive ou désarchive (R-REC-2).
#[utoipa::path(
    put, operation_id = "archiveRecipe", path = "/recipes/{id}/archived", tag = "recipes", params(("id" = RecipeId, Path)),
    request_body = ArchiveInput, responses((status = 204), (status = 404, body = Problem))
)]
pub async fn archive(
    State(state): State<AppState>,
    Path(id): Path<RecipeId>,
    Json(input): Json<ArchiveInput>,
) -> AppResult<StatusCode> {
    recipes::set_archived(&state.db, id, input.archived).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Duplique pour faire une variante (REC-12).
#[utoipa::path(post, operation_id = "duplicateRecipe", path = "/recipes/{id}/duplicate", tag = "recipes", params(("id" = RecipeId, Path)), responses((status = 201, body = CreatedRecipe)))]
pub async fn duplicate(
    State(state): State<AppState>,
    Path(id): Path<RecipeId>,
) -> AppResult<(StatusCode, Json<CreatedRecipe>)> {
    let id = recipes::duplicate(&state.db, id).await?;
    Ok((StatusCode::CREATED, Json(CreatedRecipe { id })))
}

/// Export au format schema.org/Recipe JSON-LD (REC-11, R-REC-70).
#[utoipa::path(
    get, operation_id = "exportRecipe", path = "/recipes/{id}/json-ld", tag = "recipes", params(("id" = RecipeId, Path)),
    responses((status = 200, description = "JSON-LD schema.org/Recipe", content_type = "application/ld+json"))
)]
pub async fn json_ld(State(state): State<AppState>, Path(id): Path<RecipeId>) -> AppResult<impl IntoResponse> {
    let value = recipes::json_ld(&state.db, id).await?;
    let body = serde_json::to_vec_pretty(&value).map_err(anyhow::Error::from)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/ld+json".to_owned()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"recette-{id}.jsonld\"")),
        ],
        body,
    ))
}

/// Image de la recette (REC-10).
#[utoipa::path(
    get, operation_id = "getRecipeImage", path = "/recipes/{id}/image", tag = "recipes", params(("id" = RecipeId, Path)),
    responses((status = 200, description = "Image", content_type = "image/*"), (status = 404, body = Problem))
)]
pub async fn image(State(state): State<AppState>, Path(id): Path<RecipeId>) -> AppResult<impl IntoResponse> {
    let (bytes, content_type) = images::read(&state.db, &state.data, id).await?;
    Ok(([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "private, max-age=3600")], Bytes::from(bytes)))
}

/// Remplace l'image (formulaire `multipart`, champ `image`).
#[utoipa::path(
    put, operation_id = "uploadRecipeImage", path = "/recipes/{id}/image", tag = "recipes", params(("id" = RecipeId, Path)),
    request_body(content_type = "multipart/form-data", description = "Champ `image` : JPEG, PNG ou WebP"),
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn upload_image(
    State(state): State<AppState>,
    Path(id): Path<RecipeId>,
    mut form: Multipart,
) -> AppResult<StatusCode> {
    while let Some(field) = form.next_field().await.map_err(|e| AppError::invalid("INVALID_UPLOAD", e.to_string()))? {
        if field.name() == Some("image") {
            let content_type = field.content_type().map(str::to_owned);
            let bytes = field.bytes().await.map_err(|e| AppError::invalid("INVALID_UPLOAD", e.to_string()))?;
            images::upload(&state.db, &state.data, id, content_type.as_deref(), &bytes).await?;
            return Ok(StatusCode::NO_CONTENT);
        }
    }
    Err(AppError::invalid_field("REQUIRED", "image", "image manquante"))
}

#[utoipa::path(delete, operation_id = "deleteRecipeImage", path = "/recipes/{id}/image", tag = "recipes", params(("id" = RecipeId, Path)), responses((status = 204)))]
pub async fn delete_image(State(state): State<AppState>, Path(id): Path<RecipeId>) -> AppResult<StatusCode> {
    images::delete(&state.db, &state.data, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

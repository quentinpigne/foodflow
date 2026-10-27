use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::RowId;

use crate::error::{AppResult, Problem};
use crate::services::ai::{self, AiPreview, AiSettings, AiSettingsInput, RecipeIdeaInput};
use crate::services::ai_planning::{self, AcceptInput, AcceptResult, PlanIdeaInput, Proposal};
use crate::services::recipe_import::RecipeDraft;
use crate::state::AppState;

/// Réglages de l'IA ; la clé n'est jamais renvoyée (ADR-0014).
#[utoipa::path(get, operation_id = "getAiSettings", path = "/ai/settings", tag = "ai", responses((status = 200, body = AiSettings)))]
pub async fn settings(State(state): State<AppState>) -> AppResult<Json<AiSettings>> {
    Ok(Json(ai::get_settings(&state.db, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateAiSettings", path = "/ai/settings", tag = "ai", request_body = AiSettingsInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<AiSettingsInput>,
) -> AppResult<StatusCode> {
    ai::update_settings(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ce qui sera envoyé pour créer une recette (R-REC-81).
#[utoipa::path(
    post, operation_id = "previewAiRecipe", path = "/ai/recipes/preview", tag = "ai", request_body = RecipeIdeaInput,
    responses((status = 200, body = AiPreview), (status = 422, body = Problem))
)]
pub async fn recipe_preview(
    State(state): State<AppState>,
    Json(input): Json<RecipeIdeaInput>,
) -> AppResult<Json<AiPreview>> {
    Ok(Json(ai::recipe_preview(&state.db, &input, state.clock.today()).await?))
}

/// Demande une recette à l'IA : elle arrive en brouillon (REC-19).
#[utoipa::path(
    post, operation_id = "createAiRecipe", path = "/ai/recipes", tag = "ai", request_body = RecipeIdeaInput,
    responses((status = 201, body = RecipeDraft), (status = 422, body = Problem))
)]
pub async fn create_recipe(
    State(state): State<AppState>,
    Json(input): Json<RecipeIdeaInput>,
) -> AppResult<(StatusCode, Json<RecipeDraft>)> {
    let draft = ai::create_recipe(
        &state.db,
        &state.data,
        state.fetcher.as_ref(),
        state.ai.as_ref(),
        &input,
        state.clock.today(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(draft)))
}

/// Ce qui sera envoyé pour un planning (R-PLA-66).
#[utoipa::path(
    post, operation_id = "previewAiPlan", path = "/ai/plans/preview", tag = "ai", request_body = PlanIdeaInput,
    responses((status = 200, body = AiPreview), (status = 422, body = Problem))
)]
pub async fn plan_preview(
    State(state): State<AppState>,
    Json(input): Json<PlanIdeaInput>,
) -> AppResult<Json<AiPreview>> {
    Ok(Json(ai_planning::preview(&state.db, &input, state.clock.today()).await?))
}

/// Demande un planning à l'IA (PLA-12) : une proposition, rien n'est écrit dans les repas.
#[utoipa::path(
    post, operation_id = "createAiPlan", path = "/ai/plans", tag = "ai", request_body = PlanIdeaInput,
    responses((status = 201, body = Proposal), (status = 422, body = Problem))
)]
pub async fn create_plan(
    State(state): State<AppState>,
    Json(input): Json<PlanIdeaInput>,
) -> AppResult<(StatusCode, Json<Proposal>)> {
    let proposal = ai_planning::generate(
        &state.db,
        &state.data,
        state.fetcher.as_ref(),
        state.ai.as_ref(),
        &input,
        state.clock.today(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(proposal)))
}

/// Propositions encore ouvertes.
#[utoipa::path(get, operation_id = "listAiPlans", path = "/ai/plans", tag = "ai", responses((status = 200, body = Vec<Proposal>)))]
pub async fn plans(State(state): State<AppState>) -> AppResult<Json<Vec<Proposal>>> {
    Ok(Json(ai_planning::list(&state.db, state.clock.today()).await?))
}

#[utoipa::path(get, operation_id = "getAiPlan", path = "/ai/plans/{id}", tag = "ai", params(("id" = RowId, Path)), responses((status = 200, body = Proposal), (status = 404, body = Problem)))]
pub async fn plan(State(state): State<AppState>, Path(id): Path<RowId>) -> AppResult<Json<Proposal>> {
    Ok(Json(ai_planning::get(&state.db, id, state.clock.today()).await?))
}

/// Accepte une proposition, en tout ou repas par repas (R-PLA-64).
#[utoipa::path(
    post, operation_id = "acceptAiPlan", path = "/ai/plans/{id}/accept", tag = "ai", params(("id" = RowId, Path)),
    request_body = AcceptInput, responses((status = 200, body = AcceptResult), (status = 422, body = Problem))
)]
pub async fn accept(
    State(state): State<AppState>,
    Path(id): Path<RowId>,
    Json(input): Json<AcceptInput>,
) -> AppResult<Json<AcceptResult>> {
    Ok(Json(ai_planning::accept(&state.db, id, &input, state.clock.today()).await?))
}

#[utoipa::path(delete, operation_id = "discardAiPlan", path = "/ai/plans/{id}", tag = "ai", params(("id" = RowId, Path)), responses((status = 204)))]
pub async fn discard(State(state): State<AppState>, Path(id): Path<RowId>) -> AppResult<StatusCode> {
    ai_planning::discard(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

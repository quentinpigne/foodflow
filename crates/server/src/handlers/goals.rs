use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::{GoalTemplateId, NutritionGoalId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::goals::{self, GoalTemplate, GoalVersion, MealSlot, NewVersionInput, TemplateInput, VersionInput};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedGoal {
    pub id: NutritionGoalId,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedTemplate {
    pub id: GoalTemplateId,
}

/// Versions de l'objectif du foyer, avec leurs critères (NUT-01).
#[utoipa::path(get, operation_id = "listGoalVersions", path = "/goals", tag = "goals", responses((status = 200, body = Vec<GoalVersion>)))]
pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<GoalVersion>>> {
    Ok(Json(goals::versions(&state.db, state.clock.today()).await?))
}

/// Nouvelle version, depuis un modèle, une version ou celle en vigueur (R-NUT-3, R-NUT-4).
#[utoipa::path(
    post, operation_id = "createGoalVersion", path = "/goals", tag = "goals", request_body = NewVersionInput,
    responses((status = 201, body = CreatedGoal), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<NewVersionInput>,
) -> AppResult<(StatusCode, Json<CreatedGoal>)> {
    let id = goals::create_version(&state.db, &input, state.clock.today()).await?;
    Ok((StatusCode::CREATED, Json(CreatedGoal { id })))
}

/// Modifie une version à venir (R-NUT-3).
#[utoipa::path(
    put, operation_id = "updateGoalVersion", path = "/goals/{id}", tag = "goals", params(("id" = NutritionGoalId, Path)),
    request_body = VersionInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<NutritionGoalId>,
    Json(input): Json<VersionInput>,
) -> AppResult<StatusCode> {
    goals::update_version(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id = "deleteGoalVersion", path = "/goals/{id}", tag = "goals", params(("id" = NutritionGoalId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete(State(state): State<AppState>, Path(id): Path<NutritionGoalId>) -> AppResult<StatusCode> {
    goals::delete_version(&state.db, id, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Modèles d'objectif, dont « Repères PNNS 2019 — adulte » (NUT-03).
#[utoipa::path(get, operation_id = "listGoalTemplates", path = "/goal-templates", tag = "goals", responses((status = 200, body = Vec<GoalTemplate>)))]
pub async fn templates(State(state): State<AppState>) -> AppResult<Json<Vec<GoalTemplate>>> {
    Ok(Json(goals::templates(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createGoalTemplate", path = "/goal-templates", tag = "goals", request_body = TemplateInput,
    responses((status = 201, body = CreatedTemplate), (status = 422, body = Problem))
)]
pub async fn create_template(
    State(state): State<AppState>,
    Json(input): Json<TemplateInput>,
) -> AppResult<(StatusCode, Json<CreatedTemplate>)> {
    let id = goals::create_template(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedTemplate { id })))
}

#[utoipa::path(delete, operation_id = "deleteGoalTemplate", path = "/goal-templates/{id}", tag = "goals", params(("id" = GoalTemplateId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete_template(State(state): State<AppState>, Path(id): Path<GoalTemplateId>) -> AppResult<StatusCode> {
    goals::delete_template(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Créneaux de repas et part de l'énergie de chacun (R-NUT-30).
#[utoipa::path(get, operation_id = "listMealSlots", path = "/meal-slots", tag = "goals", responses((status = 200, body = Vec<MealSlot>)))]
pub async fn slots(State(state): State<AppState>) -> AppResult<Json<Vec<MealSlot>>> {
    Ok(Json(goals::slots(&state.db).await?))
}

#[utoipa::path(
    put, operation_id = "saveMealSlots", path = "/meal-slots", tag = "goals", request_body = Vec<MealSlot>,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn save_slots(State(state): State<AppState>, Json(input): Json<Vec<MealSlot>>) -> AppResult<StatusCode> {
    goals::save_slots(&state.db, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::{HabitualMealId, MealId, MealSlotId, MemberId};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::assessment::{self, AssessmentQuery, NutritionAssessment};
use crate::services::household::{self, Member, MemberInput};
use crate::services::meal_validation::{
    self, BatchValidationInput, BatchValidationResult, ValidationInput, ValidationProposal,
};
use crate::services::meals::{self, CopyInput, CopyResult, Meal, MealInput, NewMeal, Plan, PlanQuery};
use crate::services::week::{self, HabitualMeal, HabitualMealInput, SlotTemplate, SlotTemplateInput};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedMember {
    pub id: MemberId,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedHabitualMeal {
    pub id: HabitualMealId,
}

#[derive(Serialize, ToSchema)]
pub struct CreatedMeal {
    pub id: MealId,
}

// ===================================================================== Foyer

/// Membres du foyer, avec leurs absences par défaut et leurs préférences (PLA-01, PLA-06).
#[utoipa::path(get, operation_id = "listMembers", path = "/members", tag = "planning", responses((status = 200, body = Vec<Member>)))]
pub async fn members(State(state): State<AppState>) -> AppResult<Json<Vec<Member>>> {
    Ok(Json(household::members(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createMember", path = "/members", tag = "planning", request_body = MemberInput,
    responses((status = 201, body = CreatedMember), (status = 422, body = Problem))
)]
pub async fn create_member(
    State(state): State<AppState>,
    Json(input): Json<MemberInput>,
) -> AppResult<(StatusCode, Json<CreatedMember>)> {
    let id = household::save_member(&state.db, None, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedMember { id })))
}

#[utoipa::path(
    put, operation_id = "updateMember", path = "/members/{id}", tag = "planning", params(("id" = MemberId, Path)),
    request_body = MemberInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_member(
    State(state): State<AppState>,
    Path(id): Path<MemberId>,
    Json(input): Json<MemberInput>,
) -> AppResult<StatusCode> {
    household::save_member(&state.db, Some(id), &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Supprime un membre, ou le désactive s'il a déjà été convive.
#[utoipa::path(delete, operation_id = "deleteMember", path = "/members/{id}", tag = "planning", params(("id" = MemberId, Path)), responses((status = 204)))]
pub async fn delete_member(State(state): State<AppState>, Path(id): Path<MemberId>) -> AppResult<StatusCode> {
    household::delete_member(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===================================================================== Semaine type

/// Semaine type : horaires et composition type de chaque créneau actif (PLA-02, PLA-07).
#[utoipa::path(get, operation_id = "getWeekTemplate", path = "/week-template", tag = "planning", responses((status = 200, body = Vec<SlotTemplate>)))]
pub async fn template(State(state): State<AppState>) -> AppResult<Json<Vec<SlotTemplate>>> {
    Ok(Json(week::template(&state.db).await?))
}

#[utoipa::path(
    put, operation_id = "saveSlotTemplate", path = "/week-template/{slot_id}", tag = "planning",
    params(("slot_id" = MealSlotId, Path)), request_body = SlotTemplateInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn save_template(
    State(state): State<AppState>,
    Path(slot): Path<MealSlotId>,
    Json(input): Json<SlotTemplateInput>,
) -> AppResult<StatusCode> {
    week::save_template(&state.db, slot, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Repas habituels (PLA-08).
#[utoipa::path(get, operation_id = "listHabitualMeals", path = "/habitual-meals", tag = "planning", responses((status = 200, body = Vec<HabitualMeal>)))]
pub async fn habitual_meals(State(state): State<AppState>) -> AppResult<Json<Vec<HabitualMeal>>> {
    Ok(Json(week::habitual_meals(&state.db).await?))
}

#[utoipa::path(
    post, operation_id = "createHabitualMeal", path = "/habitual-meals", tag = "planning",
    request_body = HabitualMealInput, responses((status = 201, body = CreatedHabitualMeal), (status = 422, body = Problem))
)]
pub async fn create_habitual(
    State(state): State<AppState>,
    Json(input): Json<HabitualMealInput>,
) -> AppResult<(StatusCode, Json<CreatedHabitualMeal>)> {
    let id = week::save_habitual(&state.db, None, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedHabitualMeal { id })))
}

#[utoipa::path(
    put, operation_id = "updateHabitualMeal", path = "/habitual-meals/{id}", tag = "planning",
    params(("id" = HabitualMealId, Path)), request_body = HabitualMealInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_habitual(
    State(state): State<AppState>,
    Path(id): Path<HabitualMealId>,
    Json(input): Json<HabitualMealInput>,
) -> AppResult<StatusCode> {
    week::save_habitual(&state.db, Some(id), &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id = "deleteHabitualMeal", path = "/habitual-meals/{id}", tag = "planning", params(("id" = HabitualMealId, Path)), responses((status = 204)))]
pub async fn delete_habitual(State(state): State<AppState>, Path(id): Path<HabitualMealId>) -> AppResult<StatusCode> {
    week::delete_habitual(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===================================================================== Repas

/// Repas d'une période, jour par jour, avec leurs signalements (PLA-03, PLA-05).
#[utoipa::path(get, operation_id = "getPlan", path = "/plan", tag = "planning", params(PlanQuery), responses((status = 200, body = Plan)))]
pub async fn plan(State(state): State<AppState>, Query(query): Query<PlanQuery>) -> AppResult<Json<Plan>> {
    Ok(Json(meals::plan(&state.db, &query, state.clock.today()).await?))
}

/// Copie une semaine passée comme point de départ (PLA-13).
#[utoipa::path(
    post, operation_id = "copyPlan", path = "/plan/copy", tag = "planning", request_body = CopyInput,
    responses((status = 200, body = CopyResult), (status = 422, body = Problem))
)]
pub async fn copy(State(state): State<AppState>, Json(input): Json<CopyInput>) -> AppResult<Json<CopyResult>> {
    Ok(Json(meals::copy(&state.db, &input).await?))
}

/// Crée un repas d'après la semaine type (R-PLA-5, R-PLA-21).
#[utoipa::path(
    post, operation_id = "createMeal", path = "/meals", tag = "planning", request_body = NewMeal,
    responses((status = 201, body = CreatedMeal), (status = 422, body = Problem))
)]
pub async fn create_meal(
    State(state): State<AppState>,
    Json(input): Json<NewMeal>,
) -> AppResult<(StatusCode, Json<CreatedMeal>)> {
    let id = meals::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedMeal { id })))
}

#[utoipa::path(get, operation_id = "getMeal", path = "/meals/{id}", tag = "planning", params(("id" = MealId, Path)), responses((status = 200, body = Meal), (status = 404, body = Problem)))]
pub async fn meal(State(state): State<AppState>, Path(id): Path<MealId>) -> AppResult<Json<Meal>> {
    Ok(Json(meals::get(&state.db, id, state.clock.today()).await?))
}

#[utoipa::path(
    put, operation_id = "updateMeal", path = "/meals/{id}", tag = "planning", params(("id" = MealId, Path)),
    request_body = MealInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update_meal(
    State(state): State<AppState>,
    Path(id): Path<MealId>,
    Json(input): Json<MealInput>,
) -> AppResult<StatusCode> {
    meals::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id = "deleteMeal", path = "/meals/{id}", tag = "planning", params(("id" = MealId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete_meal(State(state): State<AppState>, Path(id): Path<MealId>) -> AppResult<StatusCode> {
    meals::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ===================================================================== Validation

/// Sorties de stock et restes proposés pour valider un repas (R-PLA-30).
#[utoipa::path(get, operation_id = "getMealValidation", path = "/meals/{id}/validation", tag = "planning", params(("id" = MealId, Path)), responses((status = 200, body = ValidationProposal)))]
pub async fn proposal(State(state): State<AppState>, Path(id): Path<MealId>) -> AppResult<Json<ValidationProposal>> {
    Ok(Json(meal_validation::proposal(&state.db, id, state.clock.today()).await?))
}

/// Valide un repas (PLA-04).
#[utoipa::path(
    post, operation_id = "validateMeal", path = "/meals/{id}/validation", tag = "planning",
    params(("id" = MealId, Path)), request_body = ValidationInput,
    responses((status = 204), (status = 422, body = Problem))
)]
pub async fn validate(
    State(state): State<AppState>,
    Path(id): Path<MealId>,
    Json(input): Json<ValidationInput>,
) -> AppResult<StatusCode> {
    meal_validation::validate(&state.db, id, &input, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Annule la validation d'un repas (R-PLA-34).
#[utoipa::path(delete, operation_id = "unvalidateMeal", path = "/meals/{id}/validation", tag = "planning", params(("id" = MealId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn unvalidate(State(state): State<AppState>, Path(id): Path<MealId>) -> AppResult<StatusCode> {
    meal_validation::unvalidate(&state.db, id, state.clock.today()).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Valide plusieurs repas avec les valeurs proposées (R-PLA-33).
#[utoipa::path(
    post, operation_id = "validateMeals", path = "/meals/validation", tag = "planning",
    request_body = BatchValidationInput, responses((status = 200, body = BatchValidationResult))
)]
pub async fn validate_many(
    State(state): State<AppState>,
    Json(input): Json<BatchValidationInput>,
) -> AppResult<Json<BatchValidationResult>> {
    Ok(Json(meal_validation::validate_many(&state.db, &input, state.clock.today()).await?))
}

// ===================================================================== Bilan

/// Bilan nutritionnel prévu ou réel d'une période (NUT-04).
#[utoipa::path(get, operation_id = "getAssessment", path = "/assessment", tag = "goals", params(AssessmentQuery), responses((status = 200, body = NutritionAssessment)))]
pub async fn assessment(
    State(state): State<AppState>,
    Query(query): Query<AssessmentQuery>,
) -> AppResult<Json<NutritionAssessment>> {
    Ok(Json(assessment::assess(&state.db, &query, state.clock.today()).await?))
}

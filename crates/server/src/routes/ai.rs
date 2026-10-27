use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::ai;
use crate::state::AppState;

/// IA (ADR-0014) : réglages, recettes, plannings.
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(ai::settings, ai::update_settings))
        .routes(routes!(ai::recipe_preview))
        .routes(routes!(ai::create_recipe))
        .routes(routes!(ai::plan_preview))
        .routes(routes!(ai::create_plan, ai::plans))
        .routes(routes!(ai::plan, ai::discard))
        .routes(routes!(ai::accept))
}

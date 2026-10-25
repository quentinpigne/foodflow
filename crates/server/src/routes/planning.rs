use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::planning;
use crate::state::AppState;

/// Planification (spec 05) et bilan nutritionnel (spec 04).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(planning::members, planning::create_member))
        .routes(routes!(planning::update_member, planning::delete_member))
        .routes(routes!(planning::template))
        .routes(routes!(planning::save_template))
        .routes(routes!(planning::habitual_meals, planning::create_habitual))
        .routes(routes!(planning::update_habitual, planning::delete_habitual))
        .routes(routes!(planning::plan))
        .routes(routes!(planning::copy))
        .routes(routes!(planning::create_meal))
        .routes(routes!(planning::meal, planning::update_meal, planning::delete_meal))
        .routes(routes!(planning::proposal, planning::validate, planning::unvalidate))
        .routes(routes!(planning::validate_many))
        .routes(routes!(planning::assessment))
}

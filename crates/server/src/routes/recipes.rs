use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{goals, recipe_drafts, recipes};
use crate::state::AppState;

/// Recettes (spec 03) et objectifs nutritionnels (spec 04).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(recipes::search, recipes::create))
        .routes(routes!(recipes::tags))
        .routes(routes!(recipes::get, recipes::update, recipes::delete))
        .routes(routes!(recipes::archive))
        .routes(routes!(recipes::duplicate))
        .routes(routes!(recipes::json_ld))
        .routes(routes!(recipes::image, recipes::upload_image, recipes::delete_image))
        .routes(routes!(recipe_drafts::start, recipe_drafts::list))
        .routes(routes!(recipe_drafts::get, recipe_drafts::save, recipe_drafts::discard))
        .routes(routes!(recipe_drafts::commit))
        .routes(routes!(goals::list, goals::create))
        .routes(routes!(goals::update, goals::delete))
        .routes(routes!(goals::templates, goals::create_template))
        .routes(routes!(goals::delete_template))
        .routes(routes!(goals::slots, goals::save_slots))
}

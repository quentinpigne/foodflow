use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{foods, groups, products, units};
use crate::state::AppState;

/// Référentiel des aliments (spec 01).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(groups::list, groups::create))
        .routes(routes!(groups::update, groups::delete))
        .routes(routes!(groups::merge))
        .routes(routes!(foods::search, foods::create))
        .routes(routes!(foods::get, foods::update, foods::delete))
        .routes(routes!(foods::set_nutrients))
        .routes(routes!(foods::restore_ciqual))
        .routes(routes!(foods::change_tracking_mode))
        .routes(routes!(foods::add_unit))
        .routes(routes!(foods::update_unit, foods::delete_unit))
        .routes(routes!(units::list))
        .routes(routes!(products::search, products::create))
        .routes(routes!(products::get, products::update, products::delete))
}

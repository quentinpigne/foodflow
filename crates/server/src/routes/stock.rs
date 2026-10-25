use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{dashboard, locations, stock};
use crate::state::AppState;

/// Stock (spec 02) et tableau de bord (spec 09).
pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(locations::list, locations::create))
        .routes(routes!(locations::update, locations::delete))
        .routes(routes!(stock::list, stock::create))
        .routes(routes!(stock::get, stock::update))
        .routes(routes!(stock::consume))
        .routes(routes!(stock::waste))
        .routes(routes!(stock::adjust))
        .routes(routes!(stock::open))
        .routes(routes!(stock::check))
        .routes(routes!(stock::move_lot))
        .routes(routes!(stock::reverse))
        .routes(routes!(stock::inventory))
        .routes(routes!(dashboard::get))
}

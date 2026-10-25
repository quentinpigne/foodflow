use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::{settings, system};
use crate::state::AppState;

/// Sans session : le serveur répond.
pub fn open() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(system::health))
}

pub fn protected() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(system::info))
        .routes(routes!(system::export))
        .routes(routes!(settings::get, settings::update))
}

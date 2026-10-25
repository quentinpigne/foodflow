use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::handlers::auth;
use crate::state::AppState;

/// Sans session : état de la session et connexion.
pub fn open() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(auth::status)).routes(routes!(auth::login))
}

pub fn protected() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(auth::logout)).routes(routes!(auth::logout_everywhere))
}

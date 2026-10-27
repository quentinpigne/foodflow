//! Routes : assemblage de l'API REST sous `/api/v1`, décrite par utoipa (ADR-0009), et des fichiers de
//! l'interface. Chaque sous-module branche les handlers d'un domaine.

mod ai;
mod auth;
mod foods;
mod planning;
mod recipes;
mod static_files;
mod stock;
mod system;

use axum::Router;
use axum::middleware as layer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;

use crate::error::Problem;
use crate::middleware::require_session;
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(title = "Foodflow", description = "API de gestion alimentaire du foyer (ADR-0009)."),
    components(schemas(Problem)),
    tags(
        (name = "auth", description = "Session"),
        (name = "foods", description = "Familles, aliments, produits, unités (spec 01)"),
        (name = "stock", description = "Emplacements, lots, mouvements, inventaire (spec 02)"),
        (name = "recipes", description = "Recettes, import, export (spec 03)"),
        (name = "goals", description = "Objectifs nutritionnels, modèles, créneaux, bilan (spec 04)"),
        (name = "planning", description = "Foyer, semaine type, repas, validation (spec 05)"),
        (name = "ai", description = "IA : réglages, recettes, plannings (ADR-0014)"),
        (name = "dashboard", description = "Tableau de bord (spec 09)"),
        (name = "system", description = "Réglages, export, version"),
    )
)]
struct ApiDoc;

fn protected_routes() -> OpenApiRouter<AppState> {
    auth::protected()
        .merge(foods::router())
        .merge(stock::router())
        .merge(recipes::router())
        .merge(planning::router())
        .merge(ai::router())
        .merge(system::protected())
}

/// Routes de l'API. Sans état (génération du contrat), le middleware de session n'est pas branché.
fn api_router(state: Option<AppState>) -> OpenApiRouter<AppState> {
    let protected = match state {
        Some(state) => protected_routes().route_layer(layer::from_fn_with_state(state, require_session)),
        None => protected_routes(),
    };
    let open = auth::open().merge(system::open());
    OpenApiRouter::with_openapi(ApiDoc::openapi()).nest("/api/v1", open.merge(protected))
}

/// Application complète : API et interface compilée.
pub fn router(state: AppState, web_dir: std::path::PathBuf) -> Router {
    let (api, _) = api_router(Some(state.clone())).split_for_parts();
    api.with_state(state).merge(static_files::router(web_dir))
}

/// Contrat OpenAPI au format JSON, tel qu'il est versionné dans `openapi.json`.
pub fn openapi_json() -> serde_json::Result<String> {
    let (_, api) = api_router(None).split_for_parts();
    api.to_pretty_json()
}

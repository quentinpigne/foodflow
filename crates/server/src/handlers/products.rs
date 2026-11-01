use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use foodflow_domain::shared::ids::ProductId;
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::{AppResult, Problem};
use crate::services::products::{self, BarcodeLookup, Product, ProductInput, ProductQuery};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CreatedProduct {
    pub id: ProductId,
}

/// Lecture d'un code-barres : produit connu, ou brouillon d'Open Food Facts pour un code inconnu ; valeurs
/// proposées pour ses lots (R-STK-50, R-STK-53, R-ALI-60).
#[utoipa::path(
    get, operation_id = "lookupBarcode", path = "/barcodes/{code}", tag = "foods", params(("code" = String, Path)),
    responses((status = 200, body = BarcodeLookup), (status = 422, body = Problem))
)]
pub async fn lookup(State(state): State<AppState>, Path(code): Path<String>) -> AppResult<Json<BarcodeLookup>> {
    Ok(Json(products::lookup_barcode(&state.db, state.off.as_ref(), &code).await?))
}

/// Recherche de produits, par code-barres ou par nom (ALI-05, ALI-06).
#[utoipa::path(get, operation_id = "searchProducts", path = "/products", tag = "foods", params(ProductQuery), responses((status = 200, body = Vec<Product>), (status = 422, body = Problem)))]
pub async fn search(State(state): State<AppState>, Query(query): Query<ProductQuery>) -> AppResult<Json<Vec<Product>>> {
    Ok(Json(products::search(&state.db, &query).await?))
}

#[utoipa::path(
    post, operation_id = "createProduct", path = "/products", tag = "foods", request_body = ProductInput,
    responses((status = 201, body = CreatedProduct), (status = 422, body = Problem))
)]
pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<ProductInput>,
) -> AppResult<(StatusCode, Json<CreatedProduct>)> {
    let id = products::create(&state.db, &input).await?;
    Ok((StatusCode::CREATED, Json(CreatedProduct { id })))
}

#[utoipa::path(get, operation_id = "getProduct", path = "/products/{id}", tag = "foods", params(("id" = ProductId, Path)), responses((status = 200, body = Product), (status = 404, body = Problem)))]
pub async fn get(State(state): State<AppState>, Path(id): Path<ProductId>) -> AppResult<Json<Product>> {
    Ok(Json(products::get(&state.db, id).await?))
}

#[utoipa::path(
    put, operation_id = "updateProduct", path = "/products/{id}", tag = "foods", params(("id" = ProductId, Path)),
    request_body = ProductInput, responses((status = 204), (status = 422, body = Problem))
)]
pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<ProductId>,
    Json(input): Json<ProductInput>,
) -> AppResult<StatusCode> {
    products::update(&state.db, id, &input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, operation_id = "deleteProduct", path = "/products/{id}", tag = "foods", params(("id" = ProductId, Path)), responses((status = 204), (status = 422, body = Problem)))]
pub async fn delete(State(state): State<AppState>, Path(id): Path<ProductId>) -> AppResult<StatusCode> {
    products::delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

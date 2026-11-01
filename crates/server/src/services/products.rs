//! Produits du commerce (spec 01, ALI-05).

use foodflow_domain::foods::NutrientBasis;
use foodflow_domain::foods::TrackingMode;
use foodflow_domain::foods::barcode;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, LocationId, ProductId};
use foodflow_domain::stock::ExpiryKind;
use foodflow_domain::text::matches;
use foodflow_domain::{Amount, DomainError, Measure};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::catalog::Catalog;
use super::{clean, required};
use crate::db::products::{ProductFields, ProductRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct Product {
    pub id: ProductId,
    pub food_id: Option<FoodId>,
    pub group_id: Option<FoodGroupId>,
    /// Aliment ou famille de rattachement, pour l'affichage.
    pub attached_to: String,
    pub name: String,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    /// Mesure effective : celle de l'aliment, ou la sienne s'il est rattaché à une famille.
    pub stock_measure: Measure,
    pub package_amount: Option<Amount>,
    pub package_units: Option<i64>,
    pub nutrient_basis: NutrientBasis,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProductInput {
    pub food_id: Option<FoodId>,
    pub group_id: Option<FoodGroupId>,
    pub name: String,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    /// Requise pour un produit rattaché à une famille.
    pub stock_measure: Option<Measure>,
    pub package_amount: Option<Amount>,
    pub package_units: Option<i64>,
    pub nutrient_basis: Option<NutrientBasis>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ProductQuery {
    pub q: Option<String>,
    pub barcode: Option<String>,
}

pub fn to_product(catalog: &Catalog, row: &ProductRow) -> Product {
    let attached_to = match (row.food_id.and_then(|f| catalog.foods.get(&f)), row.group_id) {
        (Some(food), _) => food.name.clone(),
        (None, Some(group)) => catalog.group_path(group),
        (None, None) => String::new(),
    };
    Product {
        id: row.id,
        food_id: row.food_id,
        group_id: row.group_id,
        attached_to,
        name: row.name.clone(),
        brand: row.brand.clone(),
        barcode: row.barcode.clone(),
        stock_measure: catalog.product_settings(row).stock_measure,
        package_amount: row.package_amount,
        package_units: row.package_units,
        nutrient_basis: row.nutrient_basis,
    }
}

/// Recherche par code-barres exact, ou par mots du nom et de la marque.
pub async fn search(db: &Db, query: &ProductQuery) -> AppResult<Vec<Product>> {
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    if let Some(code) = clean(query.barcode.as_deref()) {
        let code = barcode::validate(&code)?;
        let found = db::products::find_by_barcode(&mut *conn, &code).await?;
        return Ok(found.iter().map(|p| to_product(&catalog, p)).collect());
    }
    let q = clean(query.q.as_deref());
    let mut products: Vec<Product> = catalog
        .products
        .values()
        .filter(|p| {
            q.as_deref().is_none_or(|q| {
                let text =
                    foodflow_domain::text::normalize(&format!("{} {}", p.name, p.brand.as_deref().unwrap_or("")));
                matches(&text, q)
            })
        })
        .map(|p| to_product(&catalog, p))
        .collect();
    products.sort_by_key(|p| p.name.to_lowercase());
    products.truncate(50);
    Ok(products)
}

/// Ce que donne la lecture d'un code-barres (R-STK-50) : le produit s'il est connu, et les valeurs
/// proposées pour ses lots (R-STK-53).
#[derive(Debug, Serialize, ToSchema)]
pub struct BarcodeLookup {
    /// Code nettoyé et vérifié (R-ALI-41).
    pub barcode: String,
    /// `None` : code inconnu, à rattacher (R-STK-52).
    pub product: Option<Product>,
    pub tracking_mode: Option<TrackingMode>,
    /// Emplacement du dernier lot du produit, sinon de son aliment.
    pub location_id: Option<LocationId>,
    /// Nature de péremption du dernier lot du produit ; sinon `NONE` si l'aliment a une durée de
    /// conservation sans date, `USE_BY` sinon.
    pub expiry_kind: Option<ExpiryKind>,
}

/// Lecture d'un code-barres pour la mise en stock ou l'inventaire par scan (R-STK-50, R-STK-53).
pub async fn lookup_barcode(db: &Db, code: &str) -> AppResult<BarcodeLookup> {
    let code = barcode::validate(code)?;
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    let Some(row) = catalog.products.values().find(|p| p.barcode.as_deref() == Some(code.as_str())) else {
        return Ok(BarcodeLookup {
            barcode: code,
            product: None,
            tracking_mode: None,
            location_id: None,
            expiry_kind: None,
        });
    };
    let settings = catalog.product_settings(row);
    let own = db::stock::latest_lot_of(&mut *conn, Some(row.id), None).await?;
    let food = match (own, row.food_id) {
        (None, Some(food)) => db::stock::latest_lot_of(&mut *conn, None, Some(food)).await?,
        _ => None,
    };
    let expiry_kind = match own {
        Some((_, kind)) => kind,
        None if settings.shelf_life_days.is_some() => ExpiryKind::None,
        None => ExpiryKind::UseBy,
    };
    Ok(BarcodeLookup {
        barcode: code,
        product: Some(to_product(&catalog, row)),
        tracking_mode: Some(settings.tracking_mode),
        location_id: own.or(food).map(|(location, _)| location),
        expiry_kind: Some(expiry_kind),
    })
}

pub async fn get(db: &Db, id: ProductId) -> AppResult<Product> {
    let mut conn = db.read.acquire().await?;
    let catalog = Catalog::load(&mut conn).await?;
    let row = catalog.products.get(&id).ok_or(AppError::NotFound("produit"))?;
    Ok(to_product(&catalog, row))
}

/// Valide un produit : rattachement (R-ALI-40), code-barres (R-ALI-41), conditionnement (R-ALI-42).
async fn fields<'a>(
    conn: &mut sqlx::SqliteConnection,
    input: &'a ProductInput,
    name: &'a str,
    barcode: Option<&'a str>,
    except: Option<ProductId>,
) -> AppResult<ProductFields<'a>> {
    match (input.food_id, input.group_id) {
        (Some(food), None) => {
            db::foods::find(&mut *conn, food).await?.ok_or(AppError::NotFound("aliment"))?;
        }
        (None, Some(_)) if input.stock_measure.is_none() => {
            return Err(AppError::invalid_field(
                "STOCK_MEASURE_REQUIRED",
                "stock_measure",
                "un produit rattaché à une famille indique sa mesure de stock",
            ));
        }
        (None, Some(_)) => {}
        _ => return Err(DomainError::ProductAttachment.into()),
    }
    if let Some(code) = barcode
        && let Some(other) = db::products::find_by_barcode(&mut *conn, code).await?
        && Some(other.id) != except
    {
        return Err(DomainError::BarcodeTaken.into());
    }
    if input.package_amount.is_some_and(|a| !a.is_positive()) || input.package_units.is_some_and(|u| u <= 0) {
        return Err(DomainError::NotPositive.into());
    }
    Ok(ProductFields {
        food_id: input.food_id,
        group_id: input.group_id,
        name,
        brand: input.brand.as_deref().map(str::trim).filter(|b| !b.is_empty()),
        barcode,
        stock_measure: if input.group_id.is_some() { input.stock_measure } else { None },
        package_amount: input.package_amount,
        package_units: input.package_units,
        nutrient_basis: input.nutrient_basis.unwrap_or(NutrientBasis::Per100G),
    })
}

pub async fn create(db: &Db, input: &ProductInput) -> AppResult<ProductId> {
    let name = required(&input.name, "name", 200)?;
    let code = clean(input.barcode.as_deref()).map(|c| barcode::validate(&c)).transpose()?;
    let mut tx = db.write.begin().await?;
    let fields = fields(&mut tx, input, &name, code.as_deref(), None).await?;
    let id = ProductId::new();
    db::products::insert(&mut *tx, id, &fields).await?;
    // Un aliment Ciqual masqué devient visible dès qu'on l'utilise (R-ALI-52).
    if let Some(food) = input.food_id {
        db::foods::unhide(&mut *tx, food).await?;
    }
    tx.commit().await?;
    Ok(id)
}

pub async fn update(db: &Db, id: ProductId, input: &ProductInput) -> AppResult<()> {
    let name = required(&input.name, "name", 200)?;
    let code = clean(input.barcode.as_deref()).map(|c| barcode::validate(&c)).transpose()?;
    let mut tx = db.write.begin().await?;
    let current = db::products::find(&mut *tx, id).await?.ok_or(AppError::NotFound("produit"))?;
    let fields = fields(&mut tx, input, &name, code.as_deref(), Some(id)).await?;
    // Les lots déjà en stock gardent leur mesure : on ne change pas celle d'un produit qui a des lots.
    let measure_changed = (current.food_id, current.stock_measure) != (fields.food_id, fields.stock_measure);
    if measure_changed && db::products::lot_count(&mut *tx, id).await? > 0 {
        let mut conn = db.read.acquire().await?;
        let catalog = Catalog::load(&mut conn).await?;
        let mut after = current.clone();
        (after.food_id, after.group_id, after.stock_measure) = (fields.food_id, fields.group_id, fields.stock_measure);
        if catalog.product_settings(&after).stock_measure != catalog.product_settings(&current).stock_measure {
            return Err(AppError::invalid_field(
                "PRODUCT_MEASURE_LOCKED",
                "stock_measure",
                "un produit qui a des lots ne change pas de mesure de stock",
            ));
        }
    }
    db::products::update(&mut *tx, id, &fields).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete(db: &Db, id: ProductId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    db::products::find(&mut *tx, id).await?.ok_or(AppError::NotFound("produit"))?;
    if db::products::lot_count(&mut *tx, id).await? > 0 {
        return Err(AppError::invalid("PRODUCT_IN_USE", "un produit qui a des lots ne peut pas être supprimé"));
    }
    db::products::delete(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(())
}

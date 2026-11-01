//! Produits du commerce (spec 01, R-ALI-40 à R-ALI-43).

use foodflow_domain::foods::NutrientBasis;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, ProductId};
use foodflow_domain::{Amount, Measure};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_enum_from_db, opt_enum_to_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct ProductRow {
    pub id: ProductId,
    pub food_id: Option<FoodId>,
    pub group_id: Option<FoodGroupId>,
    pub name: String,
    pub brand: Option<String>,
    pub barcode: Option<String>,
    pub stock_measure: Option<Measure>,
    pub package_amount: Option<Amount>,
    pub package_units: Option<i64>,
    pub nutrient_basis: NutrientBasis,
}

pub struct ProductFields<'a> {
    pub food_id: Option<FoodId>,
    pub group_id: Option<FoodGroupId>,
    pub name: &'a str,
    pub brand: Option<&'a str>,
    pub barcode: Option<&'a str>,
    pub stock_measure: Option<Measure>,
    pub package_amount: Option<Amount>,
    pub package_units: Option<i64>,
    pub nutrient_basis: NutrientBasis,
}

macro_rules! product_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(ProductRow {
            id: id_from_db(&r.id)?,
            food_id: opt_id_from_db(r.food_id.as_deref())?,
            group_id: opt_id_from_db(r.group_id.as_deref())?,
            name: r.name,
            brand: r.brand,
            barcode: r.barcode,
            stock_measure: opt_enum_from_db(r.stock_measure.as_deref())?,
            package_amount: r.package_amount.map(Amount::from_raw),
            package_units: r.package_units,
            nutrient_basis: enum_from_db(&r.nutrient_basis)?,
        })
    }};
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ProductRow>> {
    let rows = sqlx::query!(
        "SELECT id, food_id, group_id, name, brand, barcode, stock_measure, package_amount, package_units,
                nutrient_basis
         FROM product ORDER BY name COLLATE NOCASE"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| product_row!(r)).collect()
}

pub async fn list_for_food(db: impl SqliteExecutor<'_>, food: FoodId) -> anyhow::Result<Vec<ProductRow>> {
    let food = food.to_string();
    let rows = sqlx::query!(
        "SELECT id, food_id, group_id, name, brand, barcode, stock_measure, package_amount, package_units,
                nutrient_basis
         FROM product WHERE food_id = ? ORDER BY name COLLATE NOCASE",
        food
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| product_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: ProductId) -> anyhow::Result<Option<ProductRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        "SELECT id, food_id, group_id, name, brand, barcode, stock_measure, package_amount, package_units,
                nutrient_basis
         FROM product WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| product_row!(r)).transpose()
}

pub async fn find_by_barcode(db: impl SqliteExecutor<'_>, barcode: &str) -> anyhow::Result<Option<ProductRow>> {
    let row = sqlx::query!(
        "SELECT id, food_id, group_id, name, brand, barcode, stock_measure, package_amount, package_units,
                nutrient_basis
         FROM product WHERE barcode = ?",
        barcode
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| product_row!(r)).transpose()
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: ProductId, f: &ProductFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let food = f.food_id.map(|x| x.to_string());
    let group = f.group_id.map(|x| x.to_string());
    let measure = opt_enum_to_db(&f.stock_measure);
    let package = f.package_amount.map(Amount::raw);
    let basis = enum_to_db(&f.nutrient_basis);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO product (id, food_id, group_id, name, brand, barcode, stock_measure, package_amount,
             package_units, nutrient_basis, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        food,
        group,
        f.name,
        f.brand,
        f.barcode,
        measure,
        package,
        f.package_units,
        basis,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: ProductId, f: &ProductFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let food = f.food_id.map(|x| x.to_string());
    let group = f.group_id.map(|x| x.to_string());
    let measure = opt_enum_to_db(&f.stock_measure);
    let package = f.package_amount.map(Amount::raw);
    let basis = enum_to_db(&f.nutrient_basis);
    let now = timestamp();
    sqlx::query!(
        "UPDATE product SET food_id = ?, group_id = ?, name = ?, brand = ?, barcode = ?, stock_measure = ?,
             package_amount = ?, package_units = ?, nutrient_basis = ?, updated_at = ?
         WHERE id = ?",
        food,
        group,
        f.name,
        f.brand,
        f.barcode,
        measure,
        package,
        f.package_units,
        basis,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: ProductId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM product WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn lot_count(db: impl SqliteExecutor<'_>, id: ProductId) -> anyhow::Result<i64> {
    let id = id.to_string();
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM stock_lot WHERE product_id = ?"#, id)
        .fetch_one(db)
        .await?)
}

/// Garde la fiche brute d'Open Food Facts d'un produit et sa date (R-ALI-61).
pub async fn set_off_payload(db: impl SqliteExecutor<'_>, id: ProductId, payload: &str) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE product SET off_payload = ?, off_fetched_at = ? WHERE id = ?", payload, now, id)
        .execute(db)
        .await?;
    Ok(())
}

/// Valeur d'un nutriment d'un produit, pour 100 g ou 100 mL, avec sa source (`OFF`, `MANUAL`).
pub async fn set_nutrient(
    db: impl SqliteExecutor<'_>,
    id: ProductId,
    code: &str,
    value: i64,
    source: &str,
) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!(
        "INSERT INTO product_nutrient (product_id, nutrient_code, value, estimated, source) VALUES (?, ?, ?, 0, ?)
         ON CONFLICT (product_id, nutrient_code) DO UPDATE SET value = excluded.value, source = excluded.source",
        id,
        code,
        value,
        source
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Rattachements mémorisés des catégories d'Open Food Facts (R-ALI-62).
pub async fn off_mappings(
    db: impl SqliteExecutor<'_>,
) -> anyhow::Result<Vec<(String, Option<FoodId>, Option<FoodGroupId>)>> {
    let rows = sqlx::query!("SELECT off_category, food_id, group_id FROM off_category_mapping").fetch_all(db).await?;
    rows.into_iter()
        .map(|r| Ok((r.off_category, opt_id_from_db(r.food_id.as_deref())?, opt_id_from_db(r.group_id.as_deref())?)))
        .collect()
}

/// Mémorise le rattachement validé d'une catégorie d'Open Food Facts (R-ALI-62).
pub async fn set_off_mapping(
    db: impl SqliteExecutor<'_>,
    category: &str,
    food: Option<FoodId>,
    group: Option<FoodGroupId>,
) -> anyhow::Result<()> {
    let food = food.map(|f| f.to_string());
    let group = group.map(|g| g.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO off_category_mapping (off_category, food_id, group_id, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (off_category) DO UPDATE
             SET food_id = excluded.food_id, group_id = excluded.group_id, updated_at = excluded.updated_at",
        category,
        food,
        group,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

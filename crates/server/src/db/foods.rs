//! Aliments et alias (spec 01, R-ALI-10 à R-ALI-13).

use foodflow_domain::Amount;
use foodflow_domain::foods::NutrientBasis;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId, ProductId};
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_enum_from_db, opt_enum_to_db, opt_id_from_db};
use super::groups::{Settings, days};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct FoodRow {
    pub id: FoodId,
    pub group_id: FoodGroupId,
    pub name: String,
    pub name_key: String,
    pub ciqual_code: Option<String>,
    pub hidden: bool,
    pub nutrient_basis: NutrientBasis,
    pub density_mg_per_ml: Option<i64>,
    pub piece_weight_mg: Option<i64>,
    pub settings: Settings,
    pub minimum_stock: Option<Amount>,
    pub preferred_product_id: Option<ProductId>,
    pub user_modified: bool,
}

pub struct FoodFields<'a> {
    pub group_id: FoodGroupId,
    pub name: &'a str,
    pub name_key: &'a str,
    pub ciqual_code: Option<&'a str>,
    pub hidden: bool,
    pub nutrient_basis: NutrientBasis,
    pub density_mg_per_ml: Option<i64>,
    pub piece_weight_mg: Option<i64>,
    pub settings: Settings,
    pub minimum_stock: Option<Amount>,
    pub preferred_product_id: Option<ProductId>,
    pub user_modified: bool,
}

macro_rules! food_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(FoodRow {
            id: id_from_db(&r.id)?,
            group_id: id_from_db(&r.group_id)?,
            name: r.name,
            name_key: r.name_key,
            ciqual_code: r.ciqual_code,
            hidden: r.hidden,
            nutrient_basis: enum_from_db(&r.nutrient_basis)?,
            density_mg_per_ml: r.density_mg_per_ml,
            piece_weight_mg: r.piece_weight_mg,
            settings: Settings {
                nutrition_group: opt_enum_from_db(r.nutrition_group.as_deref())?,
                stock_measure: opt_enum_from_db(r.stock_measure.as_deref())?,
                tracking_mode: opt_enum_from_db(r.tracking_mode.as_deref())?,
                shelf_life_days: days(r.shelf_life_days),
                opened_shelf_life_days: days(r.opened_shelf_life_days),
                frozen_shelf_life_days: days(r.frozen_shelf_life_days),
                thawed_shelf_life_days: days(r.thawed_shelf_life_days),
                portion_measure: opt_enum_from_db(r.portion_measure.as_deref())?,
                portion_amount: r.portion_amount.map(Amount::from_raw),
            },
            minimum_stock: r.minimum_stock.map(Amount::from_raw),
            preferred_product_id: opt_id_from_db(r.preferred_product_id.as_deref())?,
            user_modified: r.user_modified,
        })
    }};
}

/// Tous les aliments non fusionnés (quelques milliers au plus : la recherche se fait en mémoire).
pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<FoodRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, group_id, name, name_key, ciqual_code, hidden AS "hidden: bool", nutrient_basis,
                  density_mg_per_ml, piece_weight_mg, nutrition_group, stock_measure, tracking_mode,
                  shelf_life_days, opened_shelf_life_days, frozen_shelf_life_days, thawed_shelf_life_days,
                  portion_measure, portion_amount, minimum_stock, preferred_product_id,
                  user_modified AS "user_modified: bool"
           FROM food WHERE merged_into_id IS NULL ORDER BY name_key"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| food_row!(r)).collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: FoodId) -> anyhow::Result<Option<FoodRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, group_id, name, name_key, ciqual_code, hidden AS "hidden: bool", nutrient_basis,
                  density_mg_per_ml, piece_weight_mg, nutrition_group, stock_measure, tracking_mode,
                  shelf_life_days, opened_shelf_life_days, frozen_shelf_life_days, thawed_shelf_life_days,
                  portion_measure, portion_amount, minimum_stock, preferred_product_id,
                  user_modified AS "user_modified: bool"
           FROM food WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| food_row!(r)).transpose()
}

pub async fn find_by_ciqual(db: impl SqliteExecutor<'_>, code: &str) -> anyhow::Result<Option<FoodRow>> {
    let row = sqlx::query!(
        r#"SELECT id, group_id, name, name_key, ciqual_code, hidden AS "hidden: bool", nutrient_basis,
                  density_mg_per_ml, piece_weight_mg, nutrition_group, stock_measure, tracking_mode,
                  shelf_life_days, opened_shelf_life_days, frozen_shelf_life_days, thawed_shelf_life_days,
                  portion_measure, portion_amount, minimum_stock, preferred_product_id,
                  user_modified AS "user_modified: bool"
           FROM food WHERE ciqual_code = ?"#,
        code
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| food_row!(r)).transpose()
}

/// Un nom (ou un alias) est-il déjà pris, sans accents ni casse (R-ALI-10) ?
pub async fn name_taken(db: impl SqliteExecutor<'_>, key: &str, except: Option<FoodId>) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string());
    let count = sqlx::query_scalar!(
        r#"SELECT (SELECT count(*) FROM food WHERE name_key = ? AND (? IS NULL OR id <> ?))
                + (SELECT count(*) FROM food_alias WHERE alias_key = ? AND (? IS NULL OR food_id <> ?)) AS "n!: i64""#,
        key,
        except,
        except,
        key,
        except,
        except
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: FoodId, f: &FoodFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let group = f.group_id.to_string();
    let basis = enum_to_db(&f.nutrient_basis);
    let s = &f.settings;
    let (ng, sm, tm, pm) = (
        opt_enum_to_db(&s.nutrition_group),
        opt_enum_to_db(&s.stock_measure),
        opt_enum_to_db(&s.tracking_mode),
        opt_enum_to_db(&s.portion_measure),
    );
    let pa = s.portion_amount.map(Amount::raw);
    let minimum = f.minimum_stock.map(Amount::raw);
    let preferred = f.preferred_product_id.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO food (id, group_id, name, name_key, ciqual_code, hidden, nutrient_basis, density_mg_per_ml,
             piece_weight_mg, nutrition_group, stock_measure, tracking_mode, shelf_life_days, opened_shelf_life_days,
             frozen_shelf_life_days, thawed_shelf_life_days, portion_measure, portion_amount, minimum_stock,
             preferred_product_id, user_modified, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        group,
        f.name,
        f.name_key,
        f.ciqual_code,
        f.hidden,
        basis,
        f.density_mg_per_ml,
        f.piece_weight_mg,
        ng,
        sm,
        tm,
        s.shelf_life_days,
        s.opened_shelf_life_days,
        s.frozen_shelf_life_days,
        s.thawed_shelf_life_days,
        pm,
        pa,
        minimum,
        preferred,
        f.user_modified,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: FoodId, f: &FoodFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let group = f.group_id.to_string();
    let basis = enum_to_db(&f.nutrient_basis);
    let s = &f.settings;
    let (ng, sm, tm, pm) = (
        opt_enum_to_db(&s.nutrition_group),
        opt_enum_to_db(&s.stock_measure),
        opt_enum_to_db(&s.tracking_mode),
        opt_enum_to_db(&s.portion_measure),
    );
    let pa = s.portion_amount.map(Amount::raw);
    let minimum = f.minimum_stock.map(Amount::raw);
    let preferred = f.preferred_product_id.map(|p| p.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE food SET group_id = ?, name = ?, name_key = ?, hidden = ?, nutrient_basis = ?, density_mg_per_ml = ?,
             piece_weight_mg = ?, nutrition_group = ?, stock_measure = ?, tracking_mode = ?, shelf_life_days = ?,
             opened_shelf_life_days = ?, frozen_shelf_life_days = ?, thawed_shelf_life_days = ?, portion_measure = ?,
             portion_amount = ?, minimum_stock = ?, preferred_product_id = ?, user_modified = ?, updated_at = ?
         WHERE id = ?",
        group,
        f.name,
        f.name_key,
        f.hidden,
        basis,
        f.density_mg_per_ml,
        f.piece_weight_mg,
        ng,
        sm,
        tm,
        s.shelf_life_days,
        s.opened_shelf_life_days,
        s.frozen_shelf_life_days,
        s.thawed_shelf_life_days,
        pm,
        pa,
        minimum,
        preferred,
        f.user_modified,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Affiche un aliment masqué (R-ALI-52) dès qu'on l'utilise.
pub async fn unhide(db: impl SqliteExecutor<'_>, id: FoodId) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE food SET hidden = 0, updated_at = ? WHERE id = ? AND hidden = 1", now, id).execute(db).await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: FoodId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM food WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Nombre d'éléments qui utilisent l'aliment : produits, lots, familles dont il est la référence.
pub async fn usage(db: impl SqliteExecutor<'_>, id: FoodId) -> anyhow::Result<i64> {
    let id = id.to_string();
    let n = sqlx::query_scalar!(
        r#"SELECT (SELECT count(*) FROM product WHERE food_id = ?1)
                + (SELECT count(*) FROM stock_lot WHERE food_id = ?1)
                + (SELECT count(*) FROM food_group WHERE reference_food_id = ?1) AS "n!: i64""#,
        id
    )
    .fetch_one(db)
    .await?;
    Ok(n)
}

pub async fn aliases(db: impl SqliteExecutor<'_>, id: FoodId) -> anyhow::Result<Vec<String>> {
    let id = id.to_string();
    Ok(sqlx::query_scalar!("SELECT alias FROM food_alias WHERE food_id = ? ORDER BY alias_key", id)
        .fetch_all(db)
        .await?)
}

/// Tous les alias, pour la recherche : (aliment, clé normalisée).
pub async fn all_alias_keys(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(FoodId, String)>> {
    let rows = sqlx::query!("SELECT food_id, alias_key FROM food_alias").fetch_all(db).await?;
    rows.into_iter().map(|r| Ok((id_from_db(&r.food_id)?, r.alias_key))).collect()
}

pub async fn replace_aliases(
    conn: &mut sqlx::SqliteConnection,
    id: FoodId,
    aliases: &[(String, String)],
) -> anyhow::Result<()> {
    let food = id.to_string();
    sqlx::query!("DELETE FROM food_alias WHERE food_id = ?", food).execute(&mut *conn).await?;
    for (alias, key) in aliases {
        sqlx::query!("INSERT INTO food_alias (food_id, alias, alias_key) VALUES (?, ?, ?)", food, alias, key)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

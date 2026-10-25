//! Familles d'aliments (spec 01, R-ALI-1 à R-ALI-4).

use foodflow_domain::foods::{NutritionGroup, TrackingMode};
use foodflow_domain::shared::ids::{FoodGroupId, FoodId};
use foodflow_domain::{Amount, Measure};
use serde::{Deserialize, Serialize};
use sqlx::SqliteExecutor;
use utoipa::ToSchema;

use super::codec::{id_from_db, opt_enum_from_db, opt_enum_to_db, opt_id_from_db};
use crate::clock::timestamp;

/// Réglages hérités d'une famille ou d'un aliment (R-ALI-2) : `None` signifie « hérité ».
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Settings {
    pub nutrition_group: Option<NutritionGroup>,
    pub stock_measure: Option<Measure>,
    pub tracking_mode: Option<TrackingMode>,
    /// Durée de conservation sans date de péremption, en jours.
    pub shelf_life_days: Option<u32>,
    pub opened_shelf_life_days: Option<u32>,
    pub frozen_shelf_life_days: Option<u32>,
    pub thawed_shelf_life_days: Option<u32>,
    pub portion_measure: Option<Measure>,
    pub portion_amount: Option<Amount>,
}

pub(crate) fn days(value: Option<i64>) -> Option<u32> {
    value.and_then(|v| u32::try_from(v).ok())
}

#[derive(Debug, Clone)]
pub struct GroupRow {
    pub id: FoodGroupId,
    pub parent_id: Option<FoodGroupId>,
    pub name: String,
    pub ciqual_code: Option<String>,
    pub settings: Settings,
    pub reference_food_id: Option<FoodId>,
    pub user_modified: bool,
    pub food_count: i64,
    pub product_count: i64,
}

pub struct GroupFields<'a> {
    pub parent_id: Option<FoodGroupId>,
    pub name: &'a str,
    pub name_key: &'a str,
    pub ciqual_code: Option<&'a str>,
    pub settings: Settings,
    pub reference_food_id: Option<FoodId>,
    pub user_modified: bool,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<GroupRow>> {
    let rows = sqlx::query!(
        r#"SELECT g.id, g.parent_id, g.name, g.ciqual_code, g.nutrition_group, g.stock_measure, g.tracking_mode,
                  g.shelf_life_days, g.opened_shelf_life_days, g.frozen_shelf_life_days, g.thawed_shelf_life_days,
                  g.portion_measure, g.portion_amount, g.reference_food_id, g.user_modified AS "user_modified: bool",
                  (SELECT count(*) FROM food f WHERE f.group_id = g.id AND f.merged_into_id IS NULL) AS "food_count!: i64",
                  (SELECT count(*) FROM product p WHERE p.group_id = g.id) AS "product_count!: i64"
           FROM food_group g ORDER BY g.name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(GroupRow {
                id: id_from_db(&r.id)?,
                parent_id: opt_id_from_db(r.parent_id.as_deref())?,
                name: r.name,
                ciqual_code: r.ciqual_code,
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
                reference_food_id: opt_id_from_db(r.reference_food_id.as_deref())?,
                user_modified: r.user_modified,
                food_count: r.food_count,
                product_count: r.product_count,
            })
        })
        .collect()
}

pub async fn find_by_ciqual(db: impl SqliteExecutor<'_>, code: &str) -> anyhow::Result<Option<FoodGroupId>> {
    let id = sqlx::query_scalar!("SELECT id FROM food_group WHERE ciqual_code = ?", code).fetch_optional(db).await?;
    id.map(|id| id_from_db(&id)).transpose()
}

/// Un nom est-il déjà pris parmi les enfants d'un même parent (R-ALI-1) ?
pub async fn name_taken(
    db: impl SqliteExecutor<'_>,
    parent: Option<FoodGroupId>,
    name_key: &str,
    except: Option<FoodGroupId>,
) -> anyhow::Result<bool> {
    let parent = parent.map(|p| p.to_string());
    let except = except.map(|e| e.to_string());
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!: i64" FROM food_group
           WHERE parent_id IS ? AND name_key = ? AND (? IS NULL OR id <> ?)"#,
        parent,
        name_key,
        except,
        except
    )
    .fetch_one(db)
    .await?;
    Ok(count > 0)
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: FoodGroupId, f: &GroupFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let parent = f.parent_id.map(|p| p.to_string());
    let s = &f.settings;
    let (ng, sm, tm, pm) = (
        opt_enum_to_db(&s.nutrition_group),
        opt_enum_to_db(&s.stock_measure),
        opt_enum_to_db(&s.tracking_mode),
        opt_enum_to_db(&s.portion_measure),
    );
    let pa = s.portion_amount.map(Amount::raw);
    let reference = f.reference_food_id.map(|r| r.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO food_group (id, parent_id, name, name_key, ciqual_code, nutrition_group, stock_measure,
             tracking_mode, shelf_life_days, opened_shelf_life_days, frozen_shelf_life_days, thawed_shelf_life_days,
             portion_measure, portion_amount, reference_food_id, user_modified, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        parent,
        f.name,
        f.name_key,
        f.ciqual_code,
        ng,
        sm,
        tm,
        s.shelf_life_days,
        s.opened_shelf_life_days,
        s.frozen_shelf_life_days,
        s.thawed_shelf_life_days,
        pm,
        pa,
        reference,
        f.user_modified,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: FoodGroupId, f: &GroupFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let parent = f.parent_id.map(|p| p.to_string());
    let s = &f.settings;
    let (ng, sm, tm, pm) = (
        opt_enum_to_db(&s.nutrition_group),
        opt_enum_to_db(&s.stock_measure),
        opt_enum_to_db(&s.tracking_mode),
        opt_enum_to_db(&s.portion_measure),
    );
    let pa = s.portion_amount.map(Amount::raw);
    let reference = f.reference_food_id.map(|r| r.to_string());
    let now = timestamp();
    sqlx::query!(
        "UPDATE food_group SET parent_id = ?, name = ?, name_key = ?, nutrition_group = ?, stock_measure = ?,
             tracking_mode = ?, shelf_life_days = ?, opened_shelf_life_days = ?, frozen_shelf_life_days = ?,
             thawed_shelf_life_days = ?, portion_measure = ?, portion_amount = ?, reference_food_id = ?,
             user_modified = ?, updated_at = ?
         WHERE id = ?",
        parent,
        f.name,
        f.name_key,
        ng,
        sm,
        tm,
        s.shelf_life_days,
        s.opened_shelf_life_days,
        s.frozen_shelf_life_days,
        s.thawed_shelf_life_days,
        pm,
        pa,
        reference,
        f.user_modified,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: FoodGroupId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM food_group WHERE id = ?", id).execute(db).await?;
    Ok(())
}

/// Fusion (R-ALI-3) : tout ce qui vise `from` est déplacé vers `into`.
pub async fn move_contents(
    conn: &mut sqlx::SqliteConnection,
    from: FoodGroupId,
    into: FoodGroupId,
) -> anyhow::Result<()> {
    let (from, into) = (from.to_string(), into.to_string());
    let now = timestamp();
    sqlx::query!("UPDATE food_group SET parent_id = ?, updated_at = ? WHERE parent_id = ?", into, now, from)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("UPDATE food SET group_id = ?, updated_at = ? WHERE group_id = ?", into, now, from)
        .execute(&mut *conn)
        .await?;
    sqlx::query!("UPDATE product SET group_id = ?, updated_at = ? WHERE group_id = ?", into, now, from)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

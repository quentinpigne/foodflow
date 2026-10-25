//! Planification (spec 05) : foyer, semaine type, repas habituels, repas, convives, éléments et
//! consommations réelles.

use chrono::NaiveTime;
use foodflow_domain::foods::NutritionGroup;
use foodflow_domain::planning::household::PreferenceLevel;
use foodflow_domain::planning::{ItemSource, MealStatus};
use foodflow_domain::recipes::{RecipeKind, Target};
use foodflow_domain::shared::ids::{
    BatchId, FoodId, HabitualMealId, LotId, MealId, MealItemId, MealSlotId, MemberId, MovementId, ProductId, RecipeId,
    RowId, SlotRoleId,
};
use foodflow_domain::{Amount, Date, Measure, Ratio};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{date_from_db, enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use super::recipes::{target_columns, target_from};
use crate::clock::timestamp;

/// Heure « HH:MM » de la base.
pub fn time_from_db(value: &str) -> anyhow::Result<NaiveTime> {
    Ok(NaiveTime::parse_from_str(value, "%H:%M")?)
}

pub fn time_to_db(value: NaiveTime) -> String {
    value.format("%H:%M").to_string()
}

fn weekday_from_db(value: i64) -> anyhow::Result<u32> {
    u32::try_from(value).ok().filter(|d| (1..=7).contains(d)).ok_or_else(|| anyhow::anyhow!("jour invalide"))
}

fn minutes_from_db(value: i64) -> anyhow::Result<u32> {
    Ok(u32::try_from(value)?)
}

// ===================================================================== Foyer

#[derive(Debug, Clone)]
pub struct MemberRow {
    pub id: MemberId,
    pub name: String,
    pub adult_equivalent: Ratio,
    pub active: bool,
    pub position: i64,
}

pub async fn members(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<MemberRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, adult_equivalent, active AS "active: bool", position FROM member ORDER BY position, name"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(MemberRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                adult_equivalent: Ratio::from_thousandths(r.adult_equivalent),
                active: r.active,
                position: r.position,
            })
        })
        .collect()
}

pub async fn member_name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    except: Option<MemberId>,
) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string());
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!: i64" FROM member WHERE lower(name) = lower(?1) AND (?2 IS NULL OR id <> ?2)"#,
        name,
        except
    )
    .fetch_one(db)
    .await?
        > 0)
}

pub async fn upsert_member(db: impl SqliteExecutor<'_>, member: &MemberRow) -> anyhow::Result<()> {
    let id = member.id.to_string();
    let coefficient = member.adult_equivalent.thousandths();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO member (id, name, adult_equivalent, active, position, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, adult_equivalent = excluded.adult_equivalent,
             active = excluded.active, position = excluded.position, updated_at = excluded.updated_at",
        id,
        member.name,
        coefficient,
        member.active,
        member.position,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Un membre jamais convive se supprime ; sinon il se désactive (il reste dans l'historique).
pub async fn member_has_meals(db: impl SqliteExecutor<'_>, id: MemberId) -> anyhow::Result<bool> {
    let id = id.to_string();
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM meal_diner WHERE member_id = ?"#, id)
        .fetch_one(db)
        .await?
        > 0)
}

pub async fn delete_member(db: impl SqliteExecutor<'_>, id: MemberId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM member WHERE id = ?", id).execute(db).await?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct PresenceRow {
    pub member_id: MemberId,
    pub slot_id: MealSlotId,
    pub weekday: u32,
    pub present: bool,
}

pub async fn presences(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PresenceRow>> {
    let rows = sqlx::query!(r#"SELECT member_id, slot_id, weekday, present AS "present: bool" FROM member_presence"#)
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PresenceRow {
                member_id: id_from_db(&r.member_id)?,
                slot_id: id_from_db(&r.slot_id)?,
                weekday: weekday_from_db(r.weekday)?,
                present: r.present,
            })
        })
        .collect()
}

/// Remplace les présences d'un membre ; seules les absences sont à enregistrer (présent par défaut).
pub async fn replace_presences(
    conn: &mut SqliteConnection,
    member: MemberId,
    rows: &[PresenceRow],
) -> anyhow::Result<()> {
    let id = member.to_string();
    sqlx::query!("DELETE FROM member_presence WHERE member_id = ?", id).execute(&mut *conn).await?;
    for row in rows {
        let slot = row.slot_id.to_string();
        let weekday = i64::from(row.weekday);
        sqlx::query!(
            "INSERT INTO member_presence (member_id, slot_id, weekday, present) VALUES (?, ?, ?, ?)",
            id,
            slot,
            weekday,
            row.present
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PreferenceRow {
    pub member_id: MemberId,
    pub target: Target,
    pub level: PreferenceLevel,
}

pub async fn preferences(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<PreferenceRow>> {
    let rows = sqlx::query!("SELECT member_id, food_id, group_id, recipe_id, level FROM preference ORDER BY rowid")
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok(PreferenceRow {
                member_id: id_from_db(&r.member_id)?,
                target: target_from(r.food_id.as_deref(), r.group_id.as_deref(), r.recipe_id.as_deref())?,
                level: enum_from_db(&r.level)?,
            })
        })
        .collect()
}

pub async fn replace_preferences(
    conn: &mut SqliteConnection,
    member: MemberId,
    rows: &[PreferenceRow],
) -> anyhow::Result<()> {
    let id = member.to_string();
    sqlx::query!("DELETE FROM preference WHERE member_id = ?", id).execute(&mut *conn).await?;
    for row in rows {
        let key = RowId::new().to_string();
        let (food, group, recipe) = target_columns(&row.target);
        let level = enum_to_db(&row.level);
        sqlx::query!(
            "INSERT INTO preference (id, member_id, food_id, group_id, recipe_id, level) VALUES (?, ?, ?, ?, ?, ?)",
            key,
            id,
            food,
            group,
            recipe,
            level
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

// ===================================================================== Semaine type

#[derive(Debug, Clone)]
pub struct ScheduleRow {
    pub slot_id: MealSlotId,
    pub weekday: u32,
    pub available_active_minutes: u32,
    pub earliest_start: NaiveTime,
    pub ready_by: NaiveTime,
    pub out_of_home: bool,
    pub habitual_meal_id: Option<HabitualMealId>,
}

pub async fn schedules(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<ScheduleRow>> {
    let rows = sqlx::query!(
        r#"SELECT slot_id, weekday, available_active_minutes, earliest_start, ready_by,
                  out_of_home AS "out_of_home: bool", habitual_meal_id
           FROM slot_schedule ORDER BY weekday"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(ScheduleRow {
                slot_id: id_from_db(&r.slot_id)?,
                weekday: weekday_from_db(r.weekday)?,
                available_active_minutes: minutes_from_db(r.available_active_minutes)?,
                earliest_start: time_from_db(&r.earliest_start)?,
                ready_by: time_from_db(&r.ready_by)?,
                out_of_home: r.out_of_home,
                habitual_meal_id: opt_id_from_db(r.habitual_meal_id.as_deref())?,
            })
        })
        .collect()
}

pub async fn upsert_schedule(db: impl SqliteExecutor<'_>, row: &ScheduleRow) -> anyhow::Result<()> {
    let slot = row.slot_id.to_string();
    let weekday = i64::from(row.weekday);
    let minutes = i64::from(row.available_active_minutes);
    let start = time_to_db(row.earliest_start);
    let ready = time_to_db(row.ready_by);
    let habitual = row.habitual_meal_id.map(|h| h.to_string());
    sqlx::query!(
        "INSERT INTO slot_schedule (slot_id, weekday, available_active_minutes, earliest_start, ready_by, out_of_home,
             habitual_meal_id)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (slot_id, weekday) DO UPDATE SET available_active_minutes = excluded.available_active_minutes,
             earliest_start = excluded.earliest_start, ready_by = excluded.ready_by,
             out_of_home = excluded.out_of_home, habitual_meal_id = excluded.habitual_meal_id",
        slot,
        weekday,
        minutes,
        start,
        ready,
        row.out_of_home,
        habitual
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct RoleRow {
    pub id: SlotRoleId,
    pub slot_id: MealSlotId,
    pub position: i64,
    pub name: String,
    pub required: bool,
    pub recipe_kinds: Vec<RecipeKind>,
    pub nutrition_groups: Vec<NutritionGroup>,
}

pub async fn roles(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RoleRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, slot_id, position, name, required AS "required: bool", recipe_kinds, nutrition_groups
           FROM slot_role ORDER BY position"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(RoleRow {
                id: id_from_db(&r.id)?,
                slot_id: id_from_db(&r.slot_id)?,
                position: r.position,
                name: r.name,
                required: r.required,
                recipe_kinds: serde_json::from_str(&r.recipe_kinds)?,
                nutrition_groups: serde_json::from_str(&r.nutrition_groups)?,
            })
        })
        .collect()
}

/// Remplace la composition type d'un créneau ; les éléments de repas qui occupaient un rôle retiré
/// deviennent des ajouts libres (`ON DELETE SET NULL`).
pub async fn replace_roles(conn: &mut SqliteConnection, slot: MealSlotId, rows: &[RoleRow]) -> anyhow::Result<()> {
    let slot_id = slot.to_string();
    let existing = roles(&mut *conn).await?;
    for old in existing.iter().filter(|r| r.slot_id == slot && !rows.iter().any(|n| n.id == r.id)) {
        let id = old.id.to_string();
        sqlx::query!("DELETE FROM slot_role WHERE id = ?", id).execute(&mut *conn).await?;
    }
    // Positions provisoires négatives, pour réordonner sans heurter l'unicité (créneau, position).
    sqlx::query!("UPDATE slot_role SET position = -position - 1 WHERE slot_id = ?", slot_id)
        .execute(&mut *conn)
        .await?;
    for row in rows {
        let id = row.id.to_string();
        let kinds = serde_json::to_string(&row.recipe_kinds)?;
        let groups = serde_json::to_string(&row.nutrition_groups)?;
        sqlx::query!(
            "INSERT INTO slot_role (id, slot_id, position, name, required, recipe_kinds, nutrition_groups)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET position = excluded.position, name = excluded.name,
                 required = excluded.required, recipe_kinds = excluded.recipe_kinds,
                 nutrition_groups = excluded.nutrition_groups",
            id,
            slot_id,
            row.position,
            row.name,
            row.required,
            kinds,
            groups
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

// ===================================================================== Repas habituels

/// Ce que vise un élément de repas habituel ou un aliment de repas : un aliment ou un produit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stocked {
    Food(FoodId),
    Product(ProductId),
}

#[derive(Debug, Clone)]
pub struct HabitualItemRow {
    pub position: i64,
    /// Un aliment, un produit ou une recette.
    pub stocked: Option<Stocked>,
    pub recipe_id: Option<RecipeId>,
    /// Par équivalent adulte (R-PLA-20).
    pub amount: Amount,
}

#[derive(Debug, Clone)]
pub struct HabitualRow {
    pub id: HabitualMealId,
    pub name: String,
    pub items: Vec<HabitualItemRow>,
}

fn stocked(food: Option<&str>, product: Option<&str>) -> anyhow::Result<Option<Stocked>> {
    Ok(match (food, product) {
        (Some(f), None) => Some(Stocked::Food(id_from_db(f)?)),
        (None, Some(p)) => Some(Stocked::Product(id_from_db(p)?)),
        _ => None,
    })
}

fn stocked_columns(value: Option<Stocked>) -> (Option<String>, Option<String>) {
    match value {
        Some(Stocked::Food(f)) => (Some(f.to_string()), None),
        Some(Stocked::Product(p)) => (None, Some(p.to_string())),
        None => (None, None),
    }
}

pub async fn habitual_meals(conn: &mut SqliteConnection) -> anyhow::Result<Vec<HabitualRow>> {
    let heads = sqlx::query!("SELECT id, name FROM habitual_meal ORDER BY name").fetch_all(&mut *conn).await?;
    let items = sqlx::query!(
        "SELECT habitual_meal_id, position, food_id, product_id, recipe_id, amount FROM habitual_meal_item
         ORDER BY habitual_meal_id, position"
    )
    .fetch_all(&mut *conn)
    .await?;
    heads
        .into_iter()
        .map(|h| {
            Ok(HabitualRow {
                id: id_from_db(&h.id)?,
                items: items
                    .iter()
                    .filter(|i| i.habitual_meal_id == h.id)
                    .map(|i| {
                        Ok(HabitualItemRow {
                            position: i.position,
                            stocked: stocked(i.food_id.as_deref(), i.product_id.as_deref())?,
                            recipe_id: opt_id_from_db(i.recipe_id.as_deref())?,
                            amount: Amount::from_raw(i.amount),
                        })
                    })
                    .collect::<anyhow::Result<_>>()?,
                name: h.name,
            })
        })
        .collect()
}

pub async fn habitual_name_taken(
    db: impl SqliteExecutor<'_>,
    name: &str,
    except: Option<HabitualMealId>,
) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string());
    Ok(sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!: i64" FROM habitual_meal WHERE lower(name) = lower(?1) AND (?2 IS NULL OR id <> ?2)"#,
        name,
        except
    )
    .fetch_one(db)
    .await?
        > 0)
}

pub async fn save_habitual(conn: &mut SqliteConnection, row: &HabitualRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO habitual_meal (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
        id,
        row.name,
        now,
        now
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("DELETE FROM habitual_meal_item WHERE habitual_meal_id = ?", id).execute(&mut *conn).await?;
    for item in &row.items {
        let key = RowId::new().to_string();
        let (food, product) = stocked_columns(item.stocked);
        let recipe = item.recipe_id.map(|r| r.to_string());
        let amount = item.amount.raw();
        sqlx::query!(
            "INSERT INTO habitual_meal_item (id, habitual_meal_id, position, food_id, product_id, recipe_id, amount)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            key,
            id,
            item.position,
            food,
            product,
            recipe,
            amount
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn delete_habitual(db: impl SqliteExecutor<'_>, id: HabitualMealId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM habitual_meal WHERE id = ?", id).execute(db).await?;
    Ok(())
}

// ===================================================================== Repas

#[derive(Debug, Clone)]
pub struct MealRow {
    pub id: MealId,
    pub date: Date,
    pub slot_id: MealSlotId,
    pub status: MealStatus,
    pub available_active_minutes: u32,
    pub earliest_start: NaiveTime,
    pub ready_by: NaiveTime,
    pub locked: bool,
    pub validation_batch_id: Option<BatchId>,
    pub note: Option<String>,
}

macro_rules! meal_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(MealRow {
            id: id_from_db(&r.id)?,
            date: date_from_db(&r.date)?,
            slot_id: id_from_db(&r.slot_id)?,
            status: enum_from_db(&r.status)?,
            available_active_minutes: minutes_from_db(r.available_active_minutes)?,
            earliest_start: time_from_db(&r.earliest_start)?,
            ready_by: time_from_db(&r.ready_by)?,
            locked: r.locked,
            validation_batch_id: opt_id_from_db(r.validation_batch_id.as_deref())?,
            note: r.note,
        })
    }};
}

/// Repas d'une période, bornes comprises.
pub async fn meals(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<MealRow>> {
    let (from, to) = (from.to_string(), to.to_string());
    let rows = sqlx::query!(
        r#"SELECT id, date, slot_id, status, available_active_minutes, earliest_start, ready_by,
                  locked AS "locked: bool", validation_batch_id, note
           FROM meal WHERE date BETWEEN ? AND ? ORDER BY date"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| meal_row!(r)).collect()
}

pub async fn find_meal(db: impl SqliteExecutor<'_>, id: MealId) -> anyhow::Result<Option<MealRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, date, slot_id, status, available_active_minutes, earliest_start, ready_by,
                  locked AS "locked: bool", validation_batch_id, note
           FROM meal WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| meal_row!(r)).transpose()
}

pub async fn meal_at(db: impl SqliteExecutor<'_>, date: Date, slot: MealSlotId) -> anyhow::Result<Option<MealId>> {
    let (date, slot) = (date.to_string(), slot.to_string());
    let id = sqlx::query_scalar!("SELECT id FROM meal WHERE date = ? AND slot_id = ?", date, slot)
        .fetch_optional(db)
        .await?;
    id.map(|i| id_from_db(&i)).transpose()
}

pub async fn upsert_meal(db: impl SqliteExecutor<'_>, meal: &MealRow) -> anyhow::Result<()> {
    let id = meal.id.to_string();
    let date = meal.date.to_string();
    let slot = meal.slot_id.to_string();
    let status = enum_to_db(&meal.status);
    let minutes = i64::from(meal.available_active_minutes);
    let start = time_to_db(meal.earliest_start);
    let ready = time_to_db(meal.ready_by);
    let batch = meal.validation_batch_id.map(|b| b.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO meal (id, date, slot_id, status, available_active_minutes, earliest_start, ready_by, locked,
             validation_batch_id, note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET status = excluded.status,
             available_active_minutes = excluded.available_active_minutes, earliest_start = excluded.earliest_start,
             ready_by = excluded.ready_by, locked = excluded.locked, validation_batch_id = excluded.validation_batch_id,
             note = excluded.note, updated_at = excluded.updated_at",
        id,
        date,
        slot,
        status,
        minutes,
        start,
        ready,
        meal.locked,
        batch,
        meal.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_meal(db: impl SqliteExecutor<'_>, id: MealId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM meal WHERE id = ?", id).execute(db).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DinerRow {
    pub meal_id: MealId,
    pub member_id: Option<MemberId>,
    pub guest_label: Option<String>,
    pub adult_equivalent: Ratio,
}

pub async fn diners(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<DinerRow>> {
    let (from, to) = (from.to_string(), to.to_string());
    let rows = sqlx::query!(
        "SELECT d.meal_id, d.member_id, d.guest_label, d.adult_equivalent
         FROM meal_diner d JOIN meal m ON m.id = d.meal_id WHERE m.date BETWEEN ? AND ? ORDER BY d.rowid",
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DinerRow {
                meal_id: id_from_db(&r.meal_id)?,
                member_id: opt_id_from_db(r.member_id.as_deref())?,
                guest_label: r.guest_label,
                adult_equivalent: Ratio::from_thousandths(r.adult_equivalent),
            })
        })
        .collect()
}

pub async fn replace_diners(conn: &mut SqliteConnection, meal: MealId, rows: &[DinerRow]) -> anyhow::Result<()> {
    let meal_id = meal.to_string();
    sqlx::query!("DELETE FROM meal_diner WHERE meal_id = ?", meal_id).execute(&mut *conn).await?;
    for row in rows {
        let key = RowId::new().to_string();
        let member = row.member_id.map(|m| m.to_string());
        let coefficient = row.adult_equivalent.thousandths();
        sqlx::query!(
            "INSERT INTO meal_diner (id, meal_id, member_id, guest_label, adult_equivalent) VALUES (?, ?, ?, ?, ?)",
            key,
            meal_id,
            member,
            row.guest_label,
            coefficient
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ItemRow {
    pub id: MealItemId,
    pub meal_id: MealId,
    pub role_id: Option<SlotRoleId>,
    pub position: i64,
    pub source: ItemSource,
    pub recipe_id: Option<RecipeId>,
    pub cooked_portions: Option<Amount>,
    pub lot_id: Option<LotId>,
    pub leftover_of_item_id: Option<MealItemId>,
    pub stocked: Option<Stocked>,
    pub amount: Option<Amount>,
    pub per_adult_equivalent: bool,
    pub eaten_ratio: Option<Ratio>,
    pub leftover_lot_id: Option<LotId>,
}

macro_rules! item_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(ItemRow {
            id: id_from_db(&r.id)?,
            meal_id: id_from_db(&r.meal_id)?,
            role_id: opt_id_from_db(r.role_id.as_deref())?,
            position: r.position,
            source: enum_from_db(&r.source)?,
            recipe_id: opt_id_from_db(r.recipe_id.as_deref())?,
            cooked_portions: r.cooked_portions.map(Amount::from_raw),
            lot_id: opt_id_from_db(r.lot_id.as_deref())?,
            leftover_of_item_id: opt_id_from_db(r.leftover_of_item_id.as_deref())?,
            stocked: stocked(r.food_id.as_deref(), r.product_id.as_deref())?,
            amount: r.amount.map(Amount::from_raw),
            per_adult_equivalent: r.per_adult_equivalent,
            eaten_ratio: r.eaten_ratio.map(Ratio::from_thousandths),
            leftover_lot_id: opt_id_from_db(r.leftover_lot_id.as_deref())?,
        })
    }};
}

/// Éléments des repas d'une période, et ceux d'ailleurs qui mangent leurs restes prévus.
pub async fn items(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<ItemRow>> {
    let (from, to) = (from.to_string(), to.to_string());
    let rows = sqlx::query!(
        r#"SELECT i.id, i.meal_id, i.role_id, i.position, i.source, i.recipe_id, i.cooked_portions, i.lot_id,
                  i.leftover_of_item_id, i.food_id, i.product_id, i.amount,
                  i.per_adult_equivalent AS "per_adult_equivalent: bool", i.eaten_ratio, i.leftover_lot_id
           FROM meal_item i JOIN meal m ON m.id = i.meal_id
           WHERE m.date BETWEEN ?1 AND ?2
              OR i.leftover_of_item_id IN (SELECT x.id FROM meal_item x JOIN meal y ON y.id = x.meal_id
                                           WHERE y.date BETWEEN ?1 AND ?2)
              OR i.id IN (SELECT x.leftover_of_item_id FROM meal_item x JOIN meal y ON y.id = x.meal_id
                          WHERE y.date BETWEEN ?1 AND ?2)
           ORDER BY i.meal_id, i.position"#,
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| item_row!(r)).collect()
}

pub async fn meal_items(db: impl SqliteExecutor<'_>, meal: MealId) -> anyhow::Result<Vec<ItemRow>> {
    let meal = meal.to_string();
    let rows = sqlx::query!(
        r#"SELECT id, meal_id, role_id, position, source, recipe_id, cooked_portions, lot_id, leftover_of_item_id,
                  food_id, product_id, amount, per_adult_equivalent AS "per_adult_equivalent: bool", eaten_ratio,
                  leftover_lot_id
           FROM meal_item WHERE meal_id = ? ORDER BY position"#,
        meal
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| item_row!(r)).collect()
}

pub async fn find_item(db: impl SqliteExecutor<'_>, id: MealItemId) -> anyhow::Result<Option<ItemRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, meal_id, role_id, position, source, recipe_id, cooked_portions, lot_id, leftover_of_item_id,
                  food_id, product_id, amount, per_adult_equivalent AS "per_adult_equivalent: bool", eaten_ratio,
                  leftover_lot_id
           FROM meal_item WHERE id = ?"#,
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| item_row!(r)).transpose()
}

/// Remplace les éléments d'un repas en gardant les identifiants connus : un repas ultérieur peut viser le
/// reste prévu d'un élément (`leftover_of_item_id`).
pub async fn replace_items(conn: &mut SqliteConnection, meal: MealId, rows: &[ItemRow]) -> anyhow::Result<()> {
    let existing = meal_items(&mut *conn, meal).await?;
    for old in existing.iter().filter(|o| !rows.iter().any(|n| n.id == o.id)) {
        let id = old.id.to_string();
        sqlx::query!("DELETE FROM meal_item WHERE id = ?", id).execute(&mut *conn).await?;
    }
    let meal_id = meal.to_string();
    // Positions provisoires négatives, pour réordonner sans heurter l'unicité (repas, position).
    sqlx::query!("UPDATE meal_item SET position = -position - 1 WHERE meal_id = ?", meal_id)
        .execute(&mut *conn)
        .await?;
    for row in rows {
        save_item(&mut *conn, row).await?;
    }
    Ok(())
}

pub async fn save_item(db: impl SqliteExecutor<'_>, row: &ItemRow) -> anyhow::Result<()> {
    let id = row.id.to_string();
    let meal = row.meal_id.to_string();
    let role = row.role_id.map(|r| r.to_string());
    let source = enum_to_db(&row.source);
    let recipe = row.recipe_id.map(|r| r.to_string());
    let cooked = row.cooked_portions.map(Amount::raw);
    let lot = row.lot_id.map(|l| l.to_string());
    let leftover_of = row.leftover_of_item_id.map(|i| i.to_string());
    let (food, product) = stocked_columns(row.stocked);
    let amount = row.amount.map(Amount::raw);
    let eaten = row.eaten_ratio.map(Ratio::thousandths);
    let leftover_lot = row.leftover_lot_id.map(|l| l.to_string());
    sqlx::query!(
        "INSERT INTO meal_item (id, meal_id, role_id, position, source, recipe_id, cooked_portions, lot_id,
             leftover_of_item_id, food_id, product_id, amount, per_adult_equivalent, eaten_ratio, leftover_lot_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET role_id = excluded.role_id, position = excluded.position,
             source = excluded.source, recipe_id = excluded.recipe_id, cooked_portions = excluded.cooked_portions,
             lot_id = excluded.lot_id, leftover_of_item_id = excluded.leftover_of_item_id,
             food_id = excluded.food_id, product_id = excluded.product_id, amount = excluded.amount,
             per_adult_equivalent = excluded.per_adult_equivalent, eaten_ratio = excluded.eaten_ratio,
             leftover_lot_id = excluded.leftover_lot_id",
        id,
        meal,
        role,
        row.position,
        source,
        recipe,
        cooked,
        lot,
        leftover_of,
        food,
        product,
        amount,
        row.per_adult_equivalent,
        eaten,
        leftover_lot
    )
    .execute(db)
    .await?;
    Ok(())
}

// ===================================================================== Consommations réelles

#[derive(Debug, Clone)]
pub struct UseRow {
    pub meal_item_id: MealItemId,
    /// Un aliment, un produit, ou une recette (part d'un lot de préparation).
    pub stocked: Option<Stocked>,
    pub recipe_id: Option<RecipeId>,
    pub amount: Amount,
    pub measure: Measure,
    pub movement_id: Option<MovementId>,
}

pub async fn insert_use(db: impl SqliteExecutor<'_>, row: &UseRow) -> anyhow::Result<()> {
    let id = RowId::new().to_string();
    let item = row.meal_item_id.to_string();
    let (food, product) = stocked_columns(row.stocked);
    let recipe = row.recipe_id.map(|r| r.to_string());
    let amount = row.amount.raw();
    let measure = enum_to_db(&row.measure);
    let movement = row.movement_id.map(|m| m.to_string());
    sqlx::query!(
        "INSERT INTO meal_ingredient_use (id, meal_item_id, food_id, product_id, recipe_id, amount, measure, movement_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        item,
        food,
        product,
        recipe,
        amount,
        measure,
        movement
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Consommations réelles des repas d'une période.
pub async fn uses(db: impl SqliteExecutor<'_>, from: Date, to: Date) -> anyhow::Result<Vec<UseRow>> {
    let (from, to) = (from.to_string(), to.to_string());
    let rows = sqlx::query!(
        "SELECT u.meal_item_id, u.food_id, u.product_id, u.recipe_id, u.amount, u.measure, u.movement_id
         FROM meal_ingredient_use u JOIN meal_item i ON i.id = u.meal_item_id JOIN meal m ON m.id = i.meal_id
         WHERE m.date BETWEEN ? AND ? ORDER BY u.rowid",
        from,
        to
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(UseRow {
                meal_item_id: id_from_db(&r.meal_item_id)?,
                stocked: stocked(r.food_id.as_deref(), r.product_id.as_deref())?,
                recipe_id: opt_id_from_db(r.recipe_id.as_deref())?,
                amount: Amount::from_raw(r.amount),
                measure: enum_from_db(&r.measure)?,
                movement_id: opt_id_from_db(r.movement_id.as_deref())?,
            })
        })
        .collect()
}

/// Rattache le lot des restes d'un élément cuisiné aux repas qui mangent son reste prévu (ou les en détache).
pub async fn attach_leftover_lot(
    db: impl SqliteExecutor<'_>,
    source: MealItemId,
    lot: Option<LotId>,
) -> anyhow::Result<()> {
    let (source, lot) = (source.to_string(), lot.map(|l| l.to_string()));
    sqlx::query!("UPDATE meal_item SET lot_id = ? WHERE leftover_of_item_id = ?", lot, source).execute(db).await?;
    Ok(())
}

/// Efface les consommations d'un repas et ce que sa validation a écrit (R-PLA-34).
pub async fn clear_validation(conn: &mut SqliteConnection, meal: MealId) -> anyhow::Result<()> {
    let meal = meal.to_string();
    sqlx::query!(
        "DELETE FROM meal_ingredient_use WHERE meal_item_id IN (SELECT id FROM meal_item WHERE meal_id = ?)",
        meal
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!("UPDATE meal_item SET eaten_ratio = NULL, leftover_lot_id = NULL WHERE meal_id = ?", meal)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

//! Recettes, ingrédients, étapes, étiquettes, brouillons d'import et rattachements mémorisés (spec 03).

use foodflow_domain::recipes::{RecipeKind, Target, Yield};
use foodflow_domain::shared::ids::{FoodId, IngredientId, RecipeDraftId, RecipeId, StepId, UnitId};
use foodflow_domain::{Amount, Measure, Ratio};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{enum_from_db, enum_to_db, id_from_db, opt_enum_from_db, opt_enum_to_db, opt_id_from_db};
use super::groups::days;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct RecipeRow {
    pub id: RecipeId,
    pub name: String,
    pub kind: RecipeKind,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub yield_: Yield,
    pub active_minutes: u32,
    pub passive_minutes: u32,
    pub fridge_days: Option<u32>,
    pub freezable: bool,
    pub frozen_days: Option<u32>,
    pub finishing_minutes: Option<u32>,
    pub leftover_food_id: Option<FoodId>,
    pub source_url: Option<String>,
    pub image_path: Option<String>,
    pub archived: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct IngredientRow {
    pub id: IngredientId,
    pub recipe_id: RecipeId,
    pub position: i64,
    pub section: Option<String>,
    pub target: Target,
    pub amount: Option<Amount>,
    pub measure: Option<Measure>,
    pub to_taste: bool,
    pub entered_quantity: Option<Ratio>,
    pub entered_unit_id: Option<UnitId>,
    pub entered_unit_label: Option<String>,
    pub note: Option<String>,
    pub optional: bool,
    pub fixed: bool,
    pub source_line: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StepRow {
    pub id: StepId,
    pub position: i64,
    pub section: Option<String>,
    pub text: String,
    pub timer_minutes: Option<u32>,
}

fn yield_from(kind: &str, portions: Option<i64>, measure: Option<&str>, amount: Option<i64>) -> anyhow::Result<Yield> {
    Ok(match (kind, portions, measure, amount) {
        ("PORTIONS", Some(p), _, _) => Yield::Portions { portions: Amount::from_raw(p) },
        ("AMOUNT", _, Some(m), Some(a)) => Yield::Amount { measure: enum_from_db(m)?, amount: Amount::from_raw(a) },
        _ => anyhow::bail!("rendement incohérent en base"),
    })
}

fn yield_columns(y: &Yield) -> (&'static str, Option<i64>, Option<String>, Option<i64>) {
    match y {
        Yield::Portions { portions } => ("PORTIONS", Some(portions.raw()), None, None),
        Yield::Amount { measure, amount } => ("AMOUNT", None, Some(enum_to_db(measure)), Some(amount.raw())),
    }
}

pub(crate) fn target_from(food: Option<&str>, group: Option<&str>, recipe: Option<&str>) -> anyhow::Result<Target> {
    Ok(match (food, group, recipe) {
        (Some(f), None, None) => Target::Food(id_from_db(f)?),
        (None, Some(g), None) => Target::Group(id_from_db(g)?),
        (None, None, Some(r)) => Target::Recipe(id_from_db(r)?),
        _ => anyhow::bail!("ingrédient sans cible unique en base"),
    })
}

pub(crate) fn target_columns(target: &Target) -> (Option<String>, Option<String>, Option<String>) {
    match target {
        Target::Food(id) => (Some(id.to_string()), None, None),
        Target::Group(id) => (None, Some(id.to_string()), None),
        Target::Recipe(id) => (None, None, Some(id.to_string())),
    }
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RecipeRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, description, notes, yield_kind, yield_portions, yield_measure, yield_amount,
                  active_minutes, passive_minutes, fridge_days, freezable AS "freezable: bool", frozen_days,
                  finishing_minutes, leftover_food_id, source_url, image_path, archived AS "archived: bool", created_at
           FROM recipe ORDER BY name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(RecipeRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                kind: enum_from_db(&r.kind)?,
                description: r.description,
                notes: r.notes,
                yield_: yield_from(&r.yield_kind, r.yield_portions, r.yield_measure.as_deref(), r.yield_amount)?,
                active_minutes: days(Some(r.active_minutes)).unwrap_or(0),
                passive_minutes: days(Some(r.passive_minutes)).unwrap_or(0),
                fridge_days: days(r.fridge_days),
                freezable: r.freezable,
                frozen_days: days(r.frozen_days),
                finishing_minutes: days(r.finishing_minutes),
                leftover_food_id: opt_id_from_db(r.leftover_food_id.as_deref())?,
                source_url: r.source_url,
                image_path: r.image_path,
                archived: r.archived,
                created_at: r.created_at,
            })
        })
        .collect()
}

pub async fn find(db: impl SqliteExecutor<'_>, id: RecipeId) -> anyhow::Result<Option<RecipeRow>> {
    Ok(list(db).await?.into_iter().find(|r| r.id == id))
}

pub async fn ingredients(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<IngredientRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, recipe_id, position, section, food_id, group_id, sub_recipe_id, amount, measure,
                  to_taste AS "to_taste: bool", entered_quantity, entered_unit_id, entered_unit_label, note,
                  optional AS "optional: bool", fixed AS "fixed: bool", source_line
           FROM recipe_ingredient ORDER BY recipe_id, position"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(IngredientRow {
                id: id_from_db(&r.id)?,
                recipe_id: id_from_db(&r.recipe_id)?,
                position: r.position,
                section: r.section,
                target: target_from(r.food_id.as_deref(), r.group_id.as_deref(), r.sub_recipe_id.as_deref())?,
                amount: r.amount.map(Amount::from_raw),
                measure: opt_enum_from_db(r.measure.as_deref())?,
                to_taste: r.to_taste,
                entered_quantity: r.entered_quantity.map(Ratio::from_thousandths),
                entered_unit_id: opt_id_from_db(r.entered_unit_id.as_deref())?,
                entered_unit_label: r.entered_unit_label,
                note: r.note,
                optional: r.optional,
                fixed: r.fixed,
                source_line: r.source_line,
            })
        })
        .collect()
}

pub async fn steps(db: impl SqliteExecutor<'_>, recipe: RecipeId) -> anyhow::Result<Vec<StepRow>> {
    let id = recipe.to_string();
    let rows = sqlx::query!(
        "SELECT id, position, section, text, timer_minutes FROM recipe_step WHERE recipe_id = ? ORDER BY position",
        id
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(StepRow {
                id: id_from_db(&r.id)?,
                position: r.position,
                section: r.section,
                text: r.text,
                timer_minutes: days(r.timer_minutes),
            })
        })
        .collect()
}

/// Toutes les étiquettes, par recette.
pub async fn tags(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(RecipeId, String)>> {
    let rows = sqlx::query!("SELECT recipe_id, tag FROM recipe_tag ORDER BY tag").fetch_all(db).await?;
    rows.into_iter().map(|r| Ok((id_from_db(&r.recipe_id)?, r.tag))).collect()
}

/// Insère ou remplace une recette et tout son contenu (ingrédients, étapes, étiquettes).
pub async fn save(
    conn: &mut SqliteConnection,
    recipe: &RecipeRow,
    ingredients: &[IngredientRow],
    steps: &[StepRow],
    tags: &[String],
    insert: bool,
) -> anyhow::Result<()> {
    let id = recipe.id.to_string();
    let kind = enum_to_db(&recipe.kind);
    let (yield_kind, portions, measure, amount) = yield_columns(&recipe.yield_);
    let leftover = recipe.leftover_food_id.map(|f| f.to_string());
    let now = timestamp();
    if insert {
        sqlx::query!(
            "INSERT INTO recipe (id, name, kind, description, notes, yield_kind, yield_portions, yield_measure,
                 yield_amount, active_minutes, passive_minutes, fridge_days, freezable, frozen_days, finishing_minutes,
                 leftover_food_id, source_url, image_path, archived, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            id,
            recipe.name,
            kind,
            recipe.description,
            recipe.notes,
            yield_kind,
            portions,
            measure,
            amount,
            recipe.active_minutes,
            recipe.passive_minutes,
            recipe.fridge_days,
            recipe.freezable,
            recipe.frozen_days,
            recipe.finishing_minutes,
            leftover,
            recipe.source_url,
            recipe.image_path,
            recipe.archived,
            now,
            now
        )
        .execute(&mut *conn)
        .await?;
    } else {
        sqlx::query!(
            "UPDATE recipe SET name = ?, kind = ?, description = ?, notes = ?, yield_kind = ?, yield_portions = ?,
                 yield_measure = ?, yield_amount = ?, active_minutes = ?, passive_minutes = ?, fridge_days = ?,
                 freezable = ?, frozen_days = ?, finishing_minutes = ?, leftover_food_id = ?, source_url = ?,
                 image_path = ?, archived = ?, updated_at = ?
             WHERE id = ?",
            recipe.name,
            kind,
            recipe.description,
            recipe.notes,
            yield_kind,
            portions,
            measure,
            amount,
            recipe.active_minutes,
            recipe.passive_minutes,
            recipe.fridge_days,
            recipe.freezable,
            recipe.frozen_days,
            recipe.finishing_minutes,
            leftover,
            recipe.source_url,
            recipe.image_path,
            recipe.archived,
            now,
            id
        )
        .execute(&mut *conn)
        .await?;
        sqlx::query!("DELETE FROM recipe_ingredient WHERE recipe_id = ?", id).execute(&mut *conn).await?;
        sqlx::query!("DELETE FROM recipe_step WHERE recipe_id = ?", id).execute(&mut *conn).await?;
        sqlx::query!("DELETE FROM recipe_tag WHERE recipe_id = ?", id).execute(&mut *conn).await?;
    }
    for i in ingredients {
        let ingredient_id = i.id.to_string();
        let (food, group, sub) = target_columns(&i.target);
        let amount = i.amount.map(Amount::raw);
        let measure = opt_enum_to_db(&i.measure);
        let entered = i.entered_quantity.map(Ratio::thousandths);
        let unit = i.entered_unit_id.map(|u| u.to_string());
        sqlx::query!(
            "INSERT INTO recipe_ingredient (id, recipe_id, position, section, food_id, group_id, sub_recipe_id, amount,
                 measure, to_taste, entered_quantity, entered_unit_id, entered_unit_label, note, optional, fixed,
                 source_line)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            ingredient_id,
            id,
            i.position,
            i.section,
            food,
            group,
            sub,
            amount,
            measure,
            i.to_taste,
            entered,
            unit,
            i.entered_unit_label,
            i.note,
            i.optional,
            i.fixed,
            i.source_line
        )
        .execute(&mut *conn)
        .await?;
    }
    for s in steps {
        let step_id = s.id.to_string();
        sqlx::query!(
            "INSERT INTO recipe_step (id, recipe_id, position, section, text, timer_minutes) VALUES (?, ?, ?, ?, ?, ?)",
            step_id,
            id,
            s.position,
            s.section,
            s.text,
            s.timer_minutes
        )
        .execute(&mut *conn)
        .await?;
    }
    for tag in tags {
        sqlx::query!("INSERT OR IGNORE INTO recipe_tag (recipe_id, tag) VALUES (?, ?)", id, tag)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

pub async fn set_archived(db: impl SqliteExecutor<'_>, id: RecipeId, archived: bool) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE recipe SET archived = ?, updated_at = ? WHERE id = ?", archived, now, id).execute(db).await?;
    Ok(())
}

pub async fn set_image(db: impl SqliteExecutor<'_>, id: RecipeId, path: Option<&str>) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE recipe SET image_path = ?, updated_at = ? WHERE id = ?", path, now, id).execute(db).await?;
    Ok(())
}

/// Recettes qui utilisent `id` comme sous-recette.
pub async fn used_as_sub_recipe(db: impl SqliteExecutor<'_>, id: RecipeId) -> anyhow::Result<i64> {
    let id = id.to_string();
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM recipe_ingredient WHERE sub_recipe_id = ?"#, id)
        .fetch_one(db)
        .await?)
}

pub async fn delete(db: impl SqliteExecutor<'_>, id: RecipeId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM recipe WHERE id = ?", id).execute(db).await?;
    Ok(())
}

// ===================================================================== Brouillons d'import

#[derive(Debug, Clone)]
pub struct DraftRow {
    pub id: RecipeDraftId,
    pub source_url: Option<String>,
    pub fetched_at: String,
    pub payload: String,
    pub raw_json_ld: String,
    pub image_path: Option<String>,
}

pub async fn insert_draft(db: impl SqliteExecutor<'_>, d: &DraftRow) -> anyhow::Result<()> {
    let id = d.id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO recipe_draft (id, source_url, fetched_at, payload, raw_json_ld, image_path, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        d.source_url,
        d.fetched_at,
        d.payload,
        d.raw_json_ld,
        d.image_path,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn drafts(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<DraftRow>> {
    let rows = sqlx::query!(
        "SELECT id, source_url, fetched_at, payload, raw_json_ld, image_path FROM recipe_draft ORDER BY created_at DESC"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DraftRow {
                id: id_from_db(&r.id)?,
                source_url: r.source_url,
                fetched_at: r.fetched_at,
                payload: r.payload,
                raw_json_ld: r.raw_json_ld,
                image_path: r.image_path,
            })
        })
        .collect()
}

pub async fn update_draft(db: impl SqliteExecutor<'_>, id: RecipeDraftId, payload: &str) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE recipe_draft SET payload = ?, updated_at = ? WHERE id = ?", payload, now, id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn set_draft_image(db: impl SqliteExecutor<'_>, id: RecipeDraftId, path: Option<&str>) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!("UPDATE recipe_draft SET image_path = ?, updated_at = ? WHERE id = ?", path, now, id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn delete_draft(db: impl SqliteExecutor<'_>, id: RecipeDraftId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM recipe_draft WHERE id = ?", id).execute(db).await?;
    Ok(())
}

// ===================================================================== Rattachements mémorisés (R-REC-66)

pub async fn mappings(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<(String, Target)>> {
    let rows = sqlx::query!("SELECT normalized_name, food_id, group_id, recipe_id FROM ingredient_mapping")
        .fetch_all(db)
        .await?;
    rows.into_iter()
        .map(|r| {
            Ok((r.normalized_name, target_from(r.food_id.as_deref(), r.group_id.as_deref(), r.recipe_id.as_deref())?))
        })
        .collect()
}

pub async fn remember(db: impl SqliteExecutor<'_>, key: &str, target: &Target) -> anyhow::Result<()> {
    let (food, group, recipe) = target_columns(target);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO ingredient_mapping (normalized_name, food_id, group_id, recipe_id, updated_at) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (normalized_name) DO UPDATE SET food_id = excluded.food_id, group_id = excluded.group_id,
             recipe_id = excluded.recipe_id, updated_at = excluded.updated_at",
        key,
        food,
        group,
        recipe,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

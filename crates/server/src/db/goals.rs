//! Objectifs nutritionnels, modèles et créneaux de repas (spec 04).

use foodflow_domain::foods::NutritionGroup;
use foodflow_domain::nutrition::Criterion;
use foodflow_domain::shared::ids::{CriterionId, GoalTemplateId, MealSlotId, NutritionGoalId};
use foodflow_domain::{Amount, Date, Ratio};
use sqlx::{SqliteConnection, SqliteExecutor};

use super::codec::{date_from_db, enum_from_db, enum_to_db, id_from_db, opt_id_from_db};
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct GoalRow {
    pub id: NutritionGoalId,
    pub valid_from: Date,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TemplateRow {
    pub id: GoalTemplateId,
    pub name: String,
    pub builtin: bool,
}

/// À qui appartient un critère : une version d'objectif ou un modèle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Goal(NutritionGoalId),
    Template(GoalTemplateId),
}

#[derive(Debug, Clone)]
pub struct CriterionRow {
    pub owner: Owner,
    pub criterion: Criterion,
}

pub async fn goals(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<GoalRow>> {
    let rows =
        sqlx::query!("SELECT id, valid_from, note FROM nutrition_goal ORDER BY valid_from").fetch_all(db).await?;
    rows.into_iter()
        .map(|r| Ok(GoalRow { id: id_from_db(&r.id)?, valid_from: date_from_db(&r.valid_from)?, note: r.note }))
        .collect()
}

pub async fn insert_goal(db: impl SqliteExecutor<'_>, goal: &GoalRow) -> anyhow::Result<()> {
    let id = goal.id.to_string();
    let from = goal.valid_from.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO nutrition_goal (id, valid_from, note, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        id,
        from,
        goal.note,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update_goal(db: impl SqliteExecutor<'_>, goal: &GoalRow) -> anyhow::Result<()> {
    let id = goal.id.to_string();
    let from = goal.valid_from.to_string();
    let now = timestamp();
    sqlx::query!(
        "UPDATE nutrition_goal SET valid_from = ?, note = ?, updated_at = ? WHERE id = ?",
        from,
        goal.note,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_goal(db: impl SqliteExecutor<'_>, id: NutritionGoalId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM nutrition_goal WHERE id = ?", id).execute(db).await?;
    Ok(())
}

pub async fn templates(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<TemplateRow>> {
    let rows =
        sqlx::query!(r#"SELECT id, name, builtin AS "builtin: bool" FROM goal_template ORDER BY builtin DESC, name"#)
            .fetch_all(db)
            .await?;
    rows.into_iter().map(|r| Ok(TemplateRow { id: id_from_db(&r.id)?, name: r.name, builtin: r.builtin })).collect()
}

pub async fn insert_template(db: impl SqliteExecutor<'_>, t: &TemplateRow) -> anyhow::Result<()> {
    let id = t.id.to_string();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO goal_template (id, name, builtin, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        id,
        t.name,
        t.builtin,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete_template(db: impl SqliteExecutor<'_>, id: GoalTemplateId) -> anyhow::Result<()> {
    let id = id.to_string();
    sqlx::query!("DELETE FROM goal_template WHERE id = ? AND builtin = 0", id).execute(db).await?;
    Ok(())
}

pub async fn criteria(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<CriterionRow>> {
    let rows = sqlx::query!(
        "SELECT id, goal_id, template_id, kind, nutrient_code, nutrition_groups, food_group_id, expression, period,
                min_value, max_value, tolerance, occurrence_threshold, importance
         FROM goal_criterion ORDER BY position"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            let owner = match (r.goal_id.as_deref(), r.template_id.as_deref()) {
                (Some(g), None) => Owner::Goal(id_from_db(g)?),
                (None, Some(t)) => Owner::Template(id_from_db(t)?),
                _ => anyhow::bail!("critère sans propriétaire unique en base"),
            };
            let groups: Vec<NutritionGroup> = serde_json::from_str(&r.nutrition_groups)?;
            Ok(CriterionRow {
                owner,
                criterion: Criterion {
                    kind: enum_from_db(&r.kind)?,
                    nutrient_code: r.nutrient_code,
                    nutrition_groups: groups,
                    food_group_id: opt_id_from_db(r.food_group_id.as_deref())?,
                    expression: enum_from_db(&r.expression)?,
                    period: enum_from_db(&r.period)?,
                    min_value: r.min_value,
                    max_value: r.max_value,
                    tolerance: r.tolerance.map(Ratio::from_thousandths),
                    occurrence_threshold: r.occurrence_threshold.map(Amount::from_raw),
                    importance: enum_from_db(&r.importance)?,
                },
            })
        })
        .collect()
}

/// Remplace tous les critères d'une version ou d'un modèle.
pub async fn replace_criteria(conn: &mut SqliteConnection, owner: Owner, criteria: &[Criterion]) -> anyhow::Result<()> {
    let (goal, template) = match owner {
        Owner::Goal(g) => (Some(g.to_string()), None),
        Owner::Template(t) => (None, Some(t.to_string())),
    };
    sqlx::query!("DELETE FROM goal_criterion WHERE goal_id IS ? AND template_id IS ?", goal, template)
        .execute(&mut *conn)
        .await?;
    for (position, c) in criteria.iter().enumerate() {
        let id = CriterionId::new().to_string();
        let kind = enum_to_db(&c.kind);
        let groups = serde_json::to_string(&c.nutrition_groups)?;
        let food_group = c.food_group_id.map(|g| g.to_string());
        let expression = enum_to_db(&c.expression);
        let period = enum_to_db(&c.period);
        let tolerance = c.tolerance.map(Ratio::thousandths);
        let threshold = c.occurrence_threshold.map(Amount::raw);
        let importance = enum_to_db(&c.importance);
        let position = i64::try_from(position)?;
        sqlx::query!(
            "INSERT INTO goal_criterion (id, goal_id, template_id, kind, nutrient_code, nutrition_groups, food_group_id,
                 expression, period, min_value, max_value, tolerance, occurrence_threshold, importance, position)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            id,
            goal,
            template,
            kind,
            c.nutrient_code,
            groups,
            food_group,
            expression,
            period,
            c.min_value,
            c.max_value,
            tolerance,
            threshold,
            importance,
            position
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SlotRow {
    pub id: MealSlotId,
    pub name: String,
    pub position: i64,
    pub energy_share: Ratio,
    pub active: bool,
}

pub async fn slots(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<SlotRow>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, position, energy_share, active AS "active: bool" FROM meal_slot ORDER BY position"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(SlotRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                position: r.position,
                energy_share: Ratio::from_thousandths(r.energy_share),
                active: r.active,
            })
        })
        .collect()
}

pub async fn upsert_slot(db: impl SqliteExecutor<'_>, slot: &SlotRow) -> anyhow::Result<()> {
    let id = slot.id.to_string();
    let share = slot.energy_share.thousandths();
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO meal_slot (id, name, position, energy_share, active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET name = excluded.name, position = excluded.position,
             energy_share = excluded.energy_share, active = excluded.active, updated_at = excluded.updated_at",
        id,
        slot.name,
        slot.position,
        share,
        slot.active,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

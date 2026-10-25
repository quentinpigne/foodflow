//! Objectifs nutritionnels : versions, modèles, créneaux de repas (spec 04, NUT-01, NUT-03).

use foodflow_domain::nutrition::{self, Criterion};
use foodflow_domain::shared::ids::{GoalTemplateId, MealSlotId, NutritionGoalId};
use foodflow_domain::{Date, DomainError, Ratio};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{clean, required};
use crate::db::goals::{GoalRow, Owner, SlotRow, TemplateRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize, ToSchema)]
pub struct GoalVersion {
    pub id: NutritionGoalId,
    pub valid_from: Date,
    pub note: Option<String>,
    /// En vigueur aujourd'hui (R-NUT-2).
    pub current: bool,
    /// Entrée en vigueur : ne se modifie plus (R-NUT-3).
    pub locked: bool,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GoalTemplate {
    pub id: GoalTemplateId,
    pub name: String,
    pub builtin: bool,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewVersionInput {
    pub valid_from: Date,
    pub note: Option<String>,
    /// Critères de départ : un modèle, ou une version existante ; sinon, ceux de la version en vigueur.
    pub template_id: Option<GoalTemplateId>,
    pub copy_from: Option<NutritionGoalId>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VersionInput {
    pub valid_from: Date,
    pub note: Option<String>,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TemplateInput {
    pub name: String,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MealSlot {
    /// Absent pour un nouveau créneau.
    pub id: Option<MealSlotId>,
    pub name: String,
    /// Part de l'énergie de la journée, en millièmes (R-NUT-30).
    pub energy_share: Ratio,
    pub active: bool,
}

fn check_criteria(criteria: &[Criterion]) -> AppResult<()> {
    for c in criteria {
        c.check()?;
    }
    Ok(())
}

pub async fn versions(db: &Db, today: Date) -> AppResult<Vec<GoalVersion>> {
    let goals = db::goals::goals(&db.read).await?;
    let criteria = db::goals::criteria(&db.read).await?;
    let dated: Vec<(Date, NutritionGoalId)> = goals.iter().map(|g| (g.valid_from, g.id)).collect();
    let current = nutrition::in_force(&dated, today).copied();
    Ok(goals
        .into_iter()
        .map(|g| GoalVersion {
            id: g.id,
            valid_from: g.valid_from,
            note: g.note,
            current: Some(g.id) == current,
            locked: nutrition::check_editable(g.valid_from, today).is_err(),
            criteria: criteria.iter().filter(|c| c.owner == Owner::Goal(g.id)).map(|c| c.criterion.clone()).collect(),
        })
        .collect())
}

/// Nouvelle version (R-NUT-3) : à une date, avec les critères d'un modèle, d'une version, ou de celle en vigueur.
pub async fn create_version(db: &Db, input: &NewVersionInput, today: Date) -> AppResult<NutritionGoalId> {
    let mut tx = db.write.begin().await?;
    let goals = db::goals::goals(&mut *tx).await?;
    if goals.iter().any(|g| g.valid_from == input.valid_from) {
        return Err(DomainError::GoalVersionTaken.into());
    }
    let all = db::goals::criteria(&mut *tx).await?;
    let dated: Vec<(Date, NutritionGoalId)> = goals.iter().map(|g| (g.valid_from, g.id)).collect();
    let source = match (input.template_id, input.copy_from) {
        (Some(t), _) => Some(Owner::Template(t)),
        (None, Some(g)) => Some(Owner::Goal(g)),
        (None, None) => nutrition::in_force(&dated, today).map(|g| Owner::Goal(*g)),
    };
    let criteria: Vec<Criterion> = all.into_iter().filter(|c| Some(c.owner) == source).map(|c| c.criterion).collect();
    let id = NutritionGoalId::new();
    db::goals::insert_goal(&mut *tx, &GoalRow { id, valid_from: input.valid_from, note: clean(input.note.as_deref()) })
        .await?;
    db::goals::replace_criteria(&mut tx, Owner::Goal(id), &criteria).await?;
    tx.commit().await?;
    Ok(id)
}

/// Modifie une version à venir ; une version entrée en vigueur ne se modifie plus (R-NUT-3).
pub async fn update_version(db: &Db, id: NutritionGoalId, input: &VersionInput, today: Date) -> AppResult<()> {
    check_criteria(&input.criteria)?;
    let mut tx = db.write.begin().await?;
    let goals = db::goals::goals(&mut *tx).await?;
    let goal = goals.iter().find(|g| g.id == id).ok_or(AppError::NotFound("version d'objectif"))?;
    nutrition::check_editable(goal.valid_from, today)?;
    nutrition::check_editable(input.valid_from, today)?;
    if goals.iter().any(|g| g.id != id && g.valid_from == input.valid_from) {
        return Err(DomainError::GoalVersionTaken.into());
    }
    db::goals::update_goal(&mut *tx, &GoalRow { id, valid_from: input.valid_from, note: clean(input.note.as_deref()) })
        .await?;
    db::goals::replace_criteria(&mut tx, Owner::Goal(id), &input.criteria).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_version(db: &Db, id: NutritionGoalId, today: Date) -> AppResult<()> {
    let goals = db::goals::goals(&db.read).await?;
    let goal = goals.iter().find(|g| g.id == id).ok_or(AppError::NotFound("version d'objectif"))?;
    nutrition::check_editable(goal.valid_from, today)?;
    db::goals::delete_goal(&db.write, id).await?;
    Ok(())
}

pub async fn templates(db: &Db) -> AppResult<Vec<GoalTemplate>> {
    let criteria = db::goals::criteria(&db.read).await?;
    Ok(db::goals::templates(&db.read)
        .await?
        .into_iter()
        .map(|t| GoalTemplate {
            id: t.id,
            criteria: criteria
                .iter()
                .filter(|c| c.owner == Owner::Template(t.id))
                .map(|c| c.criterion.clone())
                .collect(),
            name: t.name,
            builtin: t.builtin,
        })
        .collect())
}

/// Enregistre ses propres critères comme modèle (R-NUT-4).
pub async fn create_template(db: &Db, input: &TemplateInput) -> AppResult<GoalTemplateId> {
    let name = required(&input.name, "name", 120)?;
    check_criteria(&input.criteria)?;
    let mut tx = db.write.begin().await?;
    if db::goals::templates(&mut *tx).await?.iter().any(|t| t.name == name) {
        return Err(AppError::invalid_field("TEMPLATE_NAME_TAKEN", "name", "un modèle de ce nom existe déjà"));
    }
    let row = TemplateRow { id: GoalTemplateId::new(), name, builtin: false };
    db::goals::insert_template(&mut *tx, &row).await?;
    db::goals::replace_criteria(&mut tx, Owner::Template(row.id), &input.criteria).await?;
    tx.commit().await?;
    Ok(row.id)
}

pub async fn delete_template(db: &Db, id: GoalTemplateId) -> AppResult<()> {
    let template = db::goals::templates(&db.read).await?.into_iter().find(|t| t.id == id);
    match template {
        None => Err(AppError::NotFound("modèle")),
        Some(t) if t.builtin => Err(AppError::invalid("TEMPLATE_BUILTIN", "un modèle fourni ne se supprime pas")),
        Some(_) => {
            db::goals::delete_template(&db.write, id).await?;
            Ok(())
        }
    }
}

pub async fn slots(db: &Db) -> AppResult<Vec<MealSlot>> {
    Ok(db::goals::slots(&db.read)
        .await?
        .into_iter()
        .map(|s| MealSlot { id: Some(s.id), name: s.name, energy_share: s.energy_share, active: s.active })
        .collect())
}

/// Enregistre les créneaux, dans l'ordre donné ; les parts des créneaux actifs totalisent 100 % (R-NUT-30).
/// Un créneau absent de la liste est désactivé, pas supprimé (les repas passés y restent rattachés).
pub async fn save_slots(db: &Db, input: &[MealSlot]) -> AppResult<()> {
    let shares: Vec<Ratio> = input.iter().filter(|s| s.active).map(|s| s.energy_share).collect();
    nutrition::check_energy_shares(&shares)?;
    let mut tx = db.write.begin().await?;
    let existing = db::goals::slots(&mut *tx).await?;
    for (position, slot) in input.iter().enumerate() {
        let row = SlotRow {
            id: slot.id.unwrap_or_else(MealSlotId::new),
            name: required(&slot.name, "name", 60)?,
            position: i64::try_from(position).map_err(anyhow::Error::from)?,
            energy_share: slot.energy_share,
            active: slot.active,
        };
        db::goals::upsert_slot(&mut *tx, &row).await?;
    }
    for old in existing.iter().filter(|e| !input.iter().any(|s| s.id == Some(e.id))) {
        db::goals::upsert_slot(&mut *tx, &SlotRow { active: false, ..old.clone() }).await?;
    }
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::seed;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    /// Critère d'acceptation : changer la cible le 15/11 crée une version ; celle du 02/11 reste (R-NUT-3).
    #[tokio::test]
    async fn r_nut_3_versions_and_templates() {
        let (db, _dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        let pnns = templates(&db).await.unwrap().into_iter().find(|t| t.builtin).unwrap();
        let first =
            NewVersionInput { valid_from: d("2026-11-02"), note: None, template_id: Some(pnns.id), copy_from: None };
        let id = create_version(&db, &first, d("2026-11-01")).await.unwrap();
        let list = versions(&db, d("2026-11-15")).await.unwrap();
        assert_eq!(list[0].criteria.len(), 18);
        assert!(list[0].current && list[0].locked);

        // La version en vigueur ne se modifie plus ; on en crée une nouvelle, copiée d'elle.
        let edit = VersionInput { valid_from: d("2026-11-02"), note: None, criteria: list[0].criteria.clone() };
        assert!(matches!(
            update_version(&db, id, &edit, d("2026-11-15")).await,
            Err(AppError::Domain(DomainError::GoalVersionLocked))
        ));
        let next = NewVersionInput {
            valid_from: d("2026-11-16"),
            note: Some("1 900 kcal".into()),
            template_id: None,
            copy_from: None,
        };
        let future = create_version(&db, &next, d("2026-11-15")).await.unwrap();
        let mut criteria = versions(&db, d("2026-11-15")).await.unwrap()[1].criteria.clone();
        criteria[0].min_value = Some(1_900_000);
        criteria[0].max_value = Some(1_900_000);
        update_version(
            &db,
            future,
            &VersionInput { valid_from: d("2026-11-16"), note: None, criteria },
            d("2026-11-15"),
        )
        .await
        .unwrap();
        let after = versions(&db, d("2026-11-20")).await.unwrap();
        assert!(after[1].current);
        assert_eq!(after[0].criteria[0].min_value, Some(2_000_000), "l'ancienne version garde sa cible");
        assert_eq!(after[1].criteria[0].min_value, Some(1_900_000));
        // Deux versions à la même date sont refusées (R-NUT-2).
        let same = NewVersionInput { valid_from: d("2026-11-16"), note: None, template_id: None, copy_from: None };
        assert!(create_version(&db, &same, d("2026-11-15")).await.is_err());
    }

    #[tokio::test]
    async fn r_nut_30_slot_shares() {
        let (db, _dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        let mut list = slots(&db).await.unwrap();
        list[0].energy_share = Ratio::from_thousandths(300);
        assert!(matches!(save_slots(&db, &list).await, Err(AppError::Domain(DomainError::EnergySharesNotFull))));
        list[1].energy_share = Ratio::from_thousandths(300);
        save_slots(&db, &list).await.unwrap();
        assert_eq!(slots(&db).await.unwrap()[0].energy_share, Ratio::from_thousandths(300));
    }
}

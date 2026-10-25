//! Foyer (PLA-01, PLA-06) : membres et leur équivalent adulte, présence par défaut, préférences.

use std::collections::HashSet;

use foodflow_domain::planning::household::PreferenceLevel;
use foodflow_domain::recipes::Target;
use foodflow_domain::shared::ids::{MealSlotId, MemberId};
use foodflow_domain::{DomainError, Ratio};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::recipes::Kitchen;
use super::required;
use crate::db::planning::{MemberRow, PreferenceRow, PresenceRow};
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Un créneau, un jour de la semaine (1 = lundi … 7 = dimanche).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub struct SlotDay {
    pub slot_id: MealSlotId,
    pub weekday: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PreferenceView {
    pub target: Target,
    /// Nom de l'aliment, de la famille ou de la recette (lecture seule).
    #[serde(default)]
    pub target_name: String,
    pub level: PreferenceLevel,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Member {
    pub id: MemberId,
    pub name: String,
    /// Équivalent adulte, en millièmes (1,000 pour un adulte, ADR-0004).
    pub adult_equivalent: Ratio,
    pub active: bool,
    /// Créneaux où le membre est absent par défaut (R-PLA-2).
    pub absences: Vec<SlotDay>,
    pub preferences: Vec<PreferenceView>,
}

pub async fn members(db: &Db) -> AppResult<Vec<Member>> {
    let mut conn = db.read.acquire().await?;
    let rows = db::planning::members(&mut *conn).await?;
    let presences = db::planning::presences(&mut *conn).await?;
    let preferences = db::planning::preferences(&mut *conn).await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    Ok(rows
        .into_iter()
        .map(|m| Member {
            absences: presences
                .iter()
                .filter(|p| p.member_id == m.id && !p.present)
                .map(|p| SlotDay { slot_id: p.slot_id, weekday: p.weekday })
                .collect(),
            preferences: preferences
                .iter()
                .filter(|p| p.member_id == m.id)
                .map(|p| PreferenceView {
                    target: p.target,
                    target_name: kitchen.target_name(&p.target),
                    level: p.level,
                })
                .collect(),
            id: m.id,
            name: m.name,
            adult_equivalent: m.adult_equivalent,
            active: m.active,
        })
        .collect())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MemberInput {
    pub name: String,
    pub adult_equivalent: Ratio,
    #[serde(default = "yes")]
    pub active: bool,
    #[serde(default)]
    pub absences: Vec<SlotDay>,
    #[serde(default)]
    pub preferences: Vec<PreferenceView>,
}

fn yes() -> bool {
    true
}

/// Crée (`id` vide) ou modifie un membre, avec ses absences et ses préférences.
pub async fn save_member(db: &Db, id: Option<MemberId>, input: &MemberInput) -> AppResult<MemberId> {
    let name = required(&input.name, "name", 60)?;
    if !input.adult_equivalent.thousandths().is_positive() {
        return Err(AppError::invalid_field("NOT_POSITIVE", "adult_equivalent", "un coefficient positif est attendu"));
    }
    let mut tx = db.write.begin().await?;
    let existing = db::planning::members(&mut *tx).await?;
    if let Some(id) = id
        && !existing.iter().any(|m| m.id == id)
    {
        return Err(AppError::NotFound("membre"));
    }
    if db::planning::member_name_taken(&mut *tx, &name, id).await? {
        return Err(DomainError::MemberNameTaken.into());
    }
    let id = id.unwrap_or_else(MemberId::new);
    let position = existing.iter().find(|m| m.id == id).map_or(existing.len() as i64, |m| m.position);
    let row = MemberRow { id, name, adult_equivalent: input.adult_equivalent, active: input.active, position };
    db::planning::upsert_member(&mut *tx, &row).await?;
    let mut seen = HashSet::new();
    let absences: Vec<PresenceRow> = input
        .absences
        .iter()
        .filter(|a| (1..=7).contains(&a.weekday) && seen.insert(**a))
        .map(|a| PresenceRow { member_id: id, slot_id: a.slot_id, weekday: a.weekday, present: false })
        .collect();
    db::planning::replace_presences(&mut tx, id, &absences).await?;
    let mut targets = HashSet::new();
    let preferences: Vec<PreferenceRow> = input
        .preferences
        .iter()
        .filter(|p| targets.insert(p.target))
        .map(|p| PreferenceRow { member_id: id, target: p.target, level: p.level })
        .collect();
    db::planning::replace_preferences(&mut tx, id, &preferences).await?;
    tx.commit().await?;
    Ok(id)
}

/// Supprime un membre jamais convive ; un membre qui a des repas est seulement désactivé (il reste dans
/// l'historique, `Member.active`).
pub async fn delete_member(db: &Db, id: MemberId) -> AppResult<()> {
    let mut tx = db.write.begin().await?;
    let member =
        db::planning::members(&mut *tx).await?.into_iter().find(|m| m.id == id).ok_or(AppError::NotFound("membre"))?;
    if db::planning::member_has_meals(&mut *tx, id).await? {
        db::planning::upsert_member(&mut *tx, &MemberRow { active: false, ..member }).await?;
    } else {
        db::planning::delete_member(&mut *tx, id).await?;
    }
    tx.commit().await?;
    Ok(())
}

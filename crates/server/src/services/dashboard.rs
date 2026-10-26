//! Tableau de bord (spec 09, TDB-01) : ce qui demande une action dans le stock et les repas à valider.

use foodflow_domain::Date;
use foodflow_domain::planning::MealStatus;
use foodflow_domain::stock::ExpiryStatus;
use serde::Serialize;
use utoipa::ToSchema;

use super::stock::{self, Lot, LotQuery};
use crate::db::{self, Db};
use crate::error::AppResult;

#[derive(Debug, Serialize, ToSchema)]
pub struct Dashboard {
    /// À consommer en priorité (R-GAS-1) : lots `SOON`, à date dépassée ou à date à compléter, triés par
    /// date limite effective ; et lots à vérifier (R-STK-3), même épuisés, jusqu'à ce qu'un inventaire
    /// confirme l'écart.
    pub priority: Vec<Lot>,
    pub expired_count: usize,
    pub soon_count: usize,
    pub lot_count: usize,
    /// Repas des 30 derniers jours, passés, ni validés, ni sautés, ni hors foyer (R-PLA-35).
    pub meals_to_validate: usize,
    /// Date du plus ancien d'entre eux.
    pub oldest_to_validate: Option<Date>,
}

pub async fn get(db: &Db, today: Date) -> AppResult<Dashboard> {
    let lots = stock::list(db, &LotQuery { location_id: None, food_id: None, include_depleted: true }, today).await?;
    let (active, depleted): (Vec<_>, Vec<_>) = lots.into_iter().partition(|l| !l.depleted);
    let lot_count = active.len();
    let count = |status: &[ExpiryStatus]| active.iter().filter(|l| status.contains(&l.status)).count();
    let expired_count = count(&[ExpiryStatus::Expired, ExpiryStatus::PastBestBefore]);
    let soon_count = count(&[ExpiryStatus::Soon]);
    let priority = active
        .into_iter()
        .filter(|l| l.status != ExpiryStatus::Ok || l.needs_review || l.expiry_missing)
        .chain(depleted.into_iter().filter(|l| l.needs_review))
        .collect();
    let pending: Vec<Date> = db::planning::meals(&db.read, today - chrono::Days::new(30), today)
        .await?
        .into_iter()
        .filter(|m| m.status == MealStatus::Planned && m.date < today)
        .map(|m| m.date)
        .collect();
    Ok(Dashboard {
        priority,
        expired_count,
        soon_count,
        lot_count,
        meals_to_validate: pending.len(),
        oldest_to_validate: pending.iter().min().copied(),
    })
}

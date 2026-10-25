//! Péremption d'un lot : date limite effective et état (R-STK-30 à R-STK-35, R-STK-37).

use chrono::Days;
use serde::{Deserialize, Serialize};

use crate::shared::{Date, DomainError};

use super::{ExpiryKind, ExpiryStatus};

/// Durées de conservation effectives d'un aliment, en jours (R-STK-30), après résolution de l'héritage.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShelfLives {
    /// Sans date de péremption (`NONE`), à partir de l'entrée en stock.
    pub unopened_days: Option<u32>,
    pub opened_days: Option<u32>,
    pub frozen_days: Option<u32>,
    pub thawed_days: Option<u32>,
}

/// Ce qui détermine la date limite effective d'un lot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiryInput {
    pub kind: ExpiryKind,
    pub date: Option<Date>,
    /// Entrée du lot en stock.
    pub stocked_on: Date,
    pub opened_on: Option<Date>,
    /// Date de congélation, si le lot est au congélateur.
    pub frozen_on: Option<Date>,
    /// Date de décongélation, si le lot a été décongelé.
    pub thawed_on: Option<Date>,
    /// Dernier jugement « encore bon » (R-STK-37).
    pub checked_until: Option<Date>,
    pub shelf: ShelfLives,
}

/// Date limite effective. `strict` : dépassée, le lot est `EXPIRED` ; sinon `PAST_BEST_BEFORE` (R-STK-35).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveExpiry {
    pub date: Date,
    pub strict: bool,
}

fn plus(date: Date, days: Option<u32>) -> Option<Date> {
    days.and_then(|d| date.checked_add_days(Days::new(u64::from(d))))
}

fn earliest(a: Option<EffectiveExpiry>, b: Option<EffectiveExpiry>) -> Option<EffectiveExpiry> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if b.date < a.date { b } else { a }),
        (a, b) => a.or(b),
    }
}

impl ExpiryInput {
    pub fn is_frozen(&self) -> bool {
        self.frozen_on.is_some() && self.thawed_on.is_none()
    }

    /// Date limite effective (R-STK-31 à R-STK-33), `None` si le lot n'en a pas.
    pub fn effective(&self) -> Option<EffectiveExpiry> {
        let strict = |date: Option<Date>| date.map(|date| EffectiveExpiry { date, strict: true });
        // Un jugement « encore bon » fixe la nouvelle date limite (R-STK-37).
        if let Some(until) = self.checked_until {
            return strict(Some(until));
        }
        // Au congélateur, seule compte la durée au congélateur (R-STK-32).
        if self.is_frozen() {
            return self.frozen_on.and_then(|frozen| strict(plus(frozen, self.shelf.frozen_days)));
        }
        let base = match (self.thawed_on, self.kind) {
            // Décongelé : date de décongélation + durée après décongélation (R-STK-33).
            (Some(thawed), _) => strict(plus(thawed, self.shelf.thawed_days)),
            (None, ExpiryKind::UseBy) => strict(self.date),
            (None, ExpiryKind::BestBefore) => self.date.map(|date| EffectiveExpiry { date, strict: false }),
            (None, ExpiryKind::None) => strict(plus(self.stocked_on, self.shelf.unopened_days)),
        };
        // Ouvert : la plus proche entre la date et l'ouverture + la durée après ouverture (R-STK-31).
        let opened = self.opened_on.and_then(|opened| strict(plus(opened, self.shelf.opened_days)));
        earliest(base, opened)
    }

    /// Vérifie qu'on peut congeler le lot à `today` (R-STK-32, R-STK-33).
    pub fn check_freeze(&self, today: Date) -> Result<(), DomainError> {
        if self.thawed_on.is_some() {
            return Err(DomainError::RefreezeThawed);
        }
        match self.effective() {
            Some(expiry) if expiry.date < today => Err(DomainError::FreezeAfterExpiry),
            _ => Ok(()),
        }
    }
}

/// État de péremption à `today` (R-STK-35), avec le seuil « bientôt » en jours.
pub fn status(expiry: Option<EffectiveExpiry>, today: Date, soon_days: u32) -> ExpiryStatus {
    let Some(expiry) = expiry else { return ExpiryStatus::Ok };
    if expiry.date < today {
        return if expiry.strict { ExpiryStatus::Expired } else { ExpiryStatus::PastBestBefore };
    }
    let remaining = (expiry.date - today).num_days();
    if remaining < i64::from(soon_days) { ExpiryStatus::Soon } else { ExpiryStatus::Ok }
}

/// Vérifie la nouvelle date limite d'un jugement « encore bon » (R-STK-37).
pub fn check_checked_until(date: Date, until: Date) -> Result<(), DomainError> {
    if until <= date { Err(DomainError::CheckedUntilBeforeDate) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn lot(kind: ExpiryKind, date: Option<&str>) -> ExpiryInput {
        ExpiryInput {
            kind,
            date: date.map(d),
            stocked_on: d("2026-10-20"),
            opened_on: None,
            frozen_on: None,
            thawed_on: None,
            checked_until: None,
            shelf: ShelfLives {
                unopened_days: Some(7),
                opened_days: Some(2),
                frozen_days: Some(90),
                thawed_days: Some(1),
            },
        }
    }

    /// Critère d'acceptation : yaourt de DLC le 30/10 ouvert le 24/10, 2 jours après ouverture → 26/10.
    #[test]
    fn r_stk_31_opening_shortens_expiry() {
        let yogurt = ExpiryInput { opened_on: Some(d("2026-10-24")), ..lot(ExpiryKind::UseBy, Some("2026-10-30")) };
        assert_eq!(yogurt.effective(), Some(EffectiveExpiry { date: d("2026-10-26"), strict: true }));
        let late = ExpiryInput { opened_on: Some(d("2026-10-29")), ..lot(ExpiryKind::UseBy, Some("2026-10-30")) };
        assert_eq!(late.effective().unwrap().date, d("2026-10-30"));
    }

    #[test]
    fn r_stk_30_no_date_uses_default_shelf_life() {
        let apples = lot(ExpiryKind::None, None);
        assert_eq!(apples.effective(), Some(EffectiveExpiry { date: d("2026-10-27"), strict: true }));
        let salt = ExpiryInput { shelf: ShelfLives::default(), ..lot(ExpiryKind::None, None) };
        assert_eq!(salt.effective(), None);
    }

    #[test]
    fn r_stk_32_freezing_replaces_expiry() {
        let frozen = ExpiryInput { frozen_on: Some(d("2026-10-22")), ..lot(ExpiryKind::UseBy, Some("2026-10-24")) };
        assert_eq!(frozen.effective().unwrap().date, d("2027-01-20"));
        let expired = lot(ExpiryKind::UseBy, Some("2026-10-24"));
        assert_eq!(expired.check_freeze(d("2026-10-25")), Err(DomainError::FreezeAfterExpiry));
        assert!(expired.check_freeze(d("2026-10-24")).is_ok());
    }

    /// Critère d'acceptation : un lot décongelé ne se recongèle pas.
    #[test]
    fn r_stk_33_thawed_lot_cannot_be_refrozen() {
        let thawed = ExpiryInput {
            frozen_on: Some(d("2026-10-01")),
            thawed_on: Some(d("2026-10-24")),
            ..lot(ExpiryKind::UseBy, Some("2026-10-05"))
        };
        assert_eq!(thawed.effective().unwrap().date, d("2026-10-25"));
        assert_eq!(thawed.check_freeze(d("2026-10-24")), Err(DomainError::RefreezeThawed));
    }

    #[test]
    fn r_stk_35_statuses() {
        let today = d("2026-10-25");
        let use_by = |date| Some(EffectiveExpiry { date: d(date), strict: true });
        let best_before = |date| Some(EffectiveExpiry { date: d(date), strict: false });
        assert_eq!(status(use_by("2026-10-24"), today, 3), ExpiryStatus::Expired);
        assert_eq!(status(best_before("2026-10-24"), today, 3), ExpiryStatus::PastBestBefore);
        assert_eq!(status(use_by("2026-10-25"), today, 3), ExpiryStatus::Soon, "le jour même");
        assert_eq!(status(use_by("2026-10-27"), today, 3), ExpiryStatus::Soon);
        assert_eq!(status(use_by("2026-10-28"), today, 3), ExpiryStatus::Ok);
        assert_eq!(status(None, today, 3), ExpiryStatus::Ok);
        // Seuil d'un jour pour le frigo : un yaourt dans 2 jours n'est pas `SOON` (spec 07).
        assert_eq!(status(use_by("2026-10-27"), today, 1), ExpiryStatus::Ok);
    }

    #[test]
    fn r_stk_37_checked_lot_gets_new_limit() {
        let checked =
            ExpiryInput { checked_until: Some(d("2026-10-26")), ..lot(ExpiryKind::UseBy, Some("2026-10-24")) };
        assert_eq!(status(checked.effective(), d("2026-10-25"), 3), ExpiryStatus::Soon);
        assert_eq!(status(checked.effective(), d("2026-10-27"), 3), ExpiryStatus::Expired, "la question se repose");
        assert_eq!(check_checked_until(d("2026-10-25"), d("2026-10-25")), Err(DomainError::CheckedUntilBeforeDate));
    }
}

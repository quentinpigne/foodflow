//! Choix des lots d'une sortie : premier périmé, premier sorti (R-STK-36, R-STK-37).

use std::cmp::Ordering;

use crate::shared::Amount;
use crate::shared::ids::LotId;

use super::ExpiryStatus;
use super::expiry::EffectiveExpiry;

/// Lot candidat à une sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub lot: LotId,
    pub available: Amount,
    pub expiry: Option<EffectiveExpiry>,
    pub status: ExpiryStatus,
    pub opened: bool,
}

/// Résultat d'une sortie répartie sur plusieurs lots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub takes: Vec<(LotId, Amount)>,
    /// Lots à date dépassée sans réponse, qu'on ne prend pas d'office (R-STK-37) : la sortie les propose avec
    /// la question « encore utilisable ? ».
    pub to_confirm: Vec<LotId>,
    /// Quantité que les lots utilisables ne couvrent pas.
    pub shortfall: Amount,
}

fn order(a: &Candidate, b: &Candidate) -> Ordering {
    let date = |c: &Candidate| c.expiry.map(|e| e.date);
    // Les lots sans date limite passent après les autres.
    match (date(a), date(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then(b.opened.cmp(&a.opened))
    .then(a.lot.cmp(&b.lot))
}

/// Répartit une sortie sur les lots, la date limite effective la plus proche d'abord, les lots ouverts en
/// premier à date égale (R-STK-36). Les lots `EXPIRED` ou `PAST_BEST_BEFORE` ne sont pas pris d'office.
pub fn allocate(candidates: &[Candidate], requested: Amount) -> Allocation {
    let mut sorted: Vec<&Candidate> = candidates.iter().filter(|c| c.available.is_positive()).collect();
    sorted.sort_by(|a, b| order(a, b));
    let mut remaining = requested;
    let mut takes = Vec::new();
    let mut to_confirm = Vec::new();
    for candidate in sorted {
        if matches!(candidate.status, ExpiryStatus::Expired | ExpiryStatus::PastBestBefore) {
            to_confirm.push(candidate.lot);
            continue;
        }
        if !remaining.is_positive() {
            break;
        }
        let take = remaining.min(candidate.available);
        takes.push((candidate.lot, take));
        remaining -= take;
    }
    Allocation { takes, to_confirm, shortfall: remaining }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lot(available: i64, date: Option<&str>, status: ExpiryStatus, opened: bool) -> Candidate {
        Candidate {
            lot: LotId::new(),
            available: Amount::from_raw(available),
            expiry: date.map(|d| EffectiveExpiry { date: d.parse().unwrap(), strict: true }),
            status,
            opened,
        }
    }

    /// Critère d'acceptation : deux lots de crème (28/10, 05/11), 20 cL pris dans celui du 28/10.
    #[test]
    fn r_stk_36_first_expired_first_out() {
        let late = lot(200_000, Some("2026-11-05"), ExpiryStatus::Ok, false);
        let early = lot(200_000, Some("2026-10-28"), ExpiryStatus::Ok, false);
        let allocation = allocate(&[late, early], Amount::from_raw(200_000));
        assert_eq!(allocation.takes, vec![(early.lot, Amount::from_raw(200_000))]);
        assert!(allocation.shortfall.is_zero());
    }

    #[test]
    fn r_stk_36_spans_lots_opened_first_and_reports_shortfall() {
        let closed = lot(100, Some("2026-10-28"), ExpiryStatus::Ok, false);
        let opened = lot(50, Some("2026-10-28"), ExpiryStatus::Ok, true);
        let undated = lot(30, None, ExpiryStatus::Ok, false);
        let allocation = allocate(&[undated, closed, opened], Amount::from_raw(200));
        assert_eq!(
            allocation.takes,
            vec![
                (opened.lot, Amount::from_raw(50)),
                (closed.lot, Amount::from_raw(100)),
                (undated.lot, Amount::from_raw(30))
            ]
        );
        assert_eq!(allocation.shortfall, Amount::from_raw(20));
    }

    #[test]
    fn r_stk_37_expired_lots_are_not_taken_automatically() {
        let expired = lot(100, Some("2026-10-24"), ExpiryStatus::Expired, false);
        let fresh = lot(100, Some("2026-10-30"), ExpiryStatus::Ok, false);
        let allocation = allocate(&[expired, fresh], Amount::from_raw(150));
        assert_eq!(allocation.takes, vec![(fresh.lot, Amount::from_raw(100))]);
        assert_eq!(allocation.to_confirm, vec![expired.lot]);
        assert_eq!(allocation.shortfall, Amount::from_raw(50));
    }
}

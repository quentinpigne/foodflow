//! Journal d'un lot : quantité ou état calculés à partir des mouvements (ADR-0002, R-STK-2 à R-STK-22).

use std::collections::HashSet;

use crate::foods::TrackingMode;
use crate::shared::ids::MovementId;
use crate::shared::{Amount, Date, DomainError};

use super::{MovementKind, PresenceState};

/// Un mouvement tel que le domaine le voit. Les identifiants UUIDv7 donnent l'ordre de saisie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movement {
    pub id: MovementId,
    pub date: Date,
    pub kind: MovementKind,
    /// Quantité signée (lot en `QUANTITY`) ; quantité constatée pour un inventaire.
    pub amount: Option<Amount>,
    /// État (lot en `PRESENCE`) pour un achat, une correction ou un inventaire.
    pub presence: Option<PresenceState>,
    /// Mouvement annulé par celui-ci (R-STK-11).
    pub reverses: Option<MovementId>,
}

/// Quantité ou état d'un lot, calculé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LotState {
    /// Lot en `QUANTITY` : quantité calculée, jamais négative.
    pub quantity: Option<Amount>,
    /// Lot en `PRESENCE` : dernier état connu.
    pub presence: Option<PresenceState>,
    /// Date du dernier inventaire non annulé (R-STK-14).
    pub last_inventory: Option<Date>,
}

impl LotState {
    /// Quantité nulle ou état `OUT` (R-STK-4).
    pub fn is_depleted(&self) -> bool {
        match (self.quantity, self.presence) {
            (Some(quantity), _) => quantity.raw() <= 0,
            (None, Some(presence)) => presence == PresenceState::Out,
            (None, None) => true,
        }
    }
}

/// Mouvements effectifs, dans l'ordre (date, saisie) : un mouvement annulé et son annulation se
/// neutralisent, et sortent tous deux du calcul (R-STK-11).
fn effective(movements: &[Movement]) -> Vec<&Movement> {
    let reversed: HashSet<MovementId> = movements.iter().filter_map(|m| m.reverses).collect();
    let mut list: Vec<&Movement> =
        movements.iter().filter(|m| m.reverses.is_none() && !reversed.contains(&m.id)).collect();
    list.sort_by_key(|m| (m.date, m.id));
    list
}

/// Calcule l'état d'un lot (R-STK-2) : quantité du dernier inventaire + Σ des mouvements postérieurs.
pub fn state(mode: TrackingMode, movements: &[Movement]) -> LotState {
    let list = effective(movements);
    let inventory_index = list.iter().rposition(|m| m.kind == MovementKind::Inventory);
    let last_inventory = inventory_index.map(|i| list[i].date);
    let start = inventory_index.unwrap_or(0);
    match mode {
        TrackingMode::Quantity => {
            let mut quantity = Amount::ZERO;
            for movement in &list[start..] {
                match (movement.kind, movement.amount) {
                    (MovementKind::Inventory, Some(amount)) => quantity = amount,
                    (_, Some(amount)) => quantity += amount,
                    _ => {}
                }
            }
            LotState { quantity: Some(Amount::from_raw(quantity.raw().max(0))), presence: None, last_inventory }
        }
        TrackingMode::Presence => {
            LotState { quantity: None, presence: list.iter().rev().find_map(|m| m.presence), last_inventory }
        }
    }
}

/// Sortie bornée à la quantité disponible (R-STK-3) : renvoie (quantité sortie, quantité manquante).
pub fn bounded_withdrawal(available: Amount, requested: Amount) -> (Amount, Amount) {
    let taken = requested.min(Amount::from_raw(available.raw().max(0)));
    (taken, requested - taken)
}

/// Vérifie la date d'un nouveau mouvement (R-STK-14).
pub fn check_date(date: Date, today: Date, last_inventory: Option<Date>) -> Result<(), DomainError> {
    if date > today {
        return Err(DomainError::MovementInFuture);
    }
    match last_inventory {
        Some(inventory) if date < inventory => Err(DomainError::MovementBeforeInventory(inventory)),
        _ => Ok(()),
    }
}

/// Vérifie qu'un mouvement peut être annulé (R-STK-11), et renvoie la quantité de l'annulation.
pub fn reversal_of(target: &Movement, movements: &[Movement]) -> Result<Option<Amount>, DomainError> {
    if target.reverses.is_some() {
        return Err(DomainError::ReverseOfReversal);
    }
    if movements.iter().any(|m| m.reverses == Some(target.id)) {
        return Err(DomainError::AlreadyReversed);
    }
    Ok(match target.kind {
        // Un inventaire annulé est simplement ignoré : son annulation n'a pas de quantité.
        MovementKind::Inventory => None,
        _ => target.amount.map(|a| -a),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn mv(date: &str, kind: MovementKind, amount: i64) -> Movement {
        Movement {
            id: MovementId::new(),
            date: d(date),
            kind,
            amount: Some(Amount::from_raw(amount)),
            presence: None,
            reverses: None,
        }
    }

    /// Critère d'acceptation de la spec 02 : 1 L, −250 mL, −200 mL = 550 mL ; inventaire à 500 mL puis
    /// −100 mL = 400 mL.
    #[test]
    fn r_stk_2_quantity_from_journal_and_inventory() {
        let mut journal = vec![
            mv("2026-10-20", MovementKind::Purchase, 1_000_000),
            mv("2026-10-22", MovementKind::Cooked, -250_000),
            mv("2026-10-24", MovementKind::Cooked, -200_000),
        ];
        let state = state(TrackingMode::Quantity, &journal);
        assert_eq!(state.quantity, Some(Amount::from_raw(550_000)));
        journal.push(mv("2026-10-25", MovementKind::Inventory, 500_000));
        journal.push(mv("2026-10-25", MovementKind::Cooked, -100_000));
        let after = super::state(TrackingMode::Quantity, &journal);
        assert_eq!(after.quantity, Some(Amount::from_raw(400_000)));
        assert_eq!(after.last_inventory, Some(d("2026-10-25")));
    }

    #[test]
    fn r_stk_3_withdrawal_is_bounded() {
        let (taken, shortfall) = bounded_withdrawal(Amount::from_raw(200_000), Amount::from_raw(300_000));
        assert_eq!((taken.raw(), shortfall.raw()), (200_000, 100_000));
        let (taken, shortfall) = bounded_withdrawal(Amount::from_raw(500), Amount::from_raw(300));
        assert_eq!((taken.raw(), shortfall.raw()), (300, 0));
    }

    #[test]
    fn r_stk_4_depleted_lots() {
        let journal = vec![mv("2026-10-20", MovementKind::Purchase, 200), mv("2026-10-21", MovementKind::Cooked, -200)];
        assert!(state(TrackingMode::Quantity, &journal).is_depleted());
        let out = Movement {
            presence: Some(PresenceState::Out),
            amount: None,
            ..mv("2026-10-21", MovementKind::Adjustment, 0)
        };
        let presence = vec![
            Movement {
                presence: Some(PresenceState::Available),
                amount: None,
                ..mv("2026-10-20", MovementKind::Purchase, 0)
            },
            out,
        ];
        assert!(state(TrackingMode::Presence, &presence).is_depleted());
    }

    #[test]
    fn r_stk_11_reversal_neutralizes_and_happens_once() {
        let purchase = mv("2026-10-20", MovementKind::Purchase, 1_000);
        let mut journal = vec![mv("2026-10-19", MovementKind::Purchase, 300), purchase.clone()];
        let amount = reversal_of(&purchase, &journal).unwrap();
        assert_eq!(amount, Some(Amount::from_raw(-1_000)));
        let reversal = Movement { amount, reverses: Some(purchase.id), ..mv("2026-10-20", MovementKind::Purchase, 0) };
        journal.push(reversal.clone());
        assert_eq!(state(TrackingMode::Quantity, &journal).quantity, Some(Amount::from_raw(300)));
        assert_eq!(reversal_of(&purchase, &journal), Err(DomainError::AlreadyReversed));
        assert_eq!(reversal_of(&reversal, &journal), Err(DomainError::ReverseOfReversal));
    }

    #[test]
    fn reversed_inventory_is_ignored() {
        let inventory = mv("2026-10-21", MovementKind::Inventory, 300);
        let journal = vec![
            mv("2026-10-20", MovementKind::Purchase, 1_000),
            mv("2026-10-20", MovementKind::Cooked, -400),
            inventory.clone(),
            Movement { amount: None, reverses: Some(inventory.id), ..mv("2026-10-21", MovementKind::Inventory, 0) },
        ];
        let state = state(TrackingMode::Quantity, &journal);
        assert_eq!(state.quantity, Some(Amount::from_raw(600)));
        assert_eq!(state.last_inventory, None);
    }

    #[test]
    fn r_stk_14_movement_dates() {
        let today = d("2026-10-25");
        assert_eq!(check_date(d("2026-10-26"), today, None), Err(DomainError::MovementInFuture));
        assert_eq!(
            check_date(d("2026-10-19"), today, Some(d("2026-10-20"))),
            Err(DomainError::MovementBeforeInventory(d("2026-10-20")))
        );
        assert!(check_date(d("2026-10-20"), today, Some(d("2026-10-20"))).is_ok());
    }

    #[test]
    fn r_stk_22_presence_state_is_last_explicit_state() {
        let journal = vec![
            Movement {
                presence: Some(PresenceState::Available),
                amount: None,
                ..mv("2026-10-20", MovementKind::Purchase, 0)
            },
            Movement { amount: None, ..mv("2026-10-21", MovementKind::Cooked, 0) },
            Movement {
                presence: Some(PresenceState::Low),
                amount: None,
                ..mv("2026-10-22", MovementKind::Adjustment, 0)
            },
        ];
        assert_eq!(state(TrackingMode::Presence, &journal).presence, Some(PresenceState::Low));
    }

    proptest::proptest! {
        /// Annuler tous les mouvements ramène le lot à zéro (architecture.md, tests de propriétés).
        #[test]
        fn reversing_everything_restores_zero(amounts in proptest::collection::vec(0i64..10_000, 1..20)) {
            let mut journal: Vec<Movement> =
                amounts.iter().map(|a| mv("2026-10-20", MovementKind::Adjustment, *a)).collect();
            let originals = journal.clone();
            for original in &originals {
                let amount = reversal_of(original, &journal).unwrap();
                journal.push(Movement { amount, reverses: Some(original.id), ..mv("2026-10-20", MovementKind::Adjustment, 0) });
            }
            proptest::prop_assert_eq!(state(TrackingMode::Quantity, &journal).quantity, Some(Amount::ZERO));
        }
    }
}

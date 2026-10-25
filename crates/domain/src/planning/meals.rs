//! Repas : semaine type, repas habituels, portions cuisinées et restes prévus (R-PLA-1, R-PLA-13, R-PLA-20).

use chrono::NaiveTime;
use rust_decimal::Decimal;

use crate::shared::{Amount, Date, DomainError, Ratio};

/// Durée de conservation d'un plat cuisiné quand sa recette n'en indique pas (R-STK-34).
pub const DEFAULT_FRIDGE_DAYS: u32 = 3;

/// Temps total disponible d'un créneau, en minutes : heure limite − heure de début (R-PLA-1).
pub fn window_minutes(earliest_start: NaiveTime, ready_by: NaiveTime) -> Result<u32, DomainError> {
    if earliest_start >= ready_by {
        return Err(DomainError::InvalidSchedule);
    }
    u32::try_from((ready_by - earliest_start).num_minutes()).map_err(|_| DomainError::InvalidSchedule)
}

/// Quantité d'un élément de repas habituel, définie par équivalent adulte, pour les convives d'un repas
/// (R-PLA-20).
pub fn scale_per_adult(amount: Amount, adult_equivalents: Ratio) -> Result<Amount, DomainError> {
    Amount::round(amount.to_decimal() * adult_equivalents.to_decimal())
}

/// Reste prévu d'une recette cuisinée pour plus que les convives : portions cuisinées − équivalents adulte,
/// en millièmes de portion (R-PLA-13). Jamais négatif.
pub fn planned_leftover(cooked_portions: Amount, adult_equivalents: Ratio) -> Amount {
    Amount::from_raw((cooked_portions.raw() - adult_equivalents.thousandths()).max(0))
}

/// Date limite d'un plat cuisiné ou de ses restes : le jour où il est cuisiné + la conservation au frigo de la
/// recette, à défaut la durée générale (R-STK-34).
pub fn leftover_deadline(cooked_on: Date, fridge_days: Option<u32>) -> Date {
    cooked_on + chrono::Days::new(u64::from(fridge_days.unwrap_or(DEFAULT_FRIDGE_DAYS)))
}

/// Portions mangées d'un élément cuisiné : portions cuisinées − restes (R-NUT-20), jamais négatif.
pub fn eaten_portions(cooked_portions: Amount, leftover: Amount) -> Amount {
    Amount::from_raw((cooked_portions.raw() - leftover.raw()).max(0))
}

/// Part mangée, en millièmes, des portions cuisinées : sert à ramener les quantités utilisées à ce qui a été
/// mangé (R-PLA-31).
pub fn eaten_ratio(cooked_portions: Amount, leftover: Amount) -> Ratio {
    if !cooked_portions.is_positive() {
        return Ratio::ONE;
    }
    let eaten = Decimal::from(eaten_portions(cooked_portions, leftover).raw()) * Decimal::from(1_000)
        / Decimal::from(cooked_portions.raw());
    Ratio::from_thousandths(
        eaten.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero).try_into().unwrap_or(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn t(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    #[test]
    fn r_pla_1_window() {
        assert_eq!(window_minutes(t("18:30"), t("19:30")), Ok(60));
        assert_eq!(window_minutes(t("19:30"), t("18:30")), Err(DomainError::InvalidSchedule));
    }

    /// Critère d'acceptation : « Tartines », 60 g de pain par équivalent adulte, à 3,6 équivalents adulte,
    /// demande 216 g de pain (R-PLA-20).
    #[test]
    fn r_pla_20_habitual_meal_scaled() {
        let bread = Amount::from_raw(60_000);
        assert_eq!(scale_per_adult(bread, Ratio::from_thousandths(3_600)), Ok(Amount::from_raw(216_000)));
    }

    /// Critère d'acceptation : un gratin cuisiné pour 6 portions au dîner de lundi pour 3,6 équivalents adulte
    /// laisse un reste prévu de 2,4 portions (R-PLA-13).
    #[test]
    fn r_pla_13_planned_leftover() {
        let six = Amount::from_raw(6_000);
        assert_eq!(planned_leftover(six, Ratio::from_thousandths(3_600)), Amount::from_raw(2_400));
        assert_eq!(planned_leftover(six, Ratio::from_thousandths(7_000)), Amount::ZERO);
        // Conservation 2 jours : reste à manger au plus tard le mercredi.
        assert_eq!(leftover_deadline(d("2026-11-02"), Some(2)), d("2026-11-04"));
        assert_eq!(leftover_deadline(d("2026-11-02"), None), d("2026-11-05"));
    }

    /// Ce qui est mangé = cuisiné − restes (R-NUT-20, ADR-0012).
    #[test]
    fn r_nut_20_eaten() {
        let six = Amount::from_raw(6_000);
        assert_eq!(eaten_portions(six, Amount::from_raw(1_500)), Amount::from_raw(4_500));
        assert_eq!(eaten_ratio(six, Amount::from_raw(1_500)), Ratio::from_thousandths(750));
        assert_eq!(eaten_ratio(six, Amount::ZERO), Ratio::ONE);
    }
}

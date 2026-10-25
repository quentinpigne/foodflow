//! Conversions entre mesures (R-ALI-30 à R-ALI-33).
//!
//! ```text
//!            densité (mg par mL)
//!   VOLUME ◄─────────────────────► MASS ◄─────────────────────► COUNT
//!                                         poids d'une pièce (mg)
//! ```
//!
//! Tout calcul se fait en décimal exact ; seul le résultat est arrondi à l'unité de stockage.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::shared::{Amount, DomainError, Measure};

/// Données de conversion propres à un aliment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoodConversions {
    /// Masse d'un mL, en mg.
    pub density_mg_per_ml: Option<i64>,
    /// Masse moyenne d'une pièce, en mg.
    pub piece_weight_mg: Option<i64>,
}

/// Quantité saisie dans une unité (2 cuillères à soupe) → quantité stockée dans la mesure de l'unité.
///
/// `unit_amount` est la valeur d'une unité dans sa mesure, en unités de stockage (c. à soupe : 15 000 µL).
pub fn entered_to_amount(quantity: Decimal, unit_amount: Amount) -> Result<Amount, DomainError> {
    Amount::round(quantity * unit_amount.to_decimal())
}

/// Masse exacte (en mg) d'une quantité, ou l'erreur qui dit ce qui manque.
fn to_mass(amount: Decimal, from: Measure, conv: FoodConversions) -> Result<Decimal, DomainError> {
    match from {
        Measure::Mass => Ok(amount),
        // µL → mL (÷ 1 000) × mg par mL.
        Measure::Volume => {
            let density = conv.density_mg_per_ml.ok_or(DomainError::MissingDensity)?;
            Ok(amount * Decimal::from(density) / Decimal::from(1_000))
        }
        // millièmes de pièce → pièces (÷ 1 000) × mg par pièce.
        Measure::Count => {
            let weight = conv.piece_weight_mg.ok_or(DomainError::MissingPieceWeight)?;
            Ok(amount * Decimal::from(weight) / Decimal::from(1_000))
        }
    }
}

fn from_mass(mass_mg: Decimal, to: Measure, conv: FoodConversions) -> Result<Decimal, DomainError> {
    match to {
        Measure::Mass => Ok(mass_mg),
        Measure::Volume => {
            let density = conv.density_mg_per_ml.filter(|d| *d > 0).ok_or(DomainError::MissingDensity)?;
            Ok(mass_mg * Decimal::from(1_000) / Decimal::from(density))
        }
        Measure::Count => {
            let weight = conv.piece_weight_mg.filter(|w| *w > 0).ok_or(DomainError::MissingPieceWeight)?;
            Ok(mass_mg * Decimal::from(1_000) / Decimal::from(weight))
        }
    }
}

/// Valeur exacte d'une quantité dans une autre mesure, sans arrondi (R-ALI-33).
pub fn convert_exact(
    amount: Decimal,
    from: Measure,
    to: Measure,
    conv: FoodConversions,
) -> Result<Decimal, DomainError> {
    if from == to {
        return Ok(amount);
    }
    from_mass(to_mass(amount, from, conv)?, to, conv)
}

/// Convertit une quantité d'une mesure à une autre, arrondie à l'unité de stockage (R-ALI-31, R-ALI-33).
///
/// Ne suppose jamais de densité ni de poids : sans eux, la conversion est impossible (R-ALI-31).
pub fn convert(amount: Amount, from: Measure, to: Measure, conv: FoodConversions) -> Result<Amount, DomainError> {
    Amount::round(convert_exact(amount.to_decimal(), from, to, conv)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MILK: FoodConversions = FoodConversions { density_mg_per_ml: Some(1_030), piece_weight_mg: None };

    /// Critère d'acceptation de la spec 01 : 206 g de lait de densité 1,03 font 200 mL.
    #[test]
    fn r_ali_30_mass_to_volume_with_density() {
        let needed = convert(Amount::from_raw(206_000), Measure::Mass, Measure::Volume, MILK).unwrap();
        assert_eq!(needed, Amount::from_raw(200_000));
    }

    #[test]
    fn r_ali_31_conversion_without_density_is_impossible() {
        let flour = FoodConversions::default();
        let two_glasses = Amount::from_raw(400_000);
        assert_eq!(
            convert(two_glasses, Measure::Volume, Measure::Mass, flour),
            Err(DomainError::MissingDensity),
            "jamais de densité de 1 supposée"
        );
        assert_eq!(
            convert(Amount::from_raw(2_000), Measure::Count, Measure::Mass, flour),
            Err(DomainError::MissingPieceWeight)
        );
    }

    #[test]
    fn count_converts_through_mass() {
        let eggs = FoodConversions { density_mg_per_ml: Some(1_030), piece_weight_mg: Some(60_000) };
        // 3 œufs → 180 g → 174,757… mL arrondis au µL.
        let volume = convert(Amount::from_raw(3_000), Measure::Count, Measure::Volume, eggs).unwrap();
        assert_eq!(volume, Amount::from_raw(174_757));
        let back = convert(Amount::from_raw(180_000), Measure::Mass, Measure::Count, eggs).unwrap();
        assert_eq!(back, Amount::from_raw(3_000));
    }

    #[test]
    fn r_ali_32_entered_units() {
        // 2 cuillères à soupe = 30 mL.
        assert_eq!(entered_to_amount(Decimal::from(2), Amount::from_raw(15_000)).unwrap(), Amount::from_raw(30_000));
        // ½ gousse d'ail de 5 g.
        assert_eq!(entered_to_amount(Decimal::new(5, 1), Amount::from_raw(5_000)).unwrap(), Amount::from_raw(2_500));
    }

    proptest::proptest! {
        /// Aller-retour masse → volume → masse à l'arrondi près (R-ALI-33).
        #[test]
        fn mass_volume_roundtrip(mass in 0i64..10_000_000_000, density in 100i64..5_000) {
            let conv = FoodConversions { density_mg_per_ml: Some(density), piece_weight_mg: None };
            let volume = convert(Amount::from_raw(mass), Measure::Mass, Measure::Volume, conv).unwrap();
            let back = convert(volume, Measure::Volume, Measure::Mass, conv).unwrap();
            // Erreur bornée par un demi-µL converti en masse.
            proptest::prop_assert!((back.raw() - mass).abs() <= density / 1_000 + 1);
        }
    }
}

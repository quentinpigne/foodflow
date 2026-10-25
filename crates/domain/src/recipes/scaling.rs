//! Mise à l'échelle (R-REC-12 à R-REC-14).

use rust_decimal::Decimal;

use crate::shared::{Amount, DomainError};

use super::{Ingredient, Yield};

/// Facteur de mise à l'échelle, gardé en fraction : on multiplie avant de diviser, pour rester exact
/// quand c'est possible (2,6 ÷ 6 n'a pas d'écriture décimale finie, 3 × 2,6 ÷ 6 = 1,3 en a une).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Factor {
    num: Decimal,
    den: Decimal,
}

impl Factor {
    pub const ONE: Factor = Factor { num: Decimal::ONE, den: Decimal::ONE };

    pub fn new(num: Decimal, den: Decimal) -> Result<Self, DomainError> {
        if den.is_zero() { Err(DomainError::InvalidYield) } else { Ok(Self { num, den }) }
    }

    pub fn apply(self, value: Decimal) -> Decimal {
        value * self.num / self.den
    }

    /// Composition de deux facteurs (sous-recette d'une recette mise à l'échelle).
    pub fn then(self, other: Factor) -> Factor {
        Factor { num: self.num * other.num, den: self.den * other.den }
    }
}

/// Facteur pour servir `wanted` (équivalents adulte en millièmes, ou quantité de rendement) avec une
/// recette de rendement `yield_` (R-REC-12).
pub fn factor(wanted: Amount, yield_: &Yield) -> Result<Factor, DomainError> {
    yield_.check()?;
    Factor::new(wanted.to_decimal(), yield_.amount().to_decimal())
}

/// Quantité exacte d'un ingrédient pour un facteur ; un ingrédient fixe n'est pas mis à l'échelle
/// (R-REC-13). Le résultat reste décimal : l'arrondi n'intervient qu'au stockage (R-REC-14).
pub fn scaled(ingredient: &Ingredient, factor: Factor) -> Option<Decimal> {
    let amount = ingredient.amount?.to_decimal();
    Some(if ingredient.fixed { amount } else { factor.apply(amount) })
}

/// Arrondi d'affichage d'une quantité en pièces, à la demi-pièce la plus proche (R-REC-14).
pub fn display_count(raw: Decimal) -> Result<Amount, DomainError> {
    let halves =
        (raw / Decimal::from(500)).round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero);
    Amount::round(halves * Decimal::from(500))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::Target;
    use crate::shared::ids::FoodId;

    fn ingredient(amount: i64, fixed: bool) -> Ingredient {
        Ingredient {
            target: Target::Food(FoodId::new()),
            amount: Some(Amount::from_raw(amount)),
            measure: None,
            to_taste: false,
            optional: false,
            fixed,
            label: String::new(),
        }
    }

    /// Critère d'acceptation : 3 œufs pour 6 portions, servis à 2,6 équivalents adulte → 1,3 œuf, « ≈ 1 ½ ».
    #[test]
    fn r_rec_12_and_14_scaling_and_display() {
        let six = Yield::Portions { portions: Amount::from_raw(6_000) };
        let f = factor(Amount::from_raw(2_600), &six).unwrap();
        let eggs = scaled(&ingredient(3_000, false), f).unwrap();
        assert_eq!(eggs, Decimal::from(1_300));
        assert_eq!(display_count(eggs).unwrap(), Amount::from_raw(1_500));
        assert_eq!(display_count(Decimal::from(1_200)).unwrap(), Amount::from_raw(1_000));
    }

    /// Critère d'acceptation : une feuille de laurier fixe reste une feuille quand on double.
    #[test]
    fn r_rec_13_fixed_ingredient() {
        let four = Yield::Portions { portions: Amount::from_raw(4_000) };
        let f = factor(Amount::from_raw(8_000), &four).unwrap();
        assert_eq!(scaled(&ingredient(1_000, true), f).unwrap(), Decimal::from(1_000));
        assert_eq!(scaled(&ingredient(1_000, false), f).unwrap(), Decimal::from(2_000));
    }

    #[test]
    fn invalid_yield_is_refused() {
        let zero = Yield::Portions { portions: Amount::ZERO };
        assert_eq!(factor(Amount::from_raw(1_000), &zero), Err(DomainError::InvalidYield));
    }
}

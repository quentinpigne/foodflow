//! Quantités en entiers, dans l'unité de stockage de leur mesure (ADR-0001).

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use super::DomainError;

/// Grandeur d'une quantité (glossaire : mesure).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Measure {
    /// Stockée en milligrammes.
    Mass,
    /// Stockée en microlitres.
    Volume,
    /// Stockée en millièmes de pièce.
    Count,
}

impl Measure {
    /// Nombre d'unités de stockage dans l'unité d'affichage de référence (g, mL, pièce).
    pub const fn base_factor(self) -> i64 {
        match self {
            Measure::Mass | Measure::Volume | Measure::Count => 1_000,
        }
    }

    /// Unité d'affichage de référence.
    pub const fn base_symbol(self) -> &'static str {
        match self {
            Measure::Mass => "g",
            Measure::Volume => "mL",
            Measure::Count => "pièce",
        }
    }
}

/// Quantité signée dans l'unité de stockage d'une mesure : mg, µL ou millième de pièce (ou de portion).
///
/// La mesure n'est pas répétée : elle est celle de l'aliment (mesure de stock) ou de l'unité qui porte la
/// quantité. Toute conversion depuis ou vers un décimal passe par [`Measure::base_factor`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Amount(i64);

impl Amount {
    pub const ZERO: Amount = Amount(0);

    pub const fn from_raw(raw: i64) -> Self {
        Amount(raw)
    }

    pub const fn raw(self) -> i64 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub const fn abs(self) -> Self {
        Amount(self.0.abs())
    }

    pub fn min(self, other: Amount) -> Amount {
        Amount(self.0.min(other.0))
    }

    /// Valeur exacte en unité de référence (1 500 000 mg → 1 500 g).
    pub fn to_base(self, measure: Measure) -> Decimal {
        Decimal::from(self.0) / Decimal::from(measure.base_factor())
    }

    /// Valeur exacte en unités de stockage.
    pub fn to_decimal(self) -> Decimal {
        Decimal::from(self.0)
    }

    /// Arrondit une valeur en unités de stockage à l'unité la plus proche (ADR-0001, R-ALI-33).
    pub fn round(value: Decimal) -> Result<Amount, DomainError> {
        value
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_i64()
            .map(Amount)
            .ok_or(DomainError::QuantityOutOfRange)
    }

    /// Convertit une valeur en unité de référence (g, mL, pièce) en quantité stockée.
    pub fn from_base(value: Decimal, measure: Measure) -> Result<Amount, DomainError> {
        Amount::round(value * Decimal::from(measure.base_factor()))
    }
}

impl Add for Amount {
    type Output = Amount;

    fn add(self, rhs: Amount) -> Amount {
        Amount(self.0 + rhs.0)
    }
}

impl AddAssign for Amount {
    fn add_assign(&mut self, rhs: Amount) {
        self.0 += rhs.0;
    }
}

impl Sub for Amount {
    type Output = Amount;

    fn sub(self, rhs: Amount) -> Amount {
        Amount(self.0 - rhs.0)
    }
}

impl SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Amount) {
        self.0 -= rhs.0;
    }
}

impl Neg for Amount {
    type Output = Amount;

    fn neg(self) -> Amount {
        Amount(-self.0)
    }
}

impl Sum for Amount {
    fn sum<I: Iterator<Item = Amount>>(iter: I) -> Amount {
        iter.fold(Amount::ZERO, Add::add)
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for Amount {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::Integer)
            .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(utoipa::openapi::KnownFormat::Int64)))
            .description(Some("Quantité en unités de stockage : mg, µL ou millième de pièce (ADR-0001)"))
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for Amount {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_conversions_are_exact() {
        let amount = Amount::from_base(Decimal::new(15, 1), Measure::Mass).unwrap();
        assert_eq!(amount, Amount::from_raw(1_500));
        assert_eq!(Amount::from_raw(1_500_000).to_base(Measure::Mass), Decimal::from(1_500));
        assert_eq!(Amount::round(Decimal::new(25, 1)).unwrap(), Amount::from_raw(3));
        assert_eq!(Amount::round(Decimal::new(-25, 1)).unwrap(), Amount::from_raw(-3));
    }
}

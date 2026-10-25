//! Montants en centimes d'euro (ADR-0001).

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use super::DomainError;

/// Montant signé en centimes d'euro.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_cents(cents: i64) -> Self {
        Money(cents)
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// Valeur décimale exacte en euros (12,34 pour 1 234 centimes).
    pub fn to_decimal(self) -> Decimal {
        Decimal::new(self.0, 2)
    }

    /// Convertit un décimal en euros en montant, arrondi au centime, demi vers le haut (ADR-0001).
    pub fn from_decimal(value: Decimal) -> Result<Self, DomainError> {
        let cents = (value * Decimal::from(100)).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        cents.to_i64().map(Money).ok_or(DomainError::AmountOutOfRange)
    }
}

impl Add for Money {
    type Output = Money;

    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Money;

    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl Neg for Money {
    type Output = Money;

    fn neg(self) -> Money {
        Money(-self.0)
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Add::add)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} €", self.to_decimal())
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for Money {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::Integer)
            .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(utoipa::openapi::KnownFormat::Int64)))
            .description(Some("Montant en centimes d'euro (ADR-0001)"))
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for Money {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_roundtrip_rounds_half_up() {
        assert_eq!(Money::from_decimal(Decimal::new(2_195, 3)).unwrap(), Money::from_cents(220));
        assert_eq!(Money::from_cents(329).to_decimal(), Decimal::new(329, 2));
    }
}

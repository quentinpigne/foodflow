//! Coefficients en millièmes (ADR-0001) : équivalent adulte, part d'énergie, rendement.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Coefficient en millièmes : 1,000 → `1000`, 0,6 → `600`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Ratio(i64);

impl Ratio {
    pub const ONE: Ratio = Ratio(1_000);

    pub const fn from_thousandths(value: i64) -> Self {
        Ratio(value)
    }

    pub const fn thousandths(self) -> i64 {
        self.0
    }

    pub fn to_decimal(self) -> Decimal {
        Decimal::new(self.0, 3)
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for Ratio {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::Integer)
            .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(utoipa::openapi::KnownFormat::Int64)))
            .description(Some("Coefficient en millièmes : 1,000 → 1000 (ADR-0001)"))
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for Ratio {}

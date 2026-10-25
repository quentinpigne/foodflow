//! Types partagés par tous les modules du domaine.

pub mod amount;
pub mod error;
pub mod ids;
pub mod money;
pub mod ratio;

pub use amount::{Amount, Measure};
pub use error::DomainError;
pub use money::Money;
pub use ratio::Ratio;

/// Date calendaire, sans heure ni fuseau (ADR-0009, conventions de Cashflow).
pub type Date = chrono::NaiveDate;

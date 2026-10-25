//! Domaine de Foodflow : types, invariants et moteurs de calcul (architecture.md).
//!
//! Pur et sans I/O : aucune dépendance à la base, au système de fichiers, au réseau ou à l'horloge.
//! La date du jour est toujours un paramètre. Les flottants y sont interdits (ADR-0001).

pub mod foods;
pub mod nutrition;
pub mod planning;
pub mod recipes;
pub mod scheduling;
pub mod shared;
pub mod stock;
pub mod text;

pub use shared::{Amount, Date, DomainError, Measure, Money, Ratio};

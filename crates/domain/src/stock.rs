//! Stock (spec 02) : lots, journal de mouvements, péremption, choix des lots (ADR-0002).

pub mod expiry;
pub mod journal;
pub mod selection;

use serde::{Deserialize, Serialize};

/// Nature d'un emplacement : détermine les durées de conservation qui s'appliquent.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocationKind {
    Pantry,
    Fridge,
    Freezer,
}

/// Mention de péremption d'un lot.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpiryKind {
    /// DLC, « à consommer jusqu'au ».
    UseBy,
    /// DDM, « à consommer de préférence avant ».
    BestBefore,
    /// Aucune date : durée de conservation par défaut de l'aliment.
    None,
}

/// Type d'un mouvement de stock (R-STK-10).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MovementKind {
    Purchase,
    Cooked,
    Leftover,
    Prepared,
    Waste,
    Move,
    Adjustment,
    Inventory,
    Opened,
    Checked,
}

/// État d'un lot suivi en présence.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PresenceState {
    Available,
    Low,
    Out,
}

/// Motif d'un produit jeté (R-STK-13).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WasteReason {
    Expired,
    Spoiled,
    Other,
}

/// État de péremption d'un lot à une date (R-STK-35).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExpiryStatus {
    Ok,
    Soon,
    Expired,
    PastBestBefore,
}

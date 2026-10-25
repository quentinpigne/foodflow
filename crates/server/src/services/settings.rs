//! Réglages généraux de l'application (`app_setting`).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const SOON_DAYS: &str = "stock.soon_days";
/// Seuil « bientôt » par défaut (R-GAS-2).
pub const DEFAULT_SOON_DAYS: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AppSettings {
    /// Seuil « bientôt » général, en jours ; un emplacement peut avoir le sien (R-GAS-2).
    pub soon_days: u32,
}

pub async fn get(db: impl sqlx::SqliteExecutor<'_>) -> anyhow::Result<AppSettings> {
    let soon_days = db::settings::get(db, SOON_DAYS).await?.unwrap_or(DEFAULT_SOON_DAYS);
    Ok(AppSettings { soon_days })
}

pub async fn update(db: &Db, input: &AppSettings) -> AppResult<()> {
    if input.soon_days > 365 {
        return Err(AppError::invalid_field("OUT_OF_RANGE", "soon_days", "au plus 365 jours"));
    }
    db::settings::set(&db.write, SOON_DAYS, &input.soon_days).await?;
    Ok(())
}

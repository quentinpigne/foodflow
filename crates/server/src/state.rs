//! État partagé par les routes.

use std::sync::Arc;

use crate::clients::ai::AiClient;
use crate::clients::web::Fetcher;
use crate::clock::Clock;
use crate::config::DataDir;
use crate::db::Db;
use crate::services::auth::LoginLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub clock: Clock,
    pub data: DataDir,
    pub limiter: Arc<LoginLimiter>,
    /// Drapeau `Secure` du cookie de session : actif dès que le serveur est servi en HTTPS (ADR-0009).
    pub secure_cookies: bool,
    /// Téléchargement des pages de recettes (ADR-0007).
    pub fetcher: Arc<dyn Fetcher>,
    /// IA (ADR-0014), appelée seulement si elle est activée dans les réglages.
    pub ai: Arc<dyn AiClient>,
}

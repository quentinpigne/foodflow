//! Propositions de menus (`generation_run`, R-PLA-64).

use foodflow_domain::Date;
use sqlx::SqliteExecutor;

use super::codec::{date_from_db, id_from_db};
use crate::clock::timestamp;

pub use foodflow_domain::shared::ids::RowId as RunId;

#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: RunId,
    pub period_start: Date,
    pub period_end: Date,
    pub prompt: Option<String>,
    pub request_payload: Option<String>,
    pub response_payload: Option<String>,
    /// Proposition contrôlée, en JSON.
    pub proposal: String,
    pub status: String,
    pub created_at: String,
}

macro_rules! run_row {
    ($r:expr) => {{
        let r = $r;
        Ok::<_, anyhow::Error>(RunRow {
            id: id_from_db(&r.id)?,
            period_start: date_from_db(&r.period_start)?,
            period_end: date_from_db(&r.period_end)?,
            prompt: r.prompt,
            request_payload: r.request_payload,
            response_payload: r.response_payload,
            proposal: r.proposal,
            status: r.status,
            created_at: r.created_at,
        })
    }};
}

pub async fn insert(db: impl SqliteExecutor<'_>, run: &RunRow) -> anyhow::Result<()> {
    let id = run.id.to_string();
    let (start, end) = (run.period_start.to_string(), run.period_end.to_string());
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO generation_run (id, kind, period_start, period_end, prompt, request_payload, response_payload,
             proposal, status, created_at, updated_at)
         VALUES (?, 'AI', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        start,
        end,
        run.prompt,
        run.request_payload,
        run.response_payload,
        run.proposal,
        run.status,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: RunId, proposal: &str, status: &str) -> anyhow::Result<()> {
    let id = id.to_string();
    let now = timestamp();
    sqlx::query!(
        "UPDATE generation_run SET proposal = ?, status = ?, updated_at = ? WHERE id = ?",
        proposal,
        status,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find(db: impl SqliteExecutor<'_>, id: RunId) -> anyhow::Result<Option<RunRow>> {
    let id = id.to_string();
    let row = sqlx::query!(
        "SELECT id, period_start, period_end, prompt, request_payload, response_payload, proposal, status, created_at
         FROM generation_run WHERE id = ?",
        id
    )
    .fetch_optional(db)
    .await?;
    row.map(|r| run_row!(r)).transpose()
}

/// Propositions encore ouvertes (en attente ou acceptées en partie), les plus récentes d'abord.
pub async fn open(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<RunRow>> {
    let rows = sqlx::query!(
        "SELECT id, period_start, period_end, prompt, request_payload, response_payload, proposal, status, created_at
         FROM generation_run WHERE status IN ('PENDING', 'PARTIALLY_ACCEPTED') ORDER BY created_at DESC"
    )
    .fetch_all(db)
    .await?;
    rows.into_iter().map(|r| run_row!(r)).collect()
}

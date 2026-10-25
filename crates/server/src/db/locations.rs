//! Emplacements (spec 02, STK-01).

use foodflow_domain::shared::ids::LocationId;
use foodflow_domain::stock::LocationKind;
use sqlx::SqliteExecutor;

use super::codec::{enum_from_db, enum_to_db, id_from_db};
use super::groups::days;
use crate::clock::timestamp;

#[derive(Debug, Clone)]
pub struct LocationRow {
    pub id: LocationId,
    pub name: String,
    pub kind: LocationKind,
    pub position: i64,
    pub soon_days: Option<u32>,
    pub lot_count: i64,
}

pub struct LocationFields<'a> {
    pub name: &'a str,
    pub name_key: &'a str,
    pub kind: LocationKind,
    pub position: i64,
    pub soon_days: Option<u32>,
}

pub async fn list(db: impl SqliteExecutor<'_>) -> anyhow::Result<Vec<LocationRow>> {
    let rows = sqlx::query!(
        r#"SELECT l.id, l.name, l.kind, l.position, l.soon_days,
                  (SELECT count(*) FROM stock_lot s WHERE s.location_id = l.id AND s.depleted = 0) AS "lot_count!: i64"
           FROM location l ORDER BY l.position, l.name COLLATE NOCASE"#
    )
    .fetch_all(db)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(LocationRow {
                id: id_from_db(&r.id)?,
                name: r.name,
                kind: enum_from_db(&r.kind)?,
                position: r.position,
                soon_days: days(r.soon_days),
                lot_count: r.lot_count,
            })
        })
        .collect()
}

pub async fn name_taken(db: impl SqliteExecutor<'_>, key: &str, except: Option<LocationId>) -> anyhow::Result<bool> {
    let except = except.map(|e| e.to_string());
    let n = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!: i64" FROM location WHERE name_key = ? AND (? IS NULL OR id <> ?)"#,
        key,
        except,
        except
    )
    .fetch_one(db)
    .await?;
    Ok(n > 0)
}

pub async fn count(db: impl SqliteExecutor<'_>) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!(r#"SELECT count(*) AS "n!: i64" FROM location"#).fetch_one(db).await?)
}

pub async fn insert(db: impl SqliteExecutor<'_>, id: LocationId, f: &LocationFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let kind = enum_to_db(&f.kind);
    let now = timestamp();
    sqlx::query!(
        "INSERT INTO location (id, name, name_key, kind, position, soon_days, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        id,
        f.name,
        f.name_key,
        kind,
        f.position,
        f.soon_days,
        now,
        now
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn update(db: impl SqliteExecutor<'_>, id: LocationId, f: &LocationFields<'_>) -> anyhow::Result<()> {
    let id = id.to_string();
    let kind = enum_to_db(&f.kind);
    let now = timestamp();
    sqlx::query!(
        "UPDATE location SET name = ?, name_key = ?, kind = ?, position = ?, soon_days = ?, updated_at = ? WHERE id = ?",
        f.name,
        f.name_key,
        kind,
        f.position,
        f.soon_days,
        now,
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Supprime un emplacement qui n'a jamais contenu de lot.
pub async fn delete(db: impl SqliteExecutor<'_>, id: LocationId) -> anyhow::Result<bool> {
    let id = id.to_string();
    let done = sqlx::query!(
        "DELETE FROM location WHERE id = ?1
           AND NOT EXISTS (SELECT 1 FROM stock_lot WHERE location_id = ?1)
           AND NOT EXISTS (SELECT 1 FROM stock_movement WHERE from_location_id = ?1 OR to_location_id = ?1)",
        id
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

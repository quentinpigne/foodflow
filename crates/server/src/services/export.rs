//! Export complet au format JSON et restauration (ADR-0008, format décrit dans `docs/export.md`).

use anyhow::{Context, bail};
use serde_json::{Value, json};

use crate::clock::timestamp;
use crate::db::{self, Db, MIGRATOR};

pub const FORMAT: &str = "foodflow-export";

/// Version du schéma : celle de la dernière migration connue.
pub fn schema_version() -> i64 {
    MIGRATOR.iter().map(|m| m.version).max().unwrap_or(0)
}

pub async fn export(db: &Db) -> anyhow::Result<Value> {
    // Une transaction de lecture : toutes les tables vues au même instant.
    let mut tx = db.read.begin().await?;
    let tables = db::export::dump(&mut tx).await?;
    tx.commit().await?;
    Ok(json!({
        "format": FORMAT,
        "schema_version": schema_version(),
        "app_version": env!("CARGO_PKG_VERSION"),
        "exported_at": timestamp(),
        "tables": tables,
    }))
}

/// Restaure un export dans une base neuve (migrations appliquées, aucune donnée).
pub async fn restore(db: &Db, export: &Value) -> anyhow::Result<()> {
    if export.get("format").and_then(Value::as_str) != Some(FORMAT) {
        bail!("ce fichier n'est pas un export Foodflow");
    }
    let version = export.get("schema_version").and_then(Value::as_i64).context("schema_version manquant")?;
    if version != schema_version() {
        bail!("export au schéma {version}, application au schéma {} : versions incompatibles", schema_version());
    }
    let tables = export.get("tables").and_then(Value::as_object).context("tables manquantes")?;
    let mut tx = db.write.begin().await?;
    // Les références internes (catégorie parente, version précédente d'une ligne de budget) peuvent
    // précéder leur cible dans l'ordre d'export : contrôle des clés étrangères à la validation.
    sqlx::query("PRAGMA defer_foreign_keys = ON").execute(&mut *tx).await?;
    db::export::restore(&mut tx, tables).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un export suivi d'une restauration restitue exactement les mêmes données (ADR-0008).
    #[tokio::test]
    async fn export_roundtrip() {
        let (source, _dir) = db::test_db().await;
        crate::services::demo::load(&source, "2026-10-25".parse().unwrap()).await.unwrap();
        let first = export(&source).await.unwrap();

        let (target, _dir2) = db::test_db().await;
        restore(&target, &first).await.unwrap();
        let second = export(&target).await.unwrap();
        assert_eq!(first["tables"], second["tables"]);
        assert!(first["tables"]["stock_movement"].as_array().unwrap().len() >= 10);
        assert_eq!(first["tables"]["recipe"].as_array().unwrap().len(), 4);
        assert_eq!(first["tables"]["goal_criterion"].as_array().unwrap().len(), 36, "modèle PNNS et version du foyer");
    }

    #[tokio::test]
    async fn restore_refuses_non_empty_database() {
        let (source, _dir) = db::test_db().await;
        crate::services::demo::load(&source, "2026-10-25".parse().unwrap()).await.unwrap();
        let data = export(&source).await.unwrap();
        assert!(restore(&source, &data).await.is_err());
    }
}

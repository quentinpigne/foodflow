//! Version du référentiel (`catalog_version`), tenue par des déclencheurs à chaque écriture sur les familles,
//! les aliments et les produits.

use sqlx::SqliteConnection;

/// Version courante, telle que la connexion la voit (dans une transaction : avec ses propres écritures).
pub async fn version(conn: &mut SqliteConnection) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar!("SELECT version FROM catalog_version WHERE id = 1").fetch_one(&mut *conn).await?)
}

/// Fichier de la base ouverte par la connexion ; vide pour une base en mémoire.
pub async fn file(conn: &mut SqliteConnection) -> anyhow::Result<String> {
    let file: Option<String> =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'").fetch_one(&mut *conn).await?;
    Ok(file.unwrap_or_default())
}

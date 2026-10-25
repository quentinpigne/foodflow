//! Images des recettes (REC-10, R-REC-68) : conservées dans le volume, sous `images/`.

use std::path::PathBuf;

use anyhow::Context;
use foodflow_domain::shared::ids::RecipeId;

use crate::clients::web::MAX_BYTES;
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

/// Extension d'une image acceptée, d'après son type ou ses premiers octets.
pub fn extension(content_type: Option<&str>, bytes: &[u8]) -> Option<&'static str> {
    let by_type =
        match content_type.map(|t| t.split(';').next().unwrap_or_default().trim().to_ascii_lowercase()).as_deref() {
            Some("image/jpeg" | "image/jpg") => Some("jpg"),
            Some("image/png") => Some("png"),
            Some("image/webp") => Some("webp"),
            _ => None,
        };
    by_type.or(if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG") {
        Some("png")
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    })
}

pub fn content_type(path: &str) -> &'static str {
    if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

/// Écrit une image dans le volume ; renvoie son nom de fichier.
pub fn store(data: &DataDir, stem: &str, content_type: Option<&str>, bytes: &[u8]) -> AppResult<String> {
    if bytes.len() > MAX_BYTES {
        return Err(AppError::invalid("IMAGE_TOO_LARGE", "image trop volumineuse (8 Mo au plus)"));
    }
    let ext = extension(content_type, bytes)
        .ok_or_else(|| AppError::invalid("IMAGE_TYPE", "seules les images JPEG, PNG et WebP sont acceptées"))?;
    let name = format!("{stem}.{ext}");
    data.ensure().context("dossier des images")?;
    std::fs::write(data.images().join(&name), bytes).with_context(|| format!("écriture de l'image {name}"))?;
    Ok(name)
}

fn path(data: &DataDir, name: &str) -> AppResult<PathBuf> {
    // Un nom de fichier seul, jamais un chemin.
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(AppError::NotFound("image"));
    }
    Ok(data.images().join(name))
}

pub fn remove(data: &DataDir, name: &str) {
    if let Ok(p) = path(data, name) {
        let _ = std::fs::remove_file(p);
    }
}

/// Remplace l'image d'une recette.
pub async fn upload(db: &Db, data: &DataDir, id: RecipeId, content_type: Option<&str>, bytes: &[u8]) -> AppResult<()> {
    let recipe = db::recipes::find(&db.read, id).await?.ok_or(AppError::NotFound("recette"))?;
    let name = store(
        data,
        &format!("recipe-{id}-{}", crate::clock::timestamp().replace([':', '.'], "")),
        content_type,
        bytes,
    )?;
    db::recipes::set_image(&db.write, id, Some(&name)).await?;
    if let Some(old) = recipe.image_path {
        remove(data, &old);
    }
    Ok(())
}

pub async fn delete(db: &Db, data: &DataDir, id: RecipeId) -> AppResult<()> {
    let recipe = db::recipes::find(&db.read, id).await?.ok_or(AppError::NotFound("recette"))?;
    db::recipes::set_image(&db.write, id, None).await?;
    if let Some(old) = recipe.image_path {
        remove(data, &old);
    }
    Ok(())
}

/// Octets et type de l'image d'une recette.
pub async fn read(db: &Db, data: &DataDir, id: RecipeId) -> AppResult<(Vec<u8>, &'static str)> {
    let recipe = db::recipes::find(&db.read, id).await?.ok_or(AppError::NotFound("recette"))?;
    let name = recipe.image_path.ok_or(AppError::NotFound("image"))?;
    let bytes = std::fs::read(path(data, &name)?).map_err(|_| AppError::NotFound("image"))?;
    Ok((bytes, content_type(&name)))
}

//! Conversions entre colonnes SQLite et types du domaine.

use std::str::FromStr;

use anyhow::{Context, anyhow};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Enum du domaine → valeur stockée (`'LIVRET_A'`), via sa représentation serde.
pub fn enum_to_db<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => String::new(),
    }
}

/// Valeur stockée → enum du domaine.
pub fn enum_from_db<T: DeserializeOwned>(value: &str) -> anyhow::Result<T> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .with_context(|| format!("valeur inconnue en base : {value}"))
}

/// Identifiant stocké → identifiant typé.
pub fn id_from_db<T: FromStr>(value: &str) -> anyhow::Result<T> {
    value.parse().map_err(|_| anyhow!("identifiant invalide en base : {value}"))
}

pub fn opt_id_from_db<T: FromStr>(value: Option<&str>) -> anyhow::Result<Option<T>> {
    value.map(id_from_db).transpose()
}

/// Enum facultatif du domaine → valeur stockée.
pub fn opt_enum_to_db<T: Serialize>(value: &Option<T>) -> Option<String> {
    value.as_ref().map(enum_to_db)
}

/// Valeur stockée facultative → enum du domaine.
pub fn opt_enum_from_db<T: DeserializeOwned>(value: Option<&str>) -> anyhow::Result<Option<T>> {
    value.map(enum_from_db).transpose()
}

/// Date stockée (`YYYY-MM-DD`) → date du domaine.
pub fn date_from_db(value: &str) -> anyhow::Result<foodflow_domain::Date> {
    value.parse().with_context(|| format!("date invalide en base : {value}"))
}

pub fn opt_date_from_db(value: Option<&str>) -> anyhow::Result<Option<foodflow_domain::Date>> {
    value.map(date_from_db).transpose()
}

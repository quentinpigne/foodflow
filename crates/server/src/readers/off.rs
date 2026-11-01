//! Lecture d'une fiche produit d'Open Food Facts (R-ALI-60).
//!
//! Les nombres sont lus tels qu'ils sont écrits dans la réponse (`RawValue`), puis convertis en décimal
//! exact : ils ne passent jamais par un flottant (ADR-0001).

use std::collections::HashMap;

use anyhow::Context;
use foodflow_domain::foods::NutrientBasis;
use foodflow_domain::foods::nutrients::{CARBOHYDRATE, ENERGY, FAT, FIBER, PROTEIN, SALT, SATURATED_FAT, SUGARS};
use foodflow_domain::{Amount, Measure};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Deserialize;
use serde_json::value::RawValue;

/// Ce qu'une fiche d'Open Food Facts apporte à un brouillon de produit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffProduct {
    pub name: Option<String>,
    pub brand: Option<String>,
    /// Conditionnement total (« 6 × 125 g » : 750 g).
    pub package: Option<(Measure, Amount)>,
    /// Nombre d'unités d'un lot vendu groupé (« 6 × 125 g » : 6).
    pub package_units: Option<i64>,
    pub nutrient_basis: NutrientBasis,
    /// Valeurs pour 100 g ou 100 mL, dans l'unité de stockage du nutriment (cal ou mg).
    pub nutrients: Vec<(&'static str, i64)>,
    /// Catégories, de la plus générale à la plus précise (`en:pastas`, `en:spaghetti`).
    pub categories: Vec<String>,
    /// Les mêmes catégories, nommées en français (« Pâtes sèches »).
    pub category_labels: Vec<String>,
}

#[derive(Deserialize)]
struct Response<'a> {
    #[serde(borrow)]
    product: Option<RawProduct<'a>>,
}

#[derive(Deserialize)]
struct RawProduct<'a> {
    product_name_fr: Option<String>,
    product_name: Option<String>,
    brands: Option<String>,
    quantity: Option<String>,
    #[serde(borrow)]
    product_quantity: Option<&'a RawValue>,
    product_quantity_unit: Option<String>,
    #[serde(default)]
    categories_tags: Vec<String>,
    #[serde(default)]
    categories_tags_fr: Vec<String>,
    #[serde(borrow, default)]
    nutriments: HashMap<String, &'a RawValue>,
}

/// Nutriments lus, avec leur clé et le facteur vers l'unité de stockage (g → mg, kcal → cal).
const NUTRIENTS: [(&str, &str, i64); 8] = [
    (ENERGY, "energy-kcal_100g", 1_000),
    (PROTEIN, "proteins_100g", 1_000),
    (CARBOHYDRATE, "carbohydrates_100g", 1_000),
    (SUGARS, "sugars_100g", 1_000),
    (FAT, "fat_100g", 1_000),
    (SATURATED_FAT, "saturated-fat_100g", 1_000),
    (FIBER, "fiber_100g", 1_000),
    (SALT, "salt_100g", 1_000),
];

/// Lit une réponse de `api/v2/product/{code}` ; `None` si elle ne décrit aucun produit.
pub fn parse(json: &str) -> anyhow::Result<Option<OffProduct>> {
    let response: Response = serde_json::from_str(json).context("réponse d'Open Food Facts illisible")?;
    let Some(raw) = response.product else {
        return Ok(None);
    };
    let name = [raw.product_name_fr, raw.product_name]
        .into_iter()
        .flatten()
        .map(|n| n.trim().to_owned())
        .find(|n| !n.is_empty())
        .map(|n| truncate(&n, 200));
    let brand = raw
        .brands
        .as_deref()
        .and_then(|b| b.split(',').map(str::trim).find(|b| !b.is_empty()))
        .map(|b| truncate(b, 120));
    let declared = raw
        .product_quantity
        .and_then(decimal)
        .zip(raw.product_quantity_unit.as_deref())
        .and_then(|(value, unit)| amount(value, unit));
    let (text_package, package_units) = raw.quantity.as_deref().map(quantity).unwrap_or((None, None));
    let package = declared.or(text_package);
    let nutrient_basis = match package {
        Some((Measure::Volume, _)) => NutrientBasis::Per100Ml,
        _ => NutrientBasis::Per100G,
    };
    let mut nutrients = Vec::new();
    for (code, key, scale) in NUTRIENTS {
        let value = raw.nutriments.get(key).copied().and_then(decimal).map(|v| v * Decimal::from(scale));
        // Sans énergie en kcal, l'énergie en kJ : 1 kcal = 4,184 kJ.
        let value = value.or_else(|| {
            (code == ENERGY)
                .then(|| raw.nutriments.get("energy_100g").copied().and_then(decimal))
                .flatten()
                .map(|kj| kj * Decimal::from(1_000) / Decimal::new(4184, 3))
        });
        if let Some(stored) = value.and_then(stored) {
            nutrients.push((code, stored));
        }
    }
    Ok(Some(OffProduct {
        name,
        brand,
        package,
        package_units,
        nutrient_basis,
        nutrients,
        categories: raw.categories_tags,
        category_labels: raw.categories_tags_fr,
    }))
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// Un nombre écrit `12.5`, `"12,5"` ou `"500"`.
fn decimal(raw: &RawValue) -> Option<Decimal> {
    let text = raw.get().trim().trim_matches('"').replace(',', ".");
    text.trim().parse().ok().filter(|d: &Decimal| !d.is_sign_negative())
}

/// Valeur arrondie à l'entier dans l'unité de stockage ; `None` si elle est négative ou démesurée.
fn stored(value: Decimal) -> Option<i64> {
    value.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero).to_i64().filter(|v| *v >= 0)
}

/// Quantité dans une unité (`g`, `kg`, `mL`, `cL`, `L`…), en unité de stockage (mg, µL).
fn amount(value: Decimal, unit: &str) -> Option<(Measure, Amount)> {
    let (measure, scale) = match unit.trim().to_lowercase().as_str() {
        "mg" => (Measure::Mass, 1),
        "g" | "gr" | "grammes" => (Measure::Mass, 1_000),
        "kg" => (Measure::Mass, 1_000_000),
        "ml" => (Measure::Volume, 1_000),
        "cl" => (Measure::Volume, 10_000),
        "dl" => (Measure::Volume, 100_000),
        "l" | "litre" | "litres" => (Measure::Volume, 1_000_000),
        _ => return None,
    };
    stored(value * Decimal::from(scale)).filter(|v| *v > 0).map(|v| (measure, Amount::from_raw(v)))
}

/// Conditionnement écrit en clair : « 500 g », « 1,5 L », « 6 x 125 g » (total et nombre d'unités).
fn quantity(text: &str) -> (Option<(Measure, Amount)>, Option<i64>) {
    let text = text.to_lowercase().replace('×', "x");
    let (units, single) = match text.split_once('x') {
        Some((count, rest)) => match count.trim().parse::<i64>() {
            Ok(n) if n > 0 => (Some(n), rest.trim().to_owned()),
            _ => (None, text.trim().to_owned()),
        },
        None => (None, text.trim().to_owned()),
    };
    let split =
        single.find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.' || c == ' ')).unwrap_or(single.len());
    let (number, unit) = single.split_at(split);
    let Ok(value) = number.replace(' ', "").replace(',', ".").parse::<Decimal>() else {
        return (None, None);
    };
    let unit = unit.split_whitespace().next().unwrap_or_default();
    let total = value * Decimal::from(units.unwrap_or(1));
    match amount(total, unit) {
        Some(package) => (Some(package), units.filter(|n| *n > 1)),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANZANI: &str = r#"{"code":"3038350013804","status":1,"product":{
        "product_name_fr":"Spaghetti","product_name":"Spaghetti n°5","brands":"Panzani, Ebro",
        "quantity":"500 g","product_quantity":"500","product_quantity_unit":"g",
        "categories_tags":["en:plant-based-foods","en:pastas","en:spaghetti"],
        "nutriments":{"energy-kcal_100g":359,"energy_100g":1521,"proteins_100g":12.5,"carbohydrates_100g":71,
          "sugars_100g":3.5,"fat_100g":1.6,"saturated-fat_100g":0.3,"fiber_100g":3,"salt_100g":0.013}}}"#;

    /// R-ALI-60 : nom, marque, conditionnement, valeurs pour 100 g et catégories, sans flottant.
    #[test]
    fn r_ali_60_reads_a_product() {
        let product = parse(PANZANI).unwrap().unwrap();
        assert_eq!(product.name.as_deref(), Some("Spaghetti"));
        assert_eq!(product.brand.as_deref(), Some("Panzani"));
        assert_eq!(product.package, Some((Measure::Mass, Amount::from_raw(500_000))));
        assert_eq!(product.nutrient_basis, NutrientBasis::Per100G);
        let value = |code| product.nutrients.iter().find(|(c, _)| *c == code).map(|(_, v)| *v);
        assert_eq!(value(ENERGY), Some(359_000));
        assert_eq!(value(PROTEIN), Some(12_500));
        assert_eq!(value(SALT), Some(13));
        assert_eq!(product.categories.last().map(String::as_str), Some("en:spaghetti"));
    }

    #[test]
    fn r_ali_60_unknown_product() {
        assert_eq!(parse(r#"{"code":"123","status":0,"status_verbose":"product not found"}"#).unwrap(), None);
        assert!(parse("<html>").is_err());
    }

    #[test]
    fn r_ali_60_energy_from_kilojoules_and_volumes() {
        let json = r#"{"product":{"product_name":"Lait demi-écrémé","quantity":"1 L",
            "nutriments":{"energy_100g":"193","fat_100g":"1,5"}}}"#;
        let product = parse(json).unwrap().unwrap();
        assert_eq!(product.package, Some((Measure::Volume, Amount::from_raw(1_000_000))));
        assert_eq!(product.nutrient_basis, NutrientBasis::Per100Ml);
        // 193 kJ = 46,128… kcal.
        assert_eq!(product.nutrients, vec![(ENERGY, 46_128), (FAT, 1_500)]);
    }

    #[test]
    fn r_ali_60_multipacks() {
        assert_eq!(quantity("6 x 125 g"), (Some((Measure::Mass, Amount::from_raw(750_000))), Some(6)));
        assert_eq!(quantity("1,5 kg"), (Some((Measure::Mass, Amount::from_raw(1_500_000))), None));
        assert_eq!(quantity("33 cl"), (Some((Measure::Volume, Amount::from_raw(330_000))), None));
        assert_eq!(quantity("12 œufs"), (None, None));
    }
}

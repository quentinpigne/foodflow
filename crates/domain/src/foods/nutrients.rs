//! Valeurs nutritionnelles : connues, estimées ou inconnues (R-ALI-20 à R-ALI-24).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use super::ValueSource;

pub const ENERGY: &str = "ENERGY";
pub const PROTEIN: &str = "PROTEIN";
pub const CARBOHYDRATE: &str = "CARBOHYDRATE";
pub const SUGARS: &str = "SUGARS";
pub const FAT: &str = "FAT";
pub const SATURATED_FAT: &str = "SATURATED_FAT";
pub const FIBER: &str = "FIBER";
pub const SALT: &str = "SALT";

/// Unité de stockage d'un nutriment (modèle de données, `Nutrient.storage_unit`).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NutrientUnit {
    /// Calorie : millième de kcal.
    Cal,
    Mg,
    Ug,
}

/// Valeur connue d'un nutriment, pour 100 g ou 100 mL. Une valeur inconnue est une absence (R-ALI-20).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NutrientValue {
    pub value: i64,
    pub estimated: bool,
    pub source: ValueSource,
}

/// Lecture d'une teneur de la table Ciqual (R-ALI-21).
///
/// `scale` passe de l'unité de la table à l'unité de stockage (g → mg : 1 000 ; kcal → cal : 1 000).
/// Renvoie `None` pour une valeur inconnue (« - », vide).
pub fn parse_ciqual(raw: &str, scale: i64) -> Option<(i64, bool)> {
    let text = raw.trim();
    if text.is_empty() || text == "-" {
        return None;
    }
    if text.eq_ignore_ascii_case("traces") {
        return Some((0, true));
    }
    let (number, estimated) = match text.strip_prefix('<') {
        Some(rest) => (rest.trim(), true),
        None => (text, false),
    };
    let value: Decimal = number.replace(',', ".").parse().ok()?;
    let stored = (value * Decimal::from(scale)).round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    stored.to_i64().filter(|v| *v >= 0).map(|v| (v, estimated))
}

/// Valeur effective d'un nutriment d'un produit : la sienne si elle est connue, sinon celle de son aliment
/// (R-ALI-22).
pub fn effective(product: Option<NutrientValue>, food: Option<NutrientValue>) -> Option<NutrientValue> {
    product.or(food)
}

/// Énergie calculée à partir des macronutriments, en cal, si tous sont connus (R-ALI-24).
///
/// Coefficients réglementaires : protéines et glucides 4 kcal/g, lipides 9 kcal/g, fibres 2 kcal/g ; soit
/// en cal par mg : 4, 4, 9 et 2.
pub fn computed_energy(protein_mg: i64, carbohydrate_mg: i64, fat_mg: i64, fiber_mg: i64) -> NutrientValue {
    NutrientValue {
        value: 4 * protein_mg + 4 * carbohydrate_mg + 9 * fat_mg + 2 * fiber_mg,
        estimated: true,
        source: ValueSource::Computed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(value: i64) -> NutrientValue {
        NutrientValue { value, estimated: false, source: ValueSource::Ciqual }
    }

    #[test]
    fn r_ali_21_ciqual_values() {
        assert_eq!(parse_ciqual("0,12", 1_000), Some((120, false)));
        assert_eq!(parse_ciqual(" 52 ", 1_000), Some((52_000, false)));
        assert_eq!(parse_ciqual("traces", 1_000), Some((0, true)));
        assert_eq!(parse_ciqual("< 0,5", 1_000), Some((500, true)));
        assert_eq!(parse_ciqual("-", 1_000), None, "inconnue, jamais zéro (R-ALI-20)");
        assert_eq!(parse_ciqual("", 1_000), None);
    }

    /// Critère d'acceptation de la spec 01 : les fibres inconnues du produit viennent de l'aliment.
    #[test]
    fn r_ali_22_product_value_falls_back_to_food() {
        let food = Some(known(2_100));
        assert_eq!(effective(None, food), food);
        let product = Some(NutrientValue { value: 1_800, estimated: false, source: ValueSource::Off });
        assert_eq!(effective(product, food), product);
        assert_eq!(effective(None, None), None);
    }

    #[test]
    fn r_ali_24_energy_from_macronutrients() {
        // 10 g de protéines, 20 g de glucides, 5 g de lipides, 3 g de fibres : 40 + 80 + 45 + 6 = 171 kcal.
        let energy = computed_energy(10_000, 20_000, 5_000, 3_000);
        assert_eq!(energy.value, 171_000);
        assert!(energy.estimated);
    }
}

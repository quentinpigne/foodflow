//! Codes-barres des produits : EAN-8, EAN-13, UPC-A, clé de contrôle vérifiée (R-ALI-41).

use crate::shared::DomainError;

/// Code-barres nettoyé (espaces retirés) et vérifié ; un UPC-A est gardé sur 12 chiffres.
pub fn validate(value: &str) -> Result<String, DomainError> {
    let digits: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if !matches!(digits.len(), 8 | 12 | 13) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DomainError::InvalidBarcode);
    }
    let values: Vec<u32> = digits.bytes().map(|b| u32::from(b - b'0')).collect();
    let (body, check) = values.split_at(values.len() - 1);
    // Poids 3 et 1 alternés en partant du chiffre le plus à droite du corps.
    let sum: u32 = body.iter().rev().enumerate().map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d }).sum();
    if (10 - sum % 10) % 10 == check[0] { Ok(digits) } else { Err(DomainError::InvalidBarcode) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_ali_41_valid_barcodes() {
        assert_eq!(validate("3017620422003").unwrap(), "3017620422003"); // EAN-13
        assert_eq!(validate("9638 5074").unwrap(), "96385074"); // EAN-8
        assert_eq!(validate("036000291452").unwrap(), "036000291452"); // UPC-A
    }

    #[test]
    fn r_ali_41_invalid_barcodes() {
        assert!(validate("3017620422004").is_err(), "mauvaise clé");
        assert!(validate("30176204220").is_err(), "longueur");
        assert!(validate("30176204220a3").is_err(), "caractère");
    }
}

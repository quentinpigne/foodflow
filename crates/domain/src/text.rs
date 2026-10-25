//! Comparaison de noms sans accents ni casse (R-ALI-10), pour l'unicité et la recherche.

/// Forme normalisée d'un nom : minuscules, sans accents, ligatures dépliées, espaces réduits.
///
/// Couvre les lettres accentuées du français et des langues voisines ; un caractère inconnu est gardé tel
/// quel en minuscules.
pub fn normalize(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut space = false;
    for c in value.trim().chars().flat_map(char::to_lowercase) {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => out.push('a'),
            'ç' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ñ' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            '’' | '‘' => out.push('\''),
            other => out.push(other),
        }
    }
    out
}

/// Vrai si chaque mot de la recherche apparaît dans le nom normalisé (ordre libre).
pub fn matches(normalized_name: &str, query: &str) -> bool {
    normalize(query).split(' ').filter(|w| !w.is_empty()).all(|word| normalized_name.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r_ali_10_names_compare_without_accents_or_case() {
        assert_eq!(normalize("  Crème  Fraîche "), "creme fraiche");
        assert_eq!(normalize("Œuf de poule"), normalize("oeuf DE poule"));
        assert_eq!(normalize("Pâtes"), "pates");
    }

    #[test]
    fn search_matches_every_word_in_any_order() {
        let name = normalize("Lait demi-écrémé, UHT");
        assert!(matches(&name, "uht lait"));
        assert!(matches(&name, "écrémé"));
        assert!(!matches(&name, "lait entier"));
    }
}

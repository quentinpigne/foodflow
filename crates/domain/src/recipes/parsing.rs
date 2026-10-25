//! Analyse d'une ligne d'ingrédient importée (R-REC-63) : quantité, unité, nom, précision.
//!
//! « 1/2 c. à soupe d'huile d'olive vierge extra » → 0,5 · cuillère à soupe · « huile d'olive vierge
//! extra ». La précision est ce qui suit une virgule ou une parenthèse (« émincé », « bien mûres »).

use rust_decimal::Decimal;
use serde::Serialize;

use crate::text::normalize;

/// Résultat de l'analyse d'une ligne.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParsedLine {
    /// Ligne d'origine, conservée sur l'ingrédient (R-REC-65).
    pub source: String,
    pub quantity: Option<Decimal>,
    /// Intervalle « 2 à 3 » : la borne basse est gardée, la haute signalée.
    pub quantity_max: Option<Decimal>,
    /// Nom de l'unité reconnue (unité générale ou unité de cuisine courante), tel qu'écrit dans la table.
    pub unit: Option<String>,
    pub name: String,
    pub note: Option<String>,
    /// Ingrédient à volonté (sel, poivre…), sans quantité (R-REC-22).
    pub to_taste: bool,
}

/// Unités reconnues : nom canonique et écritures (déjà normalisées : minuscules, sans accents).
/// Les plus longues d'abord, pour reconnaître « cuillere a soupe » avant « c ».
const UNITS: &[(&str, &[&str])] = &[
    (
        "cuillère à soupe",
        &[
            "cuilleres a soupe",
            "cuillere a soupe",
            "cuilleree a soupe",
            "cuil. a soupe",
            "cuil a soupe",
            "c. a soupe",
            "c. a s.",
            "c.a.s.",
            "c.a.s",
            "c a s",
            "cas",
            "cs",
            "c.s.",
            "tbsp",
        ],
    ),
    (
        "cuillère à café",
        &[
            "cuilleres a cafe",
            "cuillere a cafe",
            "cuilleree a cafe",
            "cuil. a cafe",
            "cuil a cafe",
            "c. a cafe",
            "c. a c.",
            "c.a.c.",
            "c.a.c",
            "c a c",
            "cac",
            "cc",
            "c.c.",
            "tsp",
        ],
    ),
    ("kg", &["kilogrammes", "kilogramme", "kilos", "kilo", "kg"]),
    ("g", &["grammes", "gramme", "gr.", "gr", "g"]),
    ("mg", &["milligrammes", "mg"]),
    ("L", &["litres", "litre", "l"]),
    ("dL", &["decilitres", "decilitre", "dl"]),
    ("cL", &["centilitres", "centilitre", "cl"]),
    ("mL", &["millilitres", "millilitre", "ml"]),
    ("verre", &["verres", "verre"]),
    ("tasse", &["tasses", "tasse", "cup", "cups"]),
    ("livre", &["livres", "livre"]),
    ("pincée", &["pincees", "pincee"]),
    ("gousse", &["gousses", "gousse"]),
    ("tranche", &["tranches", "tranche"]),
    ("sachet", &["sachets", "sachet"]),
    ("boîte", &["boites", "boite"]),
    ("brin", &["brins", "brin"]),
    ("feuille", &["feuilles", "feuille"]),
    ("botte", &["bottes", "botte"]),
    ("bouquet", &["bouquets", "bouquet"]),
    ("filet", &["filets", "filet"]),
    ("morceau", &["morceaux", "morceau"]),
    ("poignée", &["poignees", "poignee"]),
];

/// Mots qui, seuls ou en liste, désignent un ingrédient à volonté.
const TO_TASTE: &[&str] = &[
    "sel",
    "poivre",
    "sel fin",
    "gros sel",
    "poivre noir",
    "poivre du moulin",
    "fleur de sel",
    "piment d'espelette",
    "muscade",
    "noix de muscade",
];

const TO_TASTE_MARKERS: &[&str] = &["selon gout", "a volonte", "au gout", "a votre gout", "facultatif", "q.s."];

fn fraction(c: char) -> Option<Decimal> {
    Some(match c {
        '½' => Decimal::new(5, 1),
        '¼' => Decimal::new(25, 2),
        '¾' => Decimal::new(75, 2),
        '⅓' => Decimal::ONE / Decimal::from(3),
        '⅔' => Decimal::from(2) / Decimal::from(3),
        '⅛' => Decimal::new(125, 3),
        _ => return None,
    })
}

/// Entier en tête (« 12 »), pour les fractions.
fn integer(text: &str) -> Option<(Decimal, &str)> {
    let end = text.bytes().take_while(u8::is_ascii_digit).count();
    let value: Decimal = text.get(..end).filter(|d| !d.is_empty())?.parse().ok()?;
    Some((value, &text[end..]))
}

/// Lit un nombre en tête : « 2 », « 2,5 », « 2.5 », « 1/2 », « 1 1/2 », « ½ », « 1½ ». Renvoie la valeur et
/// le reste de la ligne.
fn number(text: &str) -> Option<(Decimal, &str)> {
    let text = text.trim_start();
    let bytes = text.as_bytes();
    let mut end = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if end > 0 && matches!(bytes.get(end), Some(b',' | b'.')) && bytes.get(end + 1).is_some_and(u8::is_ascii_digit) {
        end += 1 + bytes[end + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
    }
    let mut value: Option<Decimal> = if end > 0 { Some(text[..end].replace(',', ".").parse().ok()?) } else { None };
    let mut rest = &text[end..];
    let trimmed = rest.trim_start();
    if let Some(c) = trimmed.chars().next()
        && let Some(f) = fraction(c)
    {
        // Fraction unicode, collée ou non (« 1½ », « 1 ½ », « ½ »).
        value = Some(value.unwrap_or(Decimal::ZERO) + f);
        rest = &trimmed[c.len_utf8()..];
    } else if let (Some(num), Some(after)) = (value, rest.strip_prefix('/')) {
        // « 1/2 ».
        let (den, after) = integer(after).filter(|(d, _)| !d.is_zero())?;
        value = Some(num / den);
        rest = after;
    } else if let Some(whole) = value
        && let Some((num, after)) = integer(trimmed)
        && let Some((den, after)) = after.strip_prefix('/').and_then(integer)
        && !den.is_zero()
    {
        // « 1 1/2 » : un entier, puis une fraction.
        value = Some(whole + num / den);
        rest = after;
    }
    value.map(|v| (v.round_dp(3).normalize(), rest))
}

/// Unité en tête du texte (normalisé), avec la longueur consommée dans le texte d'origine.
fn unit_at(original: &str) -> Option<(&'static str, usize)> {
    let lower = normalize(original);
    for (canonical, forms) in UNITS {
        for form in *forms {
            if let Some(after) = lower.strip_prefix(form) {
                // L'unité doit être un mot entier : suivie d'un espace, d'une apostrophe ou de la fin.
                if after.is_empty() || after.starts_with([' ', '\'', '’']) {
                    // `normalize` ne change pas le nombre de caractères sauf œ/æ, absents des unités.
                    let consumed: usize = original.chars().take(form.chars().count()).map(char::len_utf8).sum();
                    return Some((canonical, consumed));
                }
            }
        }
    }
    None
}

/// Retire « de », « d' », « des », « du » en tête du nom.
fn strip_article(text: &str) -> &str {
    let t = text.trim_start();
    for prefix in ["de la ", "de l'", "de l’", "des ", "du ", "de ", "d'", "d’"] {
        if t.len() >= prefix.len() && t.is_char_boundary(prefix.len()) && t[..prefix.len()].eq_ignore_ascii_case(prefix)
        {
            return t[prefix.len()..].trim_start();
        }
    }
    t
}

fn is_to_taste(name: &str) -> bool {
    let n = normalize(name);
    TO_TASTE.iter().any(|t| normalize(t) == n) || TO_TASTE_MARKERS.iter().any(|m| n.contains(m))
}

/// Analyse une ligne ; « Sel, poivre » donne deux ingrédients à volonté (R-REC-63).
pub fn parse_lines(line: &str) -> Vec<ParsedLine> {
    let source = line.trim();
    // Une liste d'ingrédients à volonté sans aucun chiffre : un ingrédient par élément.
    if !source.chars().any(|c| c.is_ascii_digit()) {
        let parts: Vec<&str> =
            source.split([',', ';']).flat_map(|p| p.split(" et ")).map(str::trim).filter(|p| !p.is_empty()).collect();
        if parts.len() > 1 && parts.iter().all(|p| is_to_taste(p)) {
            return parts.iter().map(|p| to_taste_line(source, p)).collect();
        }
    }
    vec![parse_line(source)]
}

fn to_taste_line(source: &str, name: &str) -> ParsedLine {
    let mut chars = name.chars();
    let name = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
    ParsedLine {
        source: source.to_owned(),
        quantity: None,
        quantity_max: None,
        unit: None,
        name,
        note: None,
        to_taste: true,
    }
}

/// Analyse une ligne d'ingrédient.
pub fn parse_line(line: &str) -> ParsedLine {
    let source = line.trim().to_owned();
    let mut rest: &str = &source;
    let mut quantity = None;
    let mut quantity_max = None;
    if let Some((q, after)) = number(rest) {
        quantity = Some(q);
        rest = after;
        // Intervalle « 2 à 3 », « 2-3 », « 2 ou 3 ».
        let t = rest.trim_start();
        for sep in ["à ", "a ", "-", "ou "] {
            if let Some(after_sep) = t.strip_prefix(sep)
                && let Some((max, after)) = number(after_sep)
            {
                quantity_max = Some(max);
                rest = after;
                break;
            }
        }
    }
    let mut unit = None;
    if let Some((u, consumed)) = unit_at(rest.trim_start()) {
        unit = Some(u.to_owned());
        rest = &rest.trim_start()[consumed..];
    }
    let rest = strip_article(rest.trim_start_matches(['.', ' ']));
    // Précision : après une virgule, ou entre parenthèses.
    let (name, note) = match rest.find([',', '(']) {
        Some(i) => {
            let note = rest[i..].trim_matches([',', '(', ')', ' ']).replace(['(', ')'], "");
            (rest[..i].trim(), Some(note.trim().to_owned()).filter(|n| !n.is_empty()))
        }
        None => (rest.trim(), None),
    };
    let to_taste =
        quantity.is_none() && is_to_taste(name) || quantity.is_none() && note.as_deref().is_some_and(is_to_taste);
    ParsedLine { source: source.clone(), quantity, quantity_max, unit, name: name.to_owned(), note, to_taste }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    /// Critère d'acceptation de la spec 03.
    #[test]
    fn r_rec_63_spoon_fraction_and_note() {
        let line = parse_line("1/2 c. à soupe d'huile d'olive, vierge extra");
        assert_eq!(line.quantity, Some(d("0.5")));
        assert_eq!(line.unit.as_deref(), Some("cuillère à soupe"));
        assert_eq!(line.name, "huile d'olive");
        assert_eq!(line.note.as_deref(), Some("vierge extra"));
    }

    #[test]
    fn r_rec_63_quantities() {
        assert_eq!(parse_line("200 g de farine").quantity, Some(d("200")));
        assert_eq!(parse_line("200 g de farine").unit.as_deref(), Some("g"));
        assert_eq!(parse_line("200 g de farine").name, "farine");
        assert_eq!(parse_line("2,5 kg de pommes de terre").quantity, Some(d("2.5")));
        assert_eq!(parse_line("1 1/2 L de lait").quantity, Some(d("1.5")));
        assert_eq!(parse_line("½ citron").quantity, Some(d("0.5")));
        assert_eq!(parse_line("1½ tasse de riz").quantity, Some(d("1.5")));
        let eggs = parse_line("3 œufs");
        assert_eq!((eggs.quantity, eggs.unit, eggs.name.as_str()), (Some(d("3")), None, "œufs"));
        let range = parse_line("2 à 3 gousses d'ail");
        assert_eq!((range.quantity, range.quantity_max), (Some(d("2")), Some(d("3"))));
        assert_eq!(range.unit.as_deref(), Some("gousse"));
        assert_eq!(range.name, "ail");
        let cs = parse_line("2 cs de sucre");
        assert_eq!(cs.unit.as_deref(), Some("cuillère à soupe"));
        let cl = parse_line("20 cl de crème liquide (entière)");
        assert_eq!(
            (cl.unit.as_deref(), cl.name.as_str(), cl.note.as_deref()),
            (Some("cL"), "crème liquide", Some("entière"))
        );
    }

    #[test]
    fn r_rec_63_ripe_note_and_no_false_unit() {
        let bananas = parse_line("4 bananes, bien mûres");
        assert_eq!((bananas.name.as_str(), bananas.note.as_deref()), ("bananes", Some("bien mûres")));
        // « gruyère » ne commence pas par l'unité « g ».
        let cheese = parse_line("100 gruyère râpé");
        assert_eq!((cheese.unit, cheese.name.as_str()), (None, "gruyère râpé"));
        // « lardons » ne commence pas par l'unité « l ».
        assert_eq!(parse_line("200 lardons").unit, None);
    }

    #[test]
    fn r_rec_63_salt_and_pepper_split_to_taste() {
        let lines = parse_lines("Sel, poivre");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.to_taste && l.quantity.is_none()));
        assert_eq!((lines[0].name.as_str(), lines[1].name.as_str()), ("Sel", "Poivre"));
        assert!(parse_line("Persil (selon goût)").to_taste);
        assert!(!parse_line("1 pincée de sel").to_taste);
        assert_eq!(parse_line("1 pincée de sel").unit.as_deref(), Some("pincée"));
    }
}

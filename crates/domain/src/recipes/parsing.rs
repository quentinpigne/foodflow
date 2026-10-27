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
    ("rouleau", &["rouleaux", "rouleau"]),
    ("cube", &["cubes", "cube"]),
    ("pavé", &["paves", "pave"]),
    ("dos", &["dos"]),
    ("branche", &["branches", "branche"]),
    ("boule", &["boules", "boule"]),
    ("barquette", &["barquettes", "barquette"]),
    ("pot", &["pots", "pot"]),
    ("bocal", &["bocaux", "bocal"]),
    ("bouteille", &["bouteilles", "bouteille"]),
];

/// Unités qui sont des contenants ou des parts (« 1 bouquet de persil ») : reconnues seulement devant « de »,
/// pour que « 1 bouquet garni » ou « 2 boules de mozzarella » gardent leur sens.
const CONTAINERS: &[&str] = &[
    "verre",
    "tasse",
    "pincée",
    "gousse",
    "tranche",
    "sachet",
    "boîte",
    "brin",
    "feuille",
    "botte",
    "bouquet",
    "filet",
    "morceau",
    "poignée",
    "rouleau",
    "cube",
    "pavé",
    "dos",
    "branche",
    "boule",
    "barquette",
    "pot",
    "bocal",
];

/// Adjectifs placés devant une unité (« 1 petite boîte de maïs », « 1 bonne pincée de sel »).
const UNIT_ADJECTIVES: &[&str] = &[
    "petits ", "petites ", "petit ", "petite ", "grands ", "grandes ", "grand ", "grande ", "gros ", "grosses ",
    "grosse ", "bons ", "bonnes ", "bon ", "bonne ", "belles ", "belle ", "beaux ", "beau ", "demi-", "demi ",
];

/// Quantités sans chiffre en tête de ligne : « quelques feuilles de basilic », « un peu de sel ».
const VAGUE: &[&str] = &["quelques ", "un peu d'", "un peu d’", "un peu de "];

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
                // L'unité doit être un mot entier : suivie d'un espace, d'une apostrophe ou de la fin ; un
                // contenant, suivi de « de ».
                let whole = after.is_empty() || after.starts_with([' ', '\'', '’']);
                let of = ["de ", "d'", "d’", "du ", "des "].iter().any(|p| after.trim_start().starts_with(p));
                if whole && (!CONTAINERS.contains(canonical) || of) {
                    // `normalize` ne change pas le nombre de caractères sauf œ/æ, absents des unités.
                    let consumed: usize = original.chars().take(form.chars().count()).map(char::len_utf8).sum();
                    return Some((canonical, consumed));
                }
            }
        }
    }
    None
}

/// Réécrit une ligne d'un site qui omet le « de » après un contenant et écrit « unité » pour une pièce
/// (« 1 sachet levure chimique », « 3 unité pomme golden ») dans la forme courante (« 1 sachet de levure
/// chimique », « 3 pomme golden »), avant son analyse.
pub fn with_container_de(line: &str) -> String {
    let Some((_, rest)) = number(line) else { return line.to_owned() };
    let head = line[..line.len() - rest.len()].trim_end();
    let rest = rest.trim_start();
    let lower = normalize(rest);
    // Longueur, dans la ligne d'origine, d'un début reconnu sur la forme normalisée (sans œ ni æ) : les
    // accents retirés changent le nombre d'octets, pas celui des caractères.
    let bytes = |chars: usize| -> usize { rest.chars().take(chars).map(char::len_utf8).sum() };
    if let Some(word) = ["unites ", "unite "].iter().find(|u| lower.starts_with(*u)) {
        return format!("{head} {}", &rest[bytes(word.chars().count())..]);
    }
    for forms in UNITS.iter().filter(|(c, _)| CONTAINERS.contains(c)).map(|(_, forms)| forms) {
        for form in *forms {
            let Some(after) = lower.strip_prefix(form).and_then(|a| a.strip_prefix(' ')) else { continue };
            let linked = ["de ", "d'", "d’", "du ", "des "].iter().any(|p| after.starts_with(p));
            if linked || after.is_empty() {
                return line.to_owned();
            }
            let unit_end = bytes(form.chars().count() + 1);
            return format!("{head} {}de {}", &rest[..unit_end], &rest[unit_end..]);
        }
    }
    line.to_owned()
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
    // Deux ingrédients sur une ligne : « 1 œuf entier + 1 blanc d'œuf ».
    if source.contains(" + ") {
        return source
            .split(" + ")
            .filter(|p| !p.trim().is_empty())
            .map(|p| ParsedLine { source: source.to_owned(), ..parse_line(p) })
            .collect();
    }
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
/// Retire un préfixe sans tenir compte de la casse.
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    (text.len() >= prefix.len()
        && text.is_char_boundary(prefix.len())
        && text[..prefix.len()].eq_ignore_ascii_case(prefix))
    .then(|| &text[prefix.len()..])
}

pub fn parse_line(line: &str) -> ParsedLine {
    let source = line.trim().to_owned();
    let mut rest: &str = &source;
    // Article ou quantité vague en tête : « le jus d'un citron », « quelques feuilles de menthe ».
    for prefix in ["le ", "la ", "les ", "l'", "l’"].iter().chain(VAGUE) {
        if let Some(after) = strip_prefix_ci(rest, prefix) {
            rest = after;
            break;
        }
    }
    let mut quantity = None;
    let mut quantity_max = None;
    // « un », « une » valent 1 devant une unité ou un nom (« une pincée de sel »).
    for one in ["une ", "un "] {
        if let Some(after) = strip_prefix_ci(rest, one) {
            quantity = Some(Decimal::ONE);
            rest = after;
            break;
        }
    }
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
    let trimmed = rest.trim_start();
    let adjective = UNIT_ADJECTIVES.iter().find_map(|a| strip_prefix_ci(trimmed, a));
    for text in adjective.into_iter().chain([trimmed]) {
        if let Some((u, consumed)) = unit_at(text) {
            unit = Some(u.to_owned());
            rest = &text[consumed..];
            break;
        }
    }
    let mut rest = strip_article(rest.trim_start_matches(['.', ' ']));
    // Un contenant après une mesure : « 600 g de dos de cabillaud », « 200 g de pavés de saumon ».
    if unit.is_some()
        && let Some((u, consumed)) = unit_at(rest)
        && CONTAINERS.contains(&u)
    {
        rest = strip_article(&rest[consumed..]);
    }
    // « jus d'un citron » → « jus de citron ».
    let rest = rest.replacen(" d'un ", " de ", 1).replacen(" d'une ", " de ", 1).replacen(" d’un ", " de ", 1);
    // Alternative (« 4 citrons jaunes ou 8 verts ») ou usage (« chocolat pour la pâte ») : la suite va en
    // précision. « pain pour hamburger » garde son nom.
    let cut = [" ou ", " pour la ", " pour le ", " pour les ", " pour l'", " pour l’"]
        .iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .filter(|&i| !rest[..i].contains(['(', ',']));
    let (rest, alternative) = match cut {
        Some(i) => (rest[..i].to_owned(), Some(rest[i + 1..].trim().to_owned())),
        None => (rest, None),
    };
    let rest = rest.as_str();
    // Précision : après une virgule, ou entre parenthèses.
    let (name, note) = match rest.find([',', '(']) {
        Some(i) => {
            let note = rest[i..].trim_matches([',', '(', ')', ' ']).replace(['(', ')'], "");
            (rest[..i].trim(), Some(note.trim().to_owned()).filter(|n| !n.is_empty()))
        }
        None => (rest.trim(), None),
    };
    let note = match (note, alternative) {
        (Some(n), Some(a)) => Some(format!("{n}, {a}")),
        (n, a) => n.or(a),
    };
    let to_taste =
        quantity.is_none() && is_to_taste(name) || quantity.is_none() && note.as_deref().is_some_and(is_to_taste);
    ParsedLine { source: source.clone(), quantity, quantity_max, unit, name: name.to_owned(), note, to_taste }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les sites qui omettent le « de » après un contenant, et écrivent « unité » pour une pièce (R-REC-63).
    #[test]
    fn r_rec_63_lines_without_de() {
        assert_eq!(with_container_de("1 sachet levure chimique"), "1 sachet de levure chimique");
        assert_eq!(with_container_de("2 cubes bouillon de volaille"), "2 cubes de bouillon de volaille");
        assert_eq!(with_container_de("1 sachet sucre vanillé"), "1 sachet de sucre vanillé", "accent après l'unité");
        assert_eq!(with_container_de("1 boîte maïs"), "1 boîte de maïs", "accent dans l'unité");
        assert_eq!(with_container_de("3 unité pomme golden"), "3 pomme golden");
        assert_eq!(with_container_de("2 unités jaune d'œuf"), "2 jaune d'œuf");
        assert_eq!(with_container_de("1 pincée de sel"), "1 pincée de sel", "déjà lié");
        assert_eq!(with_container_de("50 g sucre"), "50 g sucre", "une mesure se lit sans « de »");
        assert_eq!(with_container_de("sel et poivre"), "sel et poivre");
        let parsed = parse_line(&with_container_de("1 pincée sel"));
        assert_eq!((parsed.unit.as_deref(), parsed.name.as_str()), (Some("pincée"), "sel"));
        let piece = parse_line(&with_container_de("3 unité poire passe-crassane ou comice"));
        assert_eq!((piece.unit, piece.name.as_str()), (None, "poire passe-crassane"));
    }

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

    /// Contenants, quantités sans chiffre, alternatives (R-REC-63).
    #[test]
    fn r_rec_63_containers_vague_and_alternatives() {
        let garni = parse_line("1 bouquet garni");
        assert_eq!((garni.unit, garni.name.as_str()), (None, "bouquet garni"), "« bouquet » sans « de » est le nom");
        let parsley = parse_line("1 bouquet de persil");
        assert_eq!((parsley.unit.as_deref(), parsley.name.as_str()), (Some("bouquet"), "persil"));
        let corn = parse_line("1 petite boîte de maïs");
        assert_eq!((corn.unit.as_deref(), corn.name.as_str()), (Some("boîte"), "maïs"));
        let pastry = parse_line("1 rouleau de pâte feuilletée");
        assert_eq!((pastry.unit.as_deref(), pastry.name.as_str()), (Some("rouleau"), "pâte feuilletée"));
        let basil = parse_line("quelques feuilles de basilic");
        assert_eq!((basil.quantity, basil.unit.as_deref(), basil.name.as_str()), (None, Some("feuille"), "basilic"));
        let juice = parse_line("le jus d'un citron");
        assert_eq!(juice.name, "jus de citron");
        let pinch = parse_line("une pincée de sel");
        assert_eq!((pinch.quantity, pinch.unit.as_deref()), (Some(Decimal::ONE), Some("pincée")));
        let chocolate = parse_line("60 g de chocolat pour la pâte");
        assert_eq!((chocolate.name.as_str(), chocolate.note.as_deref()), ("chocolat", Some("pour la pâte")));
        assert_eq!(parse_line("pain pour hamburger").name, "pain pour hamburger");
        let wine = parse_line("1 bouteille de vin rouge");
        assert_eq!((wine.unit.as_deref(), wine.name.as_str()), (Some("bouteille"), "vin rouge"));
        let eggs = parse_lines("1 oeuf entier + un blanc d'oeuf");
        assert_eq!(eggs.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["oeuf entier", "blanc d'oeuf"]);
        assert!(eggs.iter().all(|l| l.source == "1 oeuf entier + un blanc d'oeuf"));
        let lemons = parse_line("4 citrons jaunes ou 8 verts");
        assert_eq!((lemons.name.as_str(), lemons.note.as_deref()), ("citrons jaunes", Some("ou 8 verts")));
        let mozzarella = parse_line("2 boules de mozzarella");
        assert_eq!((mozzarella.unit.as_deref(), mozzarella.name.as_str()), (Some("boule"), "mozzarella"));
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

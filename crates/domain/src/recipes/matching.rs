//! Rattachement d'un nom d'ingrédient importé à un aliment, une famille ou une recette (R-REC-64, R-REC-66).
//!
//! Ordre : rattachement mémorisé (ceux de l'utilisateur, puis le dictionnaire de départ, fusionnés par le
//! serveur), nom ou alias identique, recherche approchée sur le nom principal. Déterministe : à égalité, le
//! nom le plus court, puis l'ordre alphabétique, l'emportent.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::text::normalize;

use super::Target;

/// Qualité d'une proposition de rattachement.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    None,
    Low,
    Medium,
    High,
}

/// Mots sans valeur pour le rattachement.
const STOPWORDS: &[&str] =
    &["de", "du", "des", "d", "la", "le", "les", "l", "a", "au", "aux", "en", "et", "un", "une", "ou", "pour"];

/// Mots qui décrivent une préparation : un aliment cru est préféré (R-ALI-12 : quantités telles qu'achetées).
const PREPARED: &[&str] =
    &["cuit", "frit", "appertise", "egoutte", "rissole", "grille", "braise", "roti", "bouilli", "reconstitue"];

/// Formes transformées d'un aliment (purée, conserve, jus…) : écartées si la recette ne les demande pas.
const PROCESSED: &[&str] = &[
    "puree",
    "preemballe",
    "surgele",
    "deshydrate",
    "nectar",
    "jus",
    "sirop",
    "confit",
    "compote",
    "sauce",
    "soupe",
    "sec",
    "seche",
    "pane",
    "farci",
    "preleve",
    "poudre",
    "concentre",
];

/// Précisions d'un aliment Ciqual qui ne le distinguent pas (« Courgette, pulpe et peau, crue ») : elles ne
/// comptent pas comme des mots en trop.
const NEUTRAL: &[&str] =
    &["cru", "crue", "pulpe", "peau", "frais", "fraiche", "nature", "standard", "courant", "moyen", "aliment"];

/// Mots d'une recette qui précisent l'ingrédient sans le désigner (« beurre fondu », « bananes bien mûres ») :
/// ils départagent les candidats sans être exigés.
const DESCRIPTIVE: &[&str] = &[
    "fondu",
    "fondue",
    "mou",
    "molle",
    "ramolli",
    "pommade",
    "tiede",
    "froid",
    "froide",
    "chaud",
    "chaude",
    "battu",
    "battue",
    "hache",
    "hachee",
    "emince",
    "emincee",
    "cisele",
    "ciselee",
    "concasse",
    "concassee",
    "ecrase",
    "ecrasee",
    "coupe",
    "coupee",
    "pele",
    "pelee",
    "epepine",
    "epepinee",
    "lave",
    "lavee",
    "entier",
    "entiere",
    "frais",
    "fraiche",
    "sec",
    "seche",
    "mur",
    "mure",
    "bien",
    "gros",
    "grosse",
    "petit",
    "petite",
    "moyen",
    "moyenne",
    "grand",
    "grande",
    "beau",
    "belle",
    "bon",
    "bonne",
    "maison",
    "bio",
    "nature",
    "effile",
    "effilee",
    "rape",
    "rapee",
    "fume",
    "fumee",
    "vert",
    "verte",
    "rouge",
    "jaune",
    "blanc",
    "blanche",
    "noir",
    "noire",
    "doux",
    "douce",
    "extra",
    "fin",
    "fine",
    "decortique",
    "decortiquee",
    "denoyaute",
    "denoyautee",
    "non",
    "traite",
    "quelque",
    "morceau",
    "cuit",
    "cuite",
];

/// Singulier approché : appliqué de la même façon aux deux noms comparés, il suffit qu'il soit cohérent.
/// « x » ne marque le pluriel qu'après « au », « eu », « ou » (poireaux, choux), pas dans « noix ».
fn singular(word: &str) -> String {
    let long = word.chars().count() > 3;
    let plural_x = word.ends_with("aux") || word.ends_with("eux") || word.ends_with("oux");
    let plural_s = word.ends_with('s') && !word.ends_with("ss");
    if long && (plural_s || plural_x) { word[..word.len() - 1].to_owned() } else { word.to_owned() }
}

/// Masculin approché des adjectifs (« cuite », « hachée », « fraîche ») : appliqué aux deux noms comparés,
/// comme le singulier.
fn masculine(word: String) -> String {
    match word.as_str() {
        "fraiche" => "frais".to_owned(),
        "seche" => "sec".to_owned(),
        "douce" => "doux".to_owned(),
        "blanche" => "blanc".to_owned(),
        "crue" => "cru".to_owned(),
        "cuite" => "cuit".to_owned(),
        "entiere" => "entier".to_owned(),
        "bouillie" => "bouilli".to_owned(),
        "rotie" => "roti".to_owned(),
        "frite" => "frit".to_owned(),
        _ if word.ends_with("ee") && word.chars().count() > 4 => word[..word.len() - 1].to_owned(),
        _ => word,
    }
}

/// Forme comparée d'un mot : singulier, masculin, et de nouveau singulier (« fraîches » → « fraîche » →
/// « frais » → la même forme que « frais »).
fn stem(word: &str) -> String {
    singular(&masculine(singular(word)))
}

/// Mots significatifs d'un nom : minuscules, sans accents, au singulier et au masculin, sans mots vides ni
/// ponctuation.
pub fn words(name: &str) -> Vec<String> {
    normalize(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOPWORDS.contains(w))
        .map(stem)
        .collect()
}

/// Le mot (déjà réduit par `words`) fait-il partie de la liste ?
fn listed(list: &[&str], word: &str) -> bool {
    list.iter().any(|l| stem(l) == word)
}

/// Clé d'un nom pour la mémoire des rattachements (R-REC-66) : ses mots significatifs.
pub fn key(name: &str) -> String {
    words(name).join(" ")
}

/// Mots exigés et mots qui précisent seulement (DESCRIPTIVE) ; sans mot exigé, tous le sont.
fn split_query(query: &[String]) -> (Vec<String>, Vec<String>) {
    let (optional, core): (Vec<String>, Vec<String>) = query.iter().cloned().partition(|w| listed(DESCRIPTIVE, w));
    if core.is_empty() { (optional, Vec::new()) } else { (core, optional) }
}

/// Tête d'un nom Ciqual : ce qui précède la première virgule (« Avocat » dans « Avocat, pulpe, cru »).
fn head(name: &str) -> Vec<String> {
    words(name.split(',').next().unwrap_or(name))
}

/// Un aliment, une famille ou une recette candidats, sous un de leurs noms (nom ou alias).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub target: Target,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposal {
    pub target: Option<Target>,
    pub confidence: Confidence,
}

fn rank(target: &Target) -> u8 {
    // À égalité : un aliment d'abord, puis une famille, puis une recette.
    match target {
        Target::Food(_) => 0,
        Target::Group(_) => 1,
        Target::Recipe(_) => 2,
    }
}

/// Propose une cible pour un nom d'ingrédient (R-REC-64).
pub fn propose(name: &str, memory: &HashMap<String, Target>, candidates: &[Candidate]) -> Proposal {
    let query = words(name);
    if query.is_empty() {
        return Proposal { target: None, confidence: Confidence::None };
    }
    let query_key = query.join(" ");
    if let Some(target) = memory.get(&query_key) {
        return Proposal { target: Some(*target), confidence: Confidence::High };
    }
    let (core, optional) = split_query(&query);
    // Clé de tri, la plus petite l'emporte : confiance décroissante, précisions retrouvées décroissantes, forme
    // transformée non demandée, mots en trop au total, puis dans la tête du nom, rang, longueur, nom.
    type Key = (std::cmp::Reverse<Confidence>, std::cmp::Reverse<usize>, bool, usize, usize, u8, usize, String);
    let mut best: Option<(Key, Confidence, Target)> = None;
    for candidate in candidates {
        let cand = words(&candidate.name);
        if cand.is_empty() {
            continue;
        }
        let head = head(&candidate.name);
        // Le nom principal de l'ingrédient doit être celui du candidat : « avocat » n'est pas « Huile d'avocat ».
        // (« petits pois » : le premier mot de la recette compte aussi, même s'il est un qualificatif.)
        let same_noun = head.first().is_some_and(|h| Some(h) == core.first() || Some(h) == query.first());
        let confidence = if cand == query {
            Confidence::High
        } else if core.iter().all(|w| cand.contains(w)) {
            if same_noun { Confidence::Medium } else { Confidence::Low }
        } else if same_noun && core.iter().any(|w| w.chars().count() >= 3 && cand.contains(w)) {
            Confidence::Low
        } else {
            continue;
        };
        let extra = |list: &[String]| list.iter().filter(|w| !query.contains(w) && !listed(NEUTRAL, w)).count();
        let found = optional.iter().filter(|w| cand.contains(w)).count();
        let transformed = cand.iter().any(|w| (listed(PREPARED, w) || listed(PROCESSED, w)) && !query.contains(w));
        let key: Key = (
            std::cmp::Reverse(confidence),
            std::cmp::Reverse(found),
            transformed,
            extra(&cand),
            extra(&head),
            rank(&candidate.target),
            candidate.name.len(),
            candidate.name.clone(),
        );
        if best.as_ref().is_none_or(|(current, ..)| key < *current) {
            best = Some((key, confidence, candidate.target));
        }
    }
    match best {
        Some((_, confidence, target)) => Proposal { target: Some(target), confidence },
        None => Proposal { target: None, confidence: Confidence::None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::ids::{FoodGroupId, FoodId};

    fn food(name: &str) -> (Candidate, FoodId) {
        let id = FoodId::new();
        (Candidate { target: Target::Food(id), name: name.into() }, id)
    }

    #[test]
    fn words_are_singular_and_without_stopwords() {
        assert_eq!(words("Gousses d'ail"), vec!["gousse", "ail"]);
        assert_eq!(key("Pommes de terre"), "pomme terre");
        assert_eq!(key("Noix"), "noix", "« noix » ne perd pas son x");
    }

    #[test]
    fn r_rec_64_exact_then_approximate_raw_first() {
        let (zucchini_raw, raw_id) = food("Courgette, crue");
        let (zucchini_cooked, _) = food("Courgette, cuite");
        let (zucchini, plain_id) = food("Courgette");
        let candidates = vec![zucchini_cooked.clone(), zucchini_raw.clone()];
        let found = propose("courgettes", &HashMap::new(), &candidates);
        assert_eq!(found, Proposal { target: Some(Target::Food(raw_id)), confidence: Confidence::Medium });
        let mut with_exact = candidates.clone();
        with_exact.push(zucchini);
        assert_eq!(propose("Courgettes", &HashMap::new(), &with_exact).target, Some(Target::Food(plain_id)));
        assert_eq!(propose("Courgettes", &HashMap::new(), &with_exact).confidence, Confidence::High);
    }

    /// Critère d'acceptation : un rattachement validé est proposé en priorité, en confiance haute.
    #[test]
    fn r_rec_66_memory_wins() {
        let garlic = Target::Food(FoodId::new());
        let memory = HashMap::from([(key("gousses d'ail"), garlic)]);
        assert_eq!(
            propose("gousses d'ail", &memory, &[]),
            Proposal { target: Some(garlic), confidence: Confidence::High }
        );
    }

    #[test]
    fn low_and_none() {
        let (cream, cream_id) = food("Crème de lait, 30% MG, épaisse, rayon frais");
        let group = FoodGroupId::new();
        let grated = Candidate { target: Target::Group(group), name: "Fromages râpés".into() };
        let candidates = vec![cream, grated];
        // Même nom principal, une précision absente : proposé, en confiance basse.
        assert_eq!(
            propose("crème fleurette", &HashMap::new(), &candidates),
            Proposal { target: Some(Target::Food(cream_id)), confidence: Confidence::Low }
        );
        // Un qualificatif en commun ne suffit pas : « emmental râpé » n'est pas un fromage râpé quelconque.
        assert_eq!(propose("emmental râpé", &HashMap::new(), &candidates).target, None);
        assert_eq!(
            propose("xyz", &HashMap::new(), &candidates),
            Proposal { target: None, confidence: Confidence::None }
        );
    }

    /// Le nom principal l'emporte sur un plat ou un produit qui le contient ; la forme crue sur la purée.
    #[test]
    fn r_rec_64_main_noun_and_raw_form() {
        let (avocado, avocado_id) = food("Avocat, pulpe, cru");
        let (oil, _) = food("Huile d'avocat");
        let (pie, _) = food("Tarte au citron");
        let (lemon, lemon_id) = food("Citron, pulpe, cru");
        let (puree, _) = food("Courgette, purée");
        let (zucchini, zucchini_id) = food("Courgette, pulpe et peau, crue");
        let candidates = vec![oil, avocado, pie, lemon, puree, zucchini];
        let none = HashMap::new();
        assert_eq!(propose("avocats", &none, &candidates).target, Some(Target::Food(avocado_id)));
        assert_eq!(propose("citron", &none, &candidates).target, Some(Target::Food(lemon_id)));
        assert_eq!(propose("courgettes", &none, &candidates).target, Some(Target::Food(zucchini_id)));
    }

    /// Une précision de la recette départage sans être exigée : « beurre fondu » reste du beurre.
    #[test]
    fn r_rec_64_descriptive_words_are_optional() {
        let (butter, butter_id) = food("Beurre à 82% MG, doux");
        let (whole, whole_id) = food("Lait entier, UHT");
        let (semi, _) = food("Lait demi-écrémé, UHT");
        let candidates = vec![butter, semi, whole];
        let none = HashMap::new();
        assert_eq!(propose("beurre fondu", &none, &candidates).target, Some(Target::Food(butter_id)));
        assert_eq!(propose("lait entier", &none, &candidates).target, Some(Target::Food(whole_id)));
    }
}

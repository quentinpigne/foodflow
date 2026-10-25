//! Rattachement d'un nom d'ingrédient importé à un aliment, une famille ou une recette (R-REC-64, R-REC-66).
//!
//! Ordre : rattachement mémorisé, nom ou alias identique, recherche approchée. Déterministe : à égalité,
//! le nom le plus court, puis l'ordre alphabétique, l'emportent.

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

/// Singulier approché : appliqué de la même façon aux deux noms comparés, il suffit qu'il soit cohérent.
/// « x » ne marque le pluriel qu'après « au », « eu », « ou » (poireaux, choux), pas dans « noix ».
fn singular(word: &str) -> String {
    let long = word.chars().count() > 3;
    let plural_x = word.ends_with("aux") || word.ends_with("eux") || word.ends_with("oux");
    let plural_s = word.ends_with('s') && !word.ends_with("ss");
    if long && (plural_s || plural_x) { word[..word.len() - 1].to_owned() } else { word.to_owned() }
}

/// Mots significatifs d'un nom : minuscules, sans accents, au singulier, sans mots vides ni ponctuation.
pub fn words(name: &str) -> Vec<String> {
    normalize(name)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !STOPWORDS.contains(w))
        .map(singular)
        .collect()
}

/// Clé d'un nom pour la mémoire des rattachements (R-REC-66) : ses mots significatifs.
pub fn key(name: &str) -> String {
    words(name).join(" ")
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
    // (confiance, mots en trop, préparé, rang, longueur, nom) : le plus petit l'emporte, confiance décroissante.
    let mut best: Option<(Confidence, usize, bool, u8, usize, String, Target)> = None;
    for candidate in candidates {
        let cand = words(&candidate.name);
        if cand.is_empty() {
            continue;
        }
        let confidence = if cand == query {
            Confidence::High
        } else if query.iter().all(|w| cand.contains(w)) {
            Confidence::Medium
        } else if query.iter().any(|w| w.chars().count() >= 4 && cand.contains(w)) {
            Confidence::Low
        } else {
            continue;
        };
        let extra = cand.iter().filter(|w| !query.contains(w)).count();
        let prepared = cand.iter().any(|w| PREPARED.contains(&w.as_str()));
        let entry = (
            confidence,
            extra,
            prepared,
            rank(&candidate.target),
            candidate.name.len(),
            candidate.name.clone(),
            candidate.target,
        );
        let better = match &best {
            None => true,
            Some(current) => {
                (std::cmp::Reverse(entry.0), entry.1, entry.2, entry.3, entry.4, &entry.5)
                    < (std::cmp::Reverse(current.0), current.1, current.2, current.3, current.4, &current.5)
            }
        };
        if better {
            best = Some(entry);
        }
    }
    match best {
        Some((confidence, .., target)) => Proposal { target: Some(target), confidence },
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
        let group = FoodGroupId::new();
        let candidates = vec![Candidate { target: Target::Group(group), name: "Fromages râpés".into() }];
        assert_eq!(propose("emmental râpé", &HashMap::new(), &candidates).confidence, Confidence::Low);
        assert_eq!(
            propose("xyz", &HashMap::new(), &candidates),
            Proposal { target: None, confidence: Confidence::None }
        );
    }
}

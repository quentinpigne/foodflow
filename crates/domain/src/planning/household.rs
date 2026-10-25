//! Foyer et convives (ADR-0004, R-PLA-2, R-PLA-11).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::recipes::Target;
use crate::shared::Ratio;
use crate::shared::ids::MemberId;

/// Niveau d'une préférence d'un membre (ADR-0004).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreferenceLevel {
    Loves,
    Likes,
    Dislikes,
    /// La génération exclut ce qu'un convive refuse ; la saisie manuelle le signale (R-PLA-50).
    Refuses,
}

/// Un membre du foyer, tel que la planification le voit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: MemberId,
    pub name: String,
    pub adult_equivalent: Ratio,
    /// Un membre parti du foyer reste dans l'historique, mais n'est plus convive par défaut.
    pub active: bool,
}

/// Un convive d'un repas : un membre, ou un invité (`member` vide).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diner {
    pub member: Option<MemberId>,
    pub adult_equivalent: Ratio,
}

/// Équivalents adulte d'un repas : la somme des coefficients de ses convives (R-PLA-11).
pub fn adult_equivalents(diners: &[Diner]) -> Ratio {
    Ratio::from_thousandths(diners.iter().map(|d| d.adult_equivalent.thousandths()).sum())
}

/// Convives par défaut d'un créneau, un jour de la semaine : les membres actifs, sauf ceux que la semaine
/// type déclare absents (R-PLA-2, R-PLA-5).
pub fn default_diners(members: &[Member], absent: &HashSet<MemberId>) -> Vec<Diner> {
    members
        .iter()
        .filter(|m| m.active && !absent.contains(&m.id))
        .map(|m| Diner { member: Some(m.id), adult_equivalent: m.adult_equivalent })
        .collect()
}

/// Une préférence : un membre, une cible (aliment, famille ou recette), un niveau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preference {
    pub member: MemberId,
    pub target: Target,
    pub level: PreferenceLevel,
}

/// Membres présents qui refusent une des cibles (les aliments, familles et recettes d'un élément de repas,
/// sous-recettes comprises).
pub fn refusals(diners: &[Diner], preferences: &[Preference], targets: &HashSet<Target>) -> Vec<MemberId> {
    let present: HashSet<MemberId> = diners.iter().filter_map(|d| d.member).collect();
    let mut out: Vec<MemberId> = preferences
        .iter()
        .filter(|p| p.level == PreferenceLevel::Refuses && present.contains(&p.member) && targets.contains(&p.target))
        .map(|p| p.member)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::ids::FoodId;

    fn member(name: &str, coefficient: i64) -> Member {
        Member {
            id: MemberId::new(),
            name: name.into(),
            adult_equivalent: Ratio::from_thousandths(coefficient),
            active: true,
        }
    }

    /// Deux adultes et un enfant à 0,6, plus un invité : 3,6 équivalents adulte (R-PLA-11, ADR-0004).
    #[test]
    fn r_pla_11_adult_equivalents() {
        let members = vec![member("Alex", 1_000), member("Sam", 1_000), member("Lou", 600)];
        let mut diners = default_diners(&members, &HashSet::new());
        assert_eq!(adult_equivalents(&diners), Ratio::from_thousandths(2_600));
        diners.push(Diner { member: None, adult_equivalent: Ratio::ONE });
        assert_eq!(adult_equivalents(&diners), Ratio::from_thousandths(3_600));
    }

    /// Un membre absent par défaut le midi n'est pas convive ; un membre inactif non plus (R-PLA-2).
    #[test]
    fn r_pla_2_default_presence() {
        let mut members = vec![member("Alex", 1_000), member("Sam", 1_000), member("Lou", 600)];
        members[2].active = false;
        let absent = HashSet::from([members[0].id]);
        let diners = default_diners(&members, &absent);
        assert_eq!(diners.len(), 1);
        assert_eq!(diners[0].member, Some(members[1].id));
    }

    #[test]
    fn refusals_concern_present_members_only() {
        let alex = member("Alex", 1_000);
        let sam = member("Sam", 1_000);
        let mushrooms = Target::Food(FoodId::new());
        let preferences = vec![
            Preference { member: alex.id, target: mushrooms, level: PreferenceLevel::Refuses },
            Preference { member: sam.id, target: mushrooms, level: PreferenceLevel::Refuses },
        ];
        let diners = default_diners(std::slice::from_ref(&alex), &HashSet::new());
        assert_eq!(refusals(&diners, &preferences, &HashSet::from([mushrooms])), vec![alex.id]);
        assert!(refusals(&diners, &preferences, &HashSet::new()).is_empty());
    }
}

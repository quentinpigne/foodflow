//! Arborescence des familles et réglages hérités (R-ALI-1, R-ALI-2).

use std::collections::HashMap;
use std::hash::Hash;

use crate::shared::DomainError;

/// Vérifie qu'affecter `new_parent` à `node` ne crée pas de cycle (R-ALI-1).
///
/// `parents` donne le parent actuel de chaque famille.
pub fn check_parent<Id: Copy + Eq + Hash>(
    parents: &HashMap<Id, Option<Id>>,
    node: Id,
    new_parent: Option<Id>,
) -> Result<(), DomainError> {
    let mut current = new_parent;
    let mut steps = 0;
    while let Some(id) = current {
        if id == node || steps > parents.len() {
            return Err(DomainError::FoodGroupCycle);
        }
        current = parents.get(&id).copied().flatten();
        steps += 1;
    }
    Ok(())
}

/// Ancêtres d'une famille, du parent à la racine.
pub fn ancestors<Id: Copy + Eq + Hash>(parents: &HashMap<Id, Option<Id>>, node: Id) -> Vec<Id> {
    let mut out = Vec::new();
    let mut current = parents.get(&node).copied().flatten();
    while let Some(id) = current {
        if out.contains(&id) || id == node {
            break;
        }
        out.push(id);
        current = parents.get(&id).copied().flatten();
    }
    out
}

/// Valeur effective d'un réglage hérité : celle de l'élément le plus spécifique qui le définit (R-ALI-2).
///
/// `chain` va du plus spécifique (l'aliment) au plus général (la famille racine).
pub fn resolve<T: Copy>(chain: impl IntoIterator<Item = Option<T>>) -> Option<T> {
    chain.into_iter().flatten().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> HashMap<u32, Option<u32>> {
        // 1 Produits laitiers > 2 Fromages > 3 Pâtes pressées ; 4 Fruits
        HashMap::from([(1, None), (2, Some(1)), (3, Some(2)), (4, None)])
    }

    #[test]
    fn r_ali_1_no_cycle() {
        let parents = tree();
        assert!(check_parent(&parents, 3, Some(4)).is_ok());
        assert_eq!(check_parent(&parents, 1, Some(3)), Err(DomainError::FoodGroupCycle));
        assert_eq!(check_parent(&parents, 2, Some(2)), Err(DomainError::FoodGroupCycle));
        assert!(check_parent(&parents, 2, None).is_ok());
        assert_eq!(ancestors(&parents, 3), vec![2, 1]);
    }

    #[test]
    fn r_ali_2_most_specific_setting_wins() {
        assert_eq!(resolve([None, Some(30), Some(7)]), Some(30));
        assert_eq!(resolve([Some(1), Some(30)]), Some(1));
        assert_eq!(resolve::<i64>([None, None]), None);
    }
}

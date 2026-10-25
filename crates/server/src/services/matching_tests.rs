//! Mesure du rattachement des ingrédients importés (R-REC-64) : des lignes typiques des sites de recettes
//! (`fixtures/recipes/lignes-ingredients.json`), rattachées aux noms de la table Ciqual, avec le dictionnaire
//! de départ.

use std::collections::HashMap;

use foodflow_domain::recipes::Target;
use foodflow_domain::recipes::matching::{self, Candidate, Confidence};
use foodflow_domain::recipes::parsing::parse_lines;
use foodflow_domain::shared::ids::{FoodGroupId, FoodId};
use serde::Deserialize;

use super::recipe_import::dictionary;

const CIQUAL: &str = include_str!("../../../../fixtures/ciqual/noms-2020.json");
const LINES: &str = include_str!("../../../../fixtures/recipes/lignes-ingredients.json");
const DICTIONARY: &str = include_str!("../../seed/ingredient_dictionary.json");

/// Part minimale de lignes bien rattachées.
const TARGET_RATE: f64 = 0.9;
/// Part minimale sans le dictionnaire, par la seule recherche approchée.
const SEARCH_RATE: f64 = 0.7;

#[derive(Deserialize)]
struct Named {
    code: String,
    name: String,
}

#[derive(Deserialize)]
struct Ciqual {
    groups: Vec<Named>,
    foods: Vec<Named>,
}

#[derive(Deserialize)]
struct Line {
    line: String,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct Lines {
    lines: Vec<Line>,
}

/// La table Ciqual, comme après son import : un identifiant par aliment et par famille.
struct Table {
    candidates: Vec<Candidate>,
    names: HashMap<Target, String>,
    codes: HashMap<String, FoodId>,
}

fn table() -> Table {
    let ciqual: Ciqual = serde_json::from_str(CIQUAL).unwrap();
    let mut candidates = Vec::new();
    let mut names = HashMap::new();
    let mut codes = HashMap::new();
    for food in ciqual.foods {
        let target = Target::Food(FoodId::new());
        if let Target::Food(id) = target {
            codes.insert(food.code.clone(), id);
        }
        names.insert(target, food.name.clone());
        candidates.push(Candidate { target, name: food.name });
    }
    for group in ciqual.groups {
        let target = Target::Group(FoodGroupId::new());
        names.insert(target, format!("famille {}", group.name));
        candidates.push(Candidate { target, name: group.name });
    }
    candidates.sort_by(|a, b| a.name.cmp(&b.name));
    Table { candidates, names, codes }
}

#[test]
fn r_rec_64_dictionary_points_to_existing_foods() {
    #[derive(Deserialize)]
    struct Entry {
        names: Vec<String>,
        ciqual: String,
        food: String,
    }
    #[derive(Deserialize)]
    struct Dictionary {
        entries: Vec<Entry>,
    }
    let ciqual: Ciqual = serde_json::from_str(CIQUAL).unwrap();
    let by_code: HashMap<&str, &str> = ciqual.foods.iter().map(|f| (f.code.as_str(), f.name.as_str())).collect();
    let dictionary: Dictionary = serde_json::from_str(DICTIONARY).unwrap();
    let mut keys = HashMap::new();
    for entry in &dictionary.entries {
        assert_eq!(by_code.get(entry.ciqual.as_str()), Some(&entry.food.as_str()), "code Ciqual {}", entry.ciqual);
        for name in &entry.names {
            let previous = keys.insert(matching::key(name), &entry.ciqual);
            assert!(previous.is_none_or(|p| p == &entry.ciqual), "« {name} » vise deux aliments");
        }
    }
}

/// Part des lignes bien rattachées, et le détail des échecs.
fn measure(table: &Table, memory: &HashMap<String, Target>) -> (f64, Vec<String>) {
    let lines: Lines = serde_json::from_str(LINES).unwrap();
    let mut failures = Vec::new();
    for line in &lines.lines {
        let parsed = &parse_lines(&line.line)[0];
        let proposal = matching::propose(&parsed.name, memory, &table.candidates);
        let found = proposal.target.map(|t| table.names[&t].clone());
        let ok = if line.expected.is_empty() {
            proposal.confidence <= Confidence::Low
        } else {
            found.as_ref().is_some_and(|f| line.expected.contains(f))
        };
        if !ok {
            failures.push(format!(
                "  {:<40} « {} » → {:?} {}",
                line.line,
                parsed.name,
                proposal.confidence,
                found.as_deref().unwrap_or("—")
            ));
        }
    }
    (1.0 - failures.len() as f64 / lines.lines.len() as f64, failures)
}

/// Critère d'acceptation : au moins 90 % des lignes typiques sont rattachées au bon aliment ; une ligne sans
/// aliment Ciqual convenable n'a qu'une proposition de confiance basse. Sans le dictionnaire, la recherche
/// approchée seule doit encore en rattacher une majorité.
#[test]
fn r_rec_64_typical_lines_rate() {
    let table = table();
    let codes: HashMap<&str, FoodId> = table.codes.iter().map(|(c, id)| (c.as_str(), *id)).collect();
    let (rate, failures) = measure(&table, &dictionary(&codes).unwrap());
    println!("Avec le dictionnaire : {:.1} %\n{}", rate * 100.0, failures.join("\n"));
    let (alone, failures) = measure(&table, &HashMap::new());
    println!("Sans le dictionnaire : {:.1} %\n{}", alone * 100.0, failures.join("\n"));
    assert!(rate >= TARGET_RATE, "taux de rattachement {:.1} % < {:.0} %", rate * 100.0, TARGET_RATE * 100.0);
    assert!(alone >= SEARCH_RATE, "recherche seule {:.1} % < {:.0} %", alone * 100.0, SEARCH_RATE * 100.0);
}

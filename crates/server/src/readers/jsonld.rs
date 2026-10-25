//! Données schema.org/Recipe au format JSON-LD d'une page de recette (R-REC-60, R-REC-62).
//!
//! Seul le JSON-LD est lu : sans lui, l'import échoue franchement, sans extraction approximative du texte.

use rust_decimal::Decimal;
use serde_json::Value;

use foodflow_domain::recipes::durations::parse_minutes;

/// Blocs `<script type="application/ld+json">` d'une page, analysés ; un bloc illisible est ignoré.
pub fn extract(html: &str) -> Vec<Value> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<script") {
        let start = from + start;
        let Some(tag_end) = lower[start..].find('>').map(|i| start + i + 1) else { break };
        let Some(end) = lower[tag_end..].find("</script>").map(|i| tag_end + i) else { break };
        if lower[start..tag_end].contains("application/ld+json") {
            let body = html[tag_end..end].trim();
            let body = body.trim_start_matches("<!--").trim_end_matches("-->");
            let body = body.trim_start_matches("//<![CDATA[").trim_end_matches("//]]>").trim();
            if let Ok(value) = serde_json::from_str::<Value>(body) {
                out.push(value);
            }
        }
        from = end + "</script>".len();
    }
    out
}

fn is_recipe(value: &Value) -> bool {
    match value.get("@type") {
        Some(Value::String(t)) => t.eq_ignore_ascii_case("Recipe"),
        Some(Value::Array(types)) => types.iter().any(|t| t.as_str().is_some_and(|t| t.eq_ignore_ascii_case("Recipe"))),
        _ => false,
    }
}

/// Premier objet `Recipe`, directement, dans un tableau ou dans un `@graph`.
pub fn find_recipe(values: &[Value]) -> Option<Value> {
    for value in values {
        match value {
            Value::Object(map) if is_recipe(value) => return Some(Value::Object(map.clone())),
            Value::Object(map) => {
                if let Some(Value::Array(graph)) = map.get("@graph")
                    && let Some(found) = find_recipe(graph)
                {
                    return Some(found);
                }
            }
            Value::Array(items) => {
                if let Some(found) = find_recipe(items) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// Décode les entités HTML courantes que certains sites laissent dans leur JSON-LD.
pub fn decode_entities(text: &str) -> String {
    let mut out = text
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&rsquo;", "’")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&eacute;", "é")
        .replace("&egrave;", "è")
        .replace("&agrave;", "à")
        .replace("&ccedil;", "ç")
        .replace("&ocirc;", "ô")
        .replace("&ecirc;", "ê");
    out = out.replace("&amp;", "&");
    // Balises éventuelles dans les étapes (« <p>Mélanger</p> »).
    let mut clean = String::with_capacity(out.len());
    let mut in_tag = false;
    for c in out.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                clean.push(' ');
            }
            _ if !in_tag => clean.push(c),
            _ => {}
        }
    }
    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(decode_entities(s)).filter(|s| !s.is_empty()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(items) => items.iter().find_map(|i| text(Some(i))),
        Value::Object(map) => text(map.get("name").or_else(|| map.get("text"))),
        _ => None,
    }
}

fn texts(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(|i| text(Some(i))).collect(),
        Some(Value::String(s)) => s.split(',').map(decode_entities).filter(|s| !s.is_empty()).collect(),
        Some(other) => text(Some(other)).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Une étape lue, avec sa section éventuelle (`HowToSection`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedStep {
    pub section: Option<String>,
    pub text: String,
}

fn steps(value: Option<&Value>, section: Option<String>, out: &mut Vec<ImportedStep>) {
    match value {
        Some(Value::String(s)) => {
            // Une seule chaîne : une étape par ligne non vide.
            for line in s.split(['\n', '\r']).map(decode_entities).filter(|l| !l.is_empty()) {
                out.push(ImportedStep { section: section.clone(), text: line });
            }
        }
        Some(Value::Array(items)) => {
            for item in items {
                steps(Some(item), section.clone(), out);
            }
        }
        Some(Value::Object(map)) => {
            let kind = map.get("@type").and_then(Value::as_str).unwrap_or_default();
            if kind.eq_ignore_ascii_case("HowToSection") {
                let name = text(map.get("name"));
                steps(map.get("itemListElement"), name, out);
            } else if let Some(t) = text(map.get("text")).or_else(|| text(map.get("name"))) {
                out.push(ImportedStep { section: section.clone(), text: t });
            }
        }
        _ => {}
    }
}

/// Ce qu'une recette JSON-LD apporte (R-REC-62).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportedRecipe {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Premier nombre de `recipeYield` (« 4 personnes » → 4).
    pub portions: Option<Decimal>,
    pub prep_minutes: Option<u32>,
    pub cook_minutes: Option<u32>,
    pub total_minutes: Option<u32>,
    pub ingredients: Vec<String>,
    pub steps: Vec<ImportedStep>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub image: Option<String>,
}

fn first_number(text: &str) -> Option<Decimal> {
    let digits: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();
    digits.replace(',', ".").trim_end_matches('.').parse().ok()
}

fn image(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => items.iter().find_map(|i| image(Some(i))),
        Value::Object(map) => map.get("url").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

pub fn read(recipe: &Value) -> ImportedRecipe {
    let mut out = ImportedRecipe {
        name: text(recipe.get("name")),
        description: text(recipe.get("description")),
        portions: texts(recipe.get("recipeYield")).iter().find_map(|y| first_number(y)).filter(|p| *p > Decimal::ZERO),
        prep_minutes: text(recipe.get("prepTime")).and_then(|t| parse_minutes(&t)),
        cook_minutes: text(recipe.get("cookTime")).and_then(|t| parse_minutes(&t)),
        total_minutes: text(recipe.get("totalTime")).and_then(|t| parse_minutes(&t)),
        ingredients: texts(recipe.get("recipeIngredient").or_else(|| recipe.get("ingredients")))
            .into_iter()
            .filter(|i| !i.trim().is_empty())
            .collect(),
        steps: Vec::new(),
        categories: texts(recipe.get("recipeCategory")),
        keywords: [texts(recipe.get("recipeCuisine")), texts(recipe.get("keywords"))].concat(),
        image: image(recipe.get("image")),
    };
    // Dans `recipeIngredient`, une virgule fait partie d'une ligne : on ne la découpe pas.
    if let Some(Value::Array(items)) = recipe.get("recipeIngredient") {
        out.ingredients =
            items.iter().filter_map(Value::as_str).map(decode_entities).filter(|i| !i.is_empty()).collect();
    }
    steps(recipe.get("recipeInstructions"), None, &mut out.steps);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recipes").join(name),
        )
        .unwrap()
    }

    #[test]
    fn r_rec_62_reads_a_graph_recipe_with_sections() {
        let values = extract(&fixture("gratin-graph.html"));
        let recipe = find_recipe(&values).expect("recette trouvée dans le @graph");
        let r = read(&recipe);
        assert_eq!(r.name.as_deref(), Some("Gratin de courgettes"));
        assert_eq!(r.portions, Some(Decimal::from(4)));
        assert_eq!((r.prep_minutes, r.cook_minutes), (Some(20), Some(45)));
        assert_eq!(r.ingredients.len(), 6);
        assert!(r.ingredients.contains(&"20 cl de crème liquide, entière".to_owned()));
        assert_eq!(r.steps.len(), 4);
        assert_eq!(r.steps[0].section.as_deref(), Some("Préparation"));
        assert_eq!(r.steps[3].text, "Enfourner 45 minutes à 180 °C.");
        assert_eq!(r.categories, vec!["Plat principal".to_owned()]);
        assert_eq!(r.image.as_deref(), Some("https://exemple.test/gratin.jpg"));
    }

    #[test]
    fn r_rec_62_reads_plain_recipe_and_entities() {
        let values = extract(&fixture("crepes-simple.html"));
        let r = read(&find_recipe(&values).unwrap());
        assert_eq!(r.name.as_deref(), Some("Pâte à crêpes de l'Ouest"));
        assert_eq!(r.portions, Some(Decimal::from(6)));
        assert_eq!(r.total_minutes, Some(70));
        assert_eq!(r.steps.len(), 3);
        assert_eq!(r.keywords, vec!["crêpes".to_owned(), "chandeleur".to_owned()]);
    }

    #[test]
    fn r_rec_60_page_without_json_ld() {
        assert!(find_recipe(&extract("<html><body><h1>Gratin</h1></body></html>")).is_none());
        // Un JSON-LD qui n'est pas une recette ne suffit pas.
        let page = r#"<script type="application/ld+json">{"@type":"WebSite","name":"x"}</script>"#;
        assert!(find_recipe(&extract(page)).is_none());
    }
}

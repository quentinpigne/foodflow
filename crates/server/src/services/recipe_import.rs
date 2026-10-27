//! Import de recettes depuis une page web ou un fichier JSON-LD (REC-07, REC-11, R-REC-60 à R-REC-68).
//!
//! L'import produit un **brouillon** : rien n'est créé avant sa validation (ADR-0005). Chaque ligne
//! d'ingrédient est analysée et rattachée à une proposition, avec une confiance (R-REC-63, R-REC-64).

use std::collections::HashMap;

use foodflow_domain::recipes::matching::{self, Candidate, Confidence};
use foodflow_domain::recipes::parsing::parse_lines;
use foodflow_domain::recipes::{RecipeKind, Target};
use foodflow_domain::shared::ids::{FoodId, RecipeDraftId, RecipeId, UnitId};
use foodflow_domain::text::normalize;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::images;
use super::recipes::{Kitchen, RecipeInput, RecipeRef, Step};
use crate::clients::photos::PhotoSearch;
use crate::clients::web::{Fetcher, check_url};
use crate::clock::timestamp;
use crate::config::DataDir;
use crate::db::recipes::DraftRow;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};
use crate::readers::jsonld;

#[derive(Debug, Deserialize, ToSchema)]
pub struct StartImport {
    /// Adresse d'une page de recette.
    pub url: Option<String>,
    /// Ou le contenu d'un fichier schema.org/Recipe JSON-LD (export d'une recette, R-REC-70). Avec `url`,
    /// les données lues dans la page par le navigateur : la page n'est pas retéléchargée (R-REC-72).
    pub json_ld: Option<String>,
}

/// Une ligne d'ingrédient analysée, avec sa proposition de rattachement.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DraftLine {
    pub source_line: String,
    pub section: Option<String>,
    /// Nom analysé : sert à mémoriser le rattachement validé (R-REC-66).
    pub name: String,
    pub quantity: Option<String>,
    /// Borne haute d'un intervalle « 2 à 3 », signalée.
    pub quantity_max: Option<String>,
    pub unit_id: Option<UnitId>,
    pub unit_label: Option<String>,
    pub note: Option<String>,
    pub to_taste: bool,
    pub target: Option<Target>,
    pub target_name: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DraftPayload {
    pub name: String,
    pub description: Option<String>,
    pub kind: RecipeKind,
    /// Portions adulte proposées (« 4 personnes » → 4), ajustables (R-REC-10).
    pub portions: Option<String>,
    pub active_minutes: u32,
    pub passive_minutes: u32,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    pub image_url: Option<String>,
    pub steps: Vec<Step>,
    /// Lignes, celles de faible confiance d'abord (R-REC-64).
    pub lines: Vec<DraftLine>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RecipeDraft {
    pub id: RecipeDraftId,
    pub source_url: Option<String>,
    pub fetched_at: String,
    pub has_image: bool,
    pub payload: DraftPayload,
    /// Recettes déjà importées depuis la même adresse (R-REC-67).
    pub existing: Vec<RecipeRef>,
}

fn kind_of(categories: &[String]) -> RecipeKind {
    let text = normalize(&categories.join(" "));
    if text.contains("dessert") || text.contains("patisserie") {
        RecipeKind::Dessert
    } else if text.contains("entree") || text.contains("apero") {
        RecipeKind::Starter
    } else if text.contains("accompagnement") {
        RecipeKind::Side
    } else if text.contains("petit-dejeuner") || text.contains("petit dejeuner") || text.contains("brunch") {
        RecipeKind::Breakfast
    } else if text.contains("gouter") || text.contains("collation") || text.contains("snack") {
        RecipeKind::Snack
    } else if text.contains("sauce") || text.contains("base") {
        RecipeKind::Base
    } else {
        RecipeKind::Main
    }
}

fn decimal_text(value: Decimal) -> String {
    value.normalize().to_string().replace('.', ",")
}

const DICTIONARY: &str = include_str!("../../seed/ingredient_dictionary.json");

#[derive(Deserialize)]
struct Dictionary {
    entries: Vec<DictionaryEntry>,
}

#[derive(Deserialize)]
struct DictionaryEntry {
    /// Noms sous lesquels l'ingrédient apparaît dans les recettes (« farine », « farine de blé »).
    names: Vec<String>,
    /// Code Ciqual de l'aliment visé.
    ciqual: String,
}

/// Dictionnaire de départ (R-REC-64) : nom normalisé → aliment, pour les aliments Ciqual présents dans la base.
/// Les rattachements mémorisés de l'utilisateur passent devant (R-REC-66).
pub(crate) fn dictionary(foods_by_code: &HashMap<&str, FoodId>) -> anyhow::Result<HashMap<String, Target>> {
    let dictionary: Dictionary = serde_json::from_str(DICTIONARY)?;
    let mut out = HashMap::new();
    for entry in dictionary.entries {
        if let Some(food) = foods_by_code.get(entry.ciqual.as_str()) {
            for name in &entry.names {
                out.insert(matching::key(name), Target::Food(*food));
            }
        }
    }
    Ok(out)
}

/// Mémoire de rattachement : le dictionnaire de départ, puis les rattachements validés par l'utilisateur.
async fn memory(conn: &mut sqlx::SqliteConnection, kitchen: &Kitchen) -> anyhow::Result<HashMap<String, Target>> {
    let codes: HashMap<&str, FoodId> =
        kitchen.catalog.foods.values().filter_map(|f| Some((f.ciqual_code.as_deref()?, f.id))).collect();
    let mut memory = dictionary(&codes)?;
    memory.extend(db::recipes::mappings(&mut *conn).await?);
    Ok(memory)
}

/// Candidats au rattachement : aliments (et alias), familles, recettes.
async fn candidates(conn: &mut sqlx::SqliteConnection, kitchen: &Kitchen) -> anyhow::Result<Vec<Candidate>> {
    let mut out: Vec<Candidate> = kitchen
        .catalog
        .foods
        .values()
        .map(|f| Candidate { target: Target::Food(f.id), name: f.name.clone() })
        .chain(kitchen.catalog.groups.values().map(|g| Candidate { target: Target::Group(g.id), name: g.name.clone() }))
        .chain(
            kitchen
                .recipes
                .iter()
                .filter(|r| !r.archived)
                .map(|r| Candidate { target: Target::Recipe(r.id), name: r.name.clone() }),
        )
        .collect();
    for (food, alias) in db::foods::all_alias_keys(&mut *conn).await? {
        out.push(Candidate { target: Target::Food(food), name: alias });
    }
    // Ordre stable, pour des propositions déterministes.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Analyse et rattache les lignes d'ingrédients (R-REC-63, R-REC-64).
fn lines(
    kitchen: &Kitchen,
    raw: &[String],
    memory: &HashMap<String, Target>,
    candidates: &[Candidate],
) -> Vec<DraftLine> {
    let mut out: Vec<DraftLine> = Vec::new();
    for source in raw {
        for parsed in parse_lines(source) {
            let proposal = matching::propose(&parsed.name, memory, candidates);
            // Unité : générale si elle existe sous ce nom, sinon propre à l'aliment proposé, sinon gardée en texte.
            let unit = parsed.unit.as_deref().and_then(|name| {
                kitchen.units.iter().find(|u| u.food_id.is_none() && u.name == name).or_else(|| match proposal.target {
                    Some(Target::Food(food)) => {
                        kitchen.units.iter().find(|u| u.food_id == Some(food) && normalize(&u.name) == normalize(name))
                    }
                    _ => None,
                })
            });
            out.push(DraftLine {
                source_line: parsed.source.clone(),
                section: None,
                name: parsed.name.clone(),
                quantity: parsed.quantity.map(decimal_text),
                quantity_max: parsed.quantity_max.map(decimal_text),
                unit_id: unit.map(|u| u.id),
                unit_label: if unit.is_some() { None } else { parsed.unit.clone() },
                note: parsed.note.clone(),
                to_taste: parsed.to_taste,
                target_name: proposal.target.map(|t| kitchen.target_name(&t)),
                target: proposal.target,
                confidence: proposal.confidence,
            });
        }
    }
    // Les lignes les moins sûres d'abord, l'ordre de la recette ensuite (tri stable).
    out.sort_by_key(|l| l.confidence);
    out
}

fn draft_view(row: DraftRow, existing: Vec<RecipeRef>) -> AppResult<RecipeDraft> {
    Ok(RecipeDraft {
        id: row.id,
        source_url: row.source_url,
        fetched_at: row.fetched_at,
        has_image: row.image_path.is_some(),
        payload: serde_json::from_str(&row.payload).map_err(anyhow::Error::from)?,
        existing,
    })
}

fn existing(kitchen: &Kitchen, url: Option<&str>) -> Vec<RecipeRef> {
    let Some(url) = url else { return Vec::new() };
    kitchen
        .recipes
        .iter()
        .filter(|r| r.source_url.as_deref() == Some(url))
        .map(|r| RecipeRef { id: r.id, name: r.name.clone() })
        .collect()
}

/// Lance un import : télécharge la page (ou lit le JSON-LD fourni), analyse, et crée un brouillon (R-REC-61).
pub async fn start(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    input: &StartImport,
) -> AppResult<RecipeDraft> {
    let url = input.url.as_deref().map(str::trim).filter(|u| !u.is_empty());
    if let Some(url) = url {
        check_url(url).map_err(|e| AppError::invalid_field("INVALID_URL", "url", e.to_string()))?;
    }
    let values = match (url, input.json_ld.as_deref()) {
        (_, Some(text)) => serde_json::from_str(text).map(|v| vec![v]).map_err(|_| {
            AppError::invalid_field("INVALID_JSON_LD", "json_ld", "ce fichier n'est pas du JSON-LD valide")
        })?,
        (Some(url), None) => {
            let page = fetcher
                .get(url)
                .await
                .map_err(|e| AppError::invalid_field("PAGE_UNREACHABLE", "url", format!("page inaccessible : {e}")))?;
            jsonld::extract(&String::from_utf8_lossy(&page.body))
        }
        (None, None) => {
            return Err(AppError::invalid_field("REQUIRED", "url", "une adresse ou un fichier JSON-LD est requis"));
        }
    };
    // Sans données schema.org/Recipe, l'import échoue franchement (R-REC-60).
    let recipe = jsonld::find_recipe(&values).ok_or_else(|| {
        AppError::invalid(
            "NO_RECIPE_DATA",
            "cette page ne publie pas de recette au format schema.org (JSON-LD) : l'importer à la main",
        )
    })?;
    let imported = jsonld::read(&recipe);
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    let memory = memory(&mut conn, &kitchen).await?;
    let candidates = candidates(&mut conn, &kitchen).await?;
    drop(conn);
    let (active, passive) = match (imported.prep_minutes, imported.cook_minutes) {
        (None, None) => (imported.total_minutes.unwrap_or(0), 0),
        (prep, cook) => (prep.unwrap_or(0), cook.unwrap_or(0)),
    };
    let source_url = url.map(str::to_owned).or_else(|| recipe.get("url").and_then(|u| u.as_str()).map(str::to_owned));
    let payload = DraftPayload {
        name: imported.name.clone().unwrap_or_else(|| "Recette importée".to_owned()),
        description: imported.description.clone(),
        kind: kind_of(&imported.categories),
        portions: imported.portions.map(decimal_text),
        active_minutes: active,
        passive_minutes: passive,
        tags: imported.keywords.iter().map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()).collect(),
        source_url: source_url.clone(),
        image_url: imported.image.clone(),
        steps: imported
            .steps
            .iter()
            .map(|s| Step { section: s.section.clone(), text: s.text.clone(), timer_minutes: None })
            .collect(),
        lines: lines(&kitchen, &imported.ingredients, &memory, &candidates),
    };
    let id = RecipeDraftId::new();
    // L'image est copiée dans le volume dès l'import : la recette ne dépendra plus du site (R-REC-68).
    let mut image_path = None;
    if let Some(image) = imported.image.as_deref().filter(|i| check_url(i).is_ok())
        && let Ok(fetched) = fetcher.get(image).await
    {
        image_path = images::store(data, &format!("draft-{id}"), fetched.content_type.as_deref(), &fetched.body).ok();
    }
    // Sans image, une photo libre est cherchée d'après le nom, si le réglage l'active (R-REC-73).
    if image_path.is_none() {
        image_path = super::photos::find(db, data, fetcher, photos, &format!("draft-{id}"), &payload.name).await;
    }
    let row = DraftRow {
        id,
        source_url: source_url.clone(),
        fetched_at: timestamp(),
        payload: serde_json::to_string(&payload).map_err(anyhow::Error::from)?,
        raw_json_ld: recipe.to_string(),
        image_path,
    };
    db::recipes::insert_draft(&db.write, &row).await?;
    draft_view(row, existing(&kitchen, source_url.as_deref()))
}

pub async fn list(db: &Db) -> AppResult<Vec<RecipeDraft>> {
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    db::recipes::drafts(&mut *conn)
        .await?
        .into_iter()
        .map(|d| {
            let existing = existing(&kitchen, d.source_url.as_deref());
            draft_view(d, existing)
        })
        .collect()
}

async fn find(db: &Db, id: RecipeDraftId) -> AppResult<DraftRow> {
    db::recipes::drafts(&db.read).await?.into_iter().find(|d| d.id == id).ok_or(AppError::NotFound("brouillon"))
}

pub async fn get(db: &Db, id: RecipeDraftId) -> AppResult<RecipeDraft> {
    let row = find(db, id).await?;
    let mut conn = db.read.acquire().await?;
    let kitchen = Kitchen::load(&mut conn).await?;
    let existing = existing(&kitchen, row.source_url.as_deref());
    draft_view(row, existing)
}

/// Enregistre la revue en cours (le brouillon se reprend plus tard, R-REC-61).
pub async fn save(db: &Db, id: RecipeDraftId, payload: &DraftPayload) -> AppResult<()> {
    find(db, id).await?;
    db::recipes::update_draft(&db.write, id, &serde_json::to_string(payload).map_err(anyhow::Error::from)?).await?;
    Ok(())
}

pub async fn discard(db: &Db, data: &DataDir, id: RecipeDraftId) -> AppResult<()> {
    let row = find(db, id).await?;
    db::recipes::delete_draft(&db.write, id).await?;
    if let Some(image) = row.image_path {
        images::remove(data, &image);
    }
    Ok(())
}

/// Valide un brouillon : crée la recette, mémorise les rattachements (R-REC-65, R-REC-66), reprend l'image.
pub async fn commit(db: &Db, data: &DataDir, id: RecipeDraftId, input: &RecipeInput) -> AppResult<RecipeId> {
    let row = find(db, id).await?;
    let mut input = input.clone();
    if input.source_url.is_none() {
        input.source_url = row.source_url.clone();
    }
    let recipe = super::recipes::create(db, &input).await?;
    for ingredient in &input.ingredients {
        if let Some(name) = ingredient.import_name.as_deref() {
            let key = matching::key(name);
            if !key.is_empty() {
                db::recipes::remember(&db.write, &key, &ingredient.target).await?;
            }
        }
    }
    if let Some(image) = &row.image_path {
        let from = data.images().join(image);
        let name = image.replacen(&format!("draft-{id}"), &format!("recipe-{recipe}"), 1);
        if std::fs::rename(&from, data.images().join(&name)).is_ok() {
            db::recipes::set_image(&db.write, recipe, Some(&name)).await?;
        }
    }
    db::recipes::delete_draft(&db.write, id).await?;
    // Une recette nouvelle d'un planning par l'IA : ses repas peuvent désormais être acceptés (R-PLA-66).
    super::ai_planning::draft_committed(db, id, recipe).await?;
    Ok(recipe)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::clients::photos::StaticPhotoSearch;
    use crate::clients::web::{Fetched, StaticFetcher};
    use crate::services::recipes::{self, IngredientInput};
    use crate::services::{demo, seed};

    fn page(name: &str) -> Fetched {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/recipes").join(name);
        Fetched { body: std::fs::read(path).unwrap(), content_type: Some("text/html".into()) }
    }

    fn fetcher() -> StaticFetcher {
        StaticFetcher {
            pages: HashMap::from([
                ("https://exemple.test/gratin".to_owned(), page("gratin-graph.html")),
                ("https://exemple.test/crepes".to_owned(), page("crepes-simple.html")),
                (
                    "https://exemple.test/rien".to_owned(),
                    Fetched { body: b"<html><body>Rien</body></html>".to_vec(), content_type: None },
                ),
            ]),
        }
    }

    fn to_input(draft: &RecipeDraft) -> RecipeInput {
        let p = &draft.payload;
        RecipeInput {
            name: p.name.clone(),
            kind: p.kind,
            description: p.description.clone(),
            notes: None,
            yield_: foodflow_domain::recipes::Yield::Portions { portions: foodflow_domain::Amount::from_raw(4_000) },
            active_minutes: p.active_minutes,
            passive_minutes: p.passive_minutes,
            fridge_days: None,
            freezable: false,
            frozen_days: None,
            finishing_minutes: None,
            leftover_food_id: None,
            source_url: None,
            tags: p.tags.clone(),
            ingredients: p
                .lines
                .iter()
                .filter_map(|l| {
                    Some(IngredientInput {
                        section: None,
                        target: l.target?,
                        quantity: l.quantity.clone(),
                        unit_id: l.unit_id,
                        unit_label: l.unit_label.clone(),
                        to_taste: l.to_taste,
                        optional: false,
                        fixed: false,
                        note: l.note.clone(),
                        source_line: Some(l.source_line.clone()),
                        import_name: Some(l.name.clone()),
                    })
                })
                .collect(),
            steps: p.steps.clone(),
        }
    }

    /// Import d'une page : brouillon, rien de créé avant la validation ; puis validation et mémoire (R-REC-61, R-REC-66).
    #[tokio::test]
    async fn r_rec_61_to_66_import_review_and_memory() {
        let (db, dir) = db::test_db().await;
        let data = DataDir::new(dir.path());
        demo::load(&db, "2026-10-25".parse().unwrap()).await.unwrap();
        let start_input = StartImport { url: Some("https://exemple.test/gratin".into()), json_ld: None };
        let before = recipes::search(&db, &Default::default()).await.unwrap().len();
        let draft = start(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &start_input).await.unwrap();
        assert_eq!(
            recipes::search(&db, &Default::default()).await.unwrap().len(),
            before,
            "rien n'est créé avant la validation"
        );
        let p = &draft.payload;
        assert_eq!(
            (p.name.as_str(), p.kind, p.active_minutes, p.passive_minutes),
            ("Gratin de courgettes", RecipeKind::Main, 20, 45)
        );
        assert_eq!(p.portions.as_deref(), Some("4"));
        // « Sel, poivre » donne deux lignes à volonté ; la crème garde sa précision.
        assert_eq!(p.lines.len(), 7);
        assert_eq!(p.lines.iter().filter(|l| l.to_taste).count(), 2);
        let zucchini = p.lines.iter().find(|l| l.name == "courgettes").unwrap();
        assert_eq!(zucchini.target_name.as_deref(), Some("Courgette"));
        assert_eq!(zucchini.quantity.as_deref(), Some("1"));
        assert!(zucchini.unit_id.is_some(), "kg est une unité générale");
        // Les lignes les moins sûres d'abord (R-REC-64).
        assert!(p.lines.windows(2).all(|w| w[0].confidence <= w[1].confidence));

        // On rattache tout ce qui manque à la courgette pour l'exemple, puis on valide.
        let mut input = to_input(&draft);
        for line in draft.payload.lines.iter().filter(|l| l.target.is_none()) {
            input.ingredients.push(IngredientInput {
                section: None,
                target: zucchini.target.unwrap(),
                quantity: None,
                unit_id: None,
                unit_label: None,
                to_taste: true,
                optional: false,
                fixed: false,
                note: None,
                source_line: Some(line.source_line.clone()),
                import_name: Some(line.name.clone()),
            });
        }
        let id = commit(&db, &data, draft.id, &input).await.unwrap();
        let recipe = recipes::get(&db, id, None).await.unwrap();
        assert_eq!(recipe.source_url.as_deref(), Some("https://exemple.test/gratin"));
        assert_eq!(recipe.steps.len(), 4);
        assert!(list(&db).await.unwrap().is_empty(), "le brouillon validé disparaît");

        // Réimport : la même adresse est signalée, et les rattachements validés reviennent en confiance haute.
        let again = start(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &start_input).await.unwrap();
        assert_eq!(again.existing.len(), 1, "R-REC-67");
        assert!(
            again
                .payload
                .lines
                .iter()
                .find(|l| l.name == "courgettes")
                .is_some_and(|l| l.confidence == Confidence::High)
        );
    }

    /// Critère d'acceptation : une page sans données schema.org échoue, sans brouillon (R-REC-60).
    #[tokio::test]
    async fn r_rec_60_page_without_recipe_data() {
        let (db, dir) = db::test_db().await;
        seed::run(&db).await.unwrap();
        let data = DataDir::new(dir.path());
        let input = StartImport { url: Some("https://exemple.test/rien".into()), json_ld: None };
        let result = start(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &input).await;
        assert!(matches!(result, Err(AppError::Invalid { code: "NO_RECIPE_DATA", .. })));
        assert!(list(&db).await.unwrap().is_empty());
        let bad = StartImport { url: Some("file:///etc/passwd".into()), json_ld: None };
        assert!(start(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &bad).await.is_err());
    }
}

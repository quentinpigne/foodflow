//! Tests de l'IA (ADR-0014), avec des réponses enregistrées : aucun appel réseau.

use foodflow_domain::Date;
use foodflow_domain::shared::ids::RecipeId;
use serde_json::json;

use super::ai::{self, AiProvider, AiSettingsInput, RecipeIdeaInput, RecipeIdeaMode};
use super::ai_planning::{self, AcceptInput, PlanIdeaInput};
use super::recipe_import;
use super::recipes::{self, RecipeQuery};
use super::{demo, export, meals};
use crate::clients::ai::StaticAiClient;
use crate::clients::web::StaticFetcher;
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::AppError;

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const TODAY: &str = "2026-10-25";

async fn setup() -> (Db, tempfile::TempDir) {
    let (db, dir) = db::test_db().await;
    demo::load(&db, d(TODAY)).await.unwrap();
    (db, dir)
}

fn settings(key: Option<&str>, limit: u32) -> AiSettingsInput {
    AiSettingsInput {
        enabled: true,
        provider: AiProvider::Mistral,
        base_url: None,
        model: "mistral-small-latest".into(),
        api_key: key.map(str::to_owned),
        monthly_limit: limit,
        new_recipes_per_plan: 1,
    }
}

/// La revue d'un brouillon telle que l'interface l'envoie : les lignes rattachées, 4 portions.
fn input_from(payload: &recipe_import::DraftPayload) -> recipes::RecipeInput {
    recipes::RecipeInput {
        name: payload.name.clone(),
        kind: payload.kind,
        description: payload.description.clone(),
        notes: None,
        yield_: foodflow_domain::recipes::Yield::Portions { portions: foodflow_domain::Amount::from_raw(4_000) },
        active_minutes: payload.active_minutes,
        passive_minutes: payload.passive_minutes,
        fridge_days: None,
        freezable: false,
        frozen_days: None,
        finishing_minutes: None,
        leftover_food_id: None,
        source_url: None,
        tags: payload.tags.clone(),
        ingredients: payload
            .lines
            .iter()
            .filter_map(|l| {
                Some(recipes::IngredientInput {
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
        steps: payload.steps.clone(),
    }
}

fn fetcher() -> StaticFetcher {
    StaticFetcher { pages: Default::default() }
}

async fn recipe(db: &Db, name: &str) -> RecipeId {
    recipes::search(db, &RecipeQuery::default()).await.unwrap().into_iter().find(|r| r.name == name).unwrap().id
}

fn soup() -> serde_json::Value {
    json!({
        "name": "Soupe de courgettes",
        "kind": "MAIN",
        "description": "Douce et rapide.",
        "portions": 4,
        "active_minutes": 15,
        "passive_minutes": 20,
        "ingredients": ["800 g de courgettes", "1 oignon", "50 cl de lait"],
        "steps": ["Cuire les légumes.", "Mixer avec le lait."],
        "tags": ["soupe"]
    })
}

/// La clé est enregistrée mais jamais renvoyée ni exportée ; l'adresse par défaut suit le fournisseur
/// (ADR-0014).
#[tokio::test]
async fn adr_0014_key_is_write_only() {
    let (db, _dir) = setup().await;
    let initial = ai::get_settings(&db, d(TODAY)).await.unwrap();
    assert!(!initial.enabled && !initial.ready, "désactivée par défaut (ADR-0007)");
    ai::update_settings(&db, &settings(Some("secret-key"), 10)).await.unwrap();
    let saved = ai::get_settings(&db, d(TODAY)).await.unwrap();
    assert!(saved.has_key && saved.ready);
    assert_eq!(saved.base_url, "https://api.mistral.ai/v1");
    assert!(!serde_json::to_string(&saved).unwrap().contains("secret-key"));
    // Sans clé dans la demande, la clé enregistrée reste ; vide, elle est effacée.
    ai::update_settings(&db, &AiSettingsInput { provider: AiProvider::Openai, ..settings(None, 10) }).await.unwrap();
    let kept = ai::get_settings(&db, d(TODAY)).await.unwrap();
    assert!(kept.has_key);
    assert_eq!(kept.base_url, "https://api.openai.com/v1");
    let dump = export::export(&db).await.unwrap();
    assert!(!dump.to_string().contains("secret-key"), "la clé n'est pas exportée");
    ai::update_settings(&db, &settings(Some(""), 10)).await.unwrap();
    assert!(!ai::get_settings(&db, d(TODAY)).await.unwrap().has_key);
}

/// Une recette demandée par consigne arrive en brouillon ; ce qui part ne nomme pas les membres ; chaque
/// appel compte dans le plafond (R-REC-80 à R-REC-83).
#[tokio::test]
async fn r_rec_80_83_recipe_from_prompt() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    let input =
        RecipeIdeaInput { mode: RecipeIdeaMode::Prompt, prompt: Some("une soupe d'automne".into()), recipe_id: None };
    let client = StaticAiClient::new(vec![soup().to_string(), soup().to_string()]);
    let disabled = ai::create_recipe(&db, &data, &fetcher(), &client, &input, d(TODAY)).await;
    assert!(matches!(disabled, Err(AppError::Invalid { code: "AI_DISABLED", .. })));
    ai::update_settings(&db, &settings(Some("k"), 1)).await.unwrap();

    let preview = ai::recipe_preview(&db, &input, d(TODAY)).await.unwrap();
    assert!(preview.user.contains("- A : 1 équivalent adulte"), "{}", preview.user);
    assert!(preview.user.contains("- C : 0,6 équivalent adulte"));
    assert!(!preview.user.contains("Alex") && !preview.user.contains("Lou"), "jamais de nom (ADR-0007)");
    assert!(preview.user.ends_with("Propose une recette : une soupe d'automne"));

    let draft = ai::create_recipe(&db, &data, &fetcher(), &client, &input, d(TODAY)).await.unwrap();
    assert_eq!(draft.payload.name, "Soupe de courgettes");
    assert_eq!(draft.payload.portions.as_deref(), Some("4"));
    assert!(draft.payload.lines.iter().any(|l| l.target_name.as_deref() == Some("Courgette")));
    let first_sent = client.requests.lock().unwrap()[0].user.clone();
    assert_eq!(first_sent, preview.user, "l'aperçu est exactement ce qui part");
    assert_eq!(ai::get_settings(&db, d(TODAY)).await.unwrap().used_this_month, 1);
    let limited = ai::create_recipe(&db, &data, &fetcher(), &client, &input, d(TODAY)).await;
    assert!(matches!(limited, Err(AppError::Invalid { code: "AI_LIMIT", .. })), "plafond de 1 appel (R-REC-83)");
}

/// Variante, stock et choix de l'IA : les données envoyées suivent le point de départ (R-REC-80).
#[tokio::test]
async fn r_rec_80_starting_points() {
    let (db, _dir) = setup().await;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let variant =
        RecipeIdeaInput {
            mode: RecipeIdeaMode::Variant, prompt: Some("plus légère".into()), recipe_id: Some(gratin)
        };
    let preview = ai::recipe_preview(&db, &variant, d(TODAY)).await.unwrap();
    assert!(preview.user.contains("Recette de départ : Gratin de courgettes"));
    assert!(preview.user.contains("1000 g Courgette") || preview.user.contains("Courgette"));
    let stock = RecipeIdeaInput { mode: RecipeIdeaMode::Stock, prompt: None, recipe_id: None };
    assert!(ai::recipe_preview(&db, &stock, d(TODAY)).await.unwrap().user.contains("À utiliser en priorité"));
    let surprise = RecipeIdeaInput { mode: RecipeIdeaMode::Surprise, prompt: None, recipe_id: None };
    let text = ai::recipe_preview(&db, &surprise, d(TODAY)).await.unwrap().user;
    assert!(text.contains("Aussi en stock") && text.contains("goûts"));
    let missing = RecipeIdeaInput { mode: RecipeIdeaMode::Prompt, prompt: None, recipe_id: None };
    assert!(ai::recipe_preview(&db, &missing, d(TODAY)).await.is_err());
}

/// Une réponse qui ne suit pas le format est refusée, jamais corrigée en silence (R-REC-82).
#[tokio::test]
async fn r_rec_82_invalid_answer_is_refused() {
    let (db, dir) = setup().await;
    ai::update_settings(&db, &settings(Some("k"), 10)).await.unwrap();
    let input = RecipeIdeaInput { mode: RecipeIdeaMode::Prompt, prompt: Some("un dessert".into()), recipe_id: None };
    let client = StaticAiClient::new(vec![json!({ "name": "Rien" }).to_string()]);
    let result = ai::create_recipe(&db, &DataDir::new(dir.path()), &fetcher(), &client, &input, d(TODAY)).await;
    assert!(matches!(result, Err(AppError::Domain(foodflow_domain::DomainError::AiResponse(_)))), "{result:?}");
}

/// Un planning par l'IA : éléments invalides écartés, recette nouvelle en brouillon, repas créés à
/// l'acceptation, et seulement une fois le brouillon validé (R-PLA-64, R-PLA-66, R-PLA-67).
#[tokio::test]
async fn r_pla_64_66_67_plan_with_new_recipe() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    ai::update_settings(&db, &settings(Some("k"), 10)).await.unwrap();
    let template = super::week::template(&db).await.unwrap();
    let dinner = template.iter().find(|s| s.name == "Dîner").unwrap().slot_id;
    let lunch = template.iter().find(|s| s.name == "Déjeuner").unwrap().slot_id;
    let gratin = recipe(&db, "Gratin de courgettes").await;
    let input =
        PlanIdeaInput { start: d("2026-11-02"), days: Some(2), slot_ids: vec![dinner], prompt: Some("léger".into()) };
    let preview = ai_planning::preview(&db, &input, d(TODAY)).await.unwrap();
    assert!(preview.user.contains(&format!("[créneau {dinner}]")));
    assert!(preview.user.contains(&format!("[recette {gratin}]")));
    assert!(preview.user.contains("au plus 1 recette(s) nouvelle(s)"));
    assert!(preview.user.contains("Consigne du foyer : léger"));
    assert!(!preview.user.contains(&lunch.to_string()), "seulement les créneaux demandés");

    let answer = json!({
        "meals": [
            { "date": "2026-11-02", "slot_id": dinner, "items": [{ "recipe_id": gratin, "new_recipe": null, "lot_id": null }], "reason": "Des légumes." },
            { "date": "2026-11-03", "slot_id": dinner, "items": [{ "recipe_id": null, "new_recipe": 0, "lot_id": null }], "reason": "Une soupe." },
            { "date": "2026-11-03", "slot_id": lunch, "items": [{ "recipe_id": gratin, "new_recipe": null, "lot_id": null }], "reason": "" },
            { "date": "2026-11-02", "slot_id": dinner, "items": [{ "recipe_id": RecipeId::new(), "new_recipe": null, "lot_id": null }], "reason": "" }
        ],
        "new_recipes": [soup()]
    });
    let client = StaticAiClient::new(vec![answer.to_string()]);
    let proposal = ai_planning::generate(&db, &data, &fetcher(), &client, &input, d(TODAY)).await.unwrap();
    assert_eq!(proposal.meals.len(), 2);
    assert_eq!(proposal.rejected.len(), 2, "créneau non demandé, doublon : {:?}", proposal.rejected);
    assert!(proposal.meals[0].ready && !proposal.meals[1].ready);
    let draft = proposal.new_recipes[0].draft_id.expect("recette nouvelle en brouillon");

    // Rien n'est écrit avant l'acceptation ; le repas de la recette nouvelle attend son brouillon.
    assert!(db::planning::meals(&db.read, d("2026-11-02"), d("2026-11-03")).await.unwrap().is_empty());
    let first = ai_planning::accept(&db, proposal.id, &AcceptInput { meals: vec![] }, d(TODAY)).await.unwrap();
    assert_eq!(first.accepted, 1);
    assert_eq!(first.skipped.len(), 1);
    let plan = meals::plan(&db, &meals::PlanQuery { start: d("2026-11-02"), days: Some(2) }, d(TODAY)).await.unwrap();
    let monday = &plan.days[0].meals[0];
    assert_eq!(monday.items[0].recipe_id, Some(gratin));
    assert_eq!(monday.items[0].cooked_portions, Some(foodflow_domain::Amount::from_raw(2_600)));

    // Le brouillon validé, le second repas s'accepte.
    let review = recipe_import::get(&db, draft).await.unwrap();
    let body = input_from(&review.payload);
    recipe_import::commit(&db, &data, draft, &body).await.unwrap();
    let second = ai_planning::accept(&db, proposal.id, &AcceptInput { meals: vec![] }, d(TODAY)).await.unwrap();
    assert_eq!((second.accepted, second.skipped.len()), (1, 0));
    assert_eq!(ai_planning::get(&db, proposal.id, d(TODAY)).await.unwrap().status, "ACCEPTED");
}

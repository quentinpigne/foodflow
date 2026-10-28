//! Tests de l'IA (ADR-0014), avec des réponses enregistrées : aucun appel réseau.

use foodflow_domain::Date;
use foodflow_domain::shared::ids::RecipeId;
use serde_json::json;

use super::ai::{self, AiProvider, AiSettingsInput, RecipeIdeaInput, RecipeIdeaMode};
use super::ai_planning::{self, AcceptInput, PlanIdeaInput};
use super::photos::PhotoSettingsInput;
use super::recipe_import;
use super::recipes::{self, RecipeQuery};
use super::{demo, export, meals};
use crate::clients::ai::StaticAiClient;
use crate::clients::photos::StaticPhotoSearch;
use crate::clients::web::{Fetched, StaticFetcher};
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
    let disabled =
        ai::create_recipe(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &client, &input, d(TODAY)).await;
    assert!(matches!(disabled, Err(AppError::Invalid { code: "AI_DISABLED", .. })));
    ai::update_settings(&db, &settings(Some("k"), 1)).await.unwrap();

    let preview = ai::recipe_preview(&db, &input, d(TODAY)).await.unwrap();
    assert!(preview.user.contains("- A : 1 équivalent adulte"), "{}", preview.user);
    assert!(preview.user.contains("- C : 0,6 équivalent adulte"));
    assert!(!preview.user.contains("Alex") && !preview.user.contains("Lou"), "jamais de nom (ADR-0007)");
    assert!(preview.user.ends_with("Propose une recette : une soupe d'automne"));

    let draft = ai::create_recipe(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &client, &input, d(TODAY))
        .await
        .unwrap();
    assert_eq!(draft.payload.name, "Soupe de courgettes");
    assert_eq!(draft.payload.portions.as_deref(), Some("4"));
    assert!(draft.payload.lines.iter().any(|l| l.target_name.as_deref() == Some("Courgette")));
    let first_sent = client.requests.lock().unwrap()[0].user.clone();
    assert_eq!(first_sent, preview.user, "l'aperçu est exactement ce qui part");
    assert_eq!(ai::get_settings(&db, d(TODAY)).await.unwrap().used_this_month, 1);
    let limited =
        ai::create_recipe(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &client, &input, d(TODAY)).await;
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
    let result = ai::create_recipe(
        &db,
        &DataDir::new(dir.path()),
        &fetcher(),
        &StaticPhotoSearch::default(),
        &client,
        &input,
        d(TODAY),
    )
    .await;
    let error = result.unwrap_err().to_string();
    assert!(error.starts_with("réponse de l'IA invalide : la recette ne suit pas le format demandé : "), "{error}");
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
    // Références courtes à la place des identifiants : le créneau demandé est C1.
    assert!(preview.user.contains("(lundi dîner) [créneau C1]"), "{}", preview.user);
    assert!(!preview.user.contains(&gratin.to_string()) && !preview.user.contains(&dinner.to_string()));
    let gratin_ref = preview
        .user
        .lines()
        .find(|l| l.contains("Gratin de courgettes"))
        .and_then(|l| l.split("[recette ").nth(1))
        .and_then(|l| l.split(']').next())
        .unwrap()
        .to_owned();
    assert!(gratin_ref.starts_with('R'), "{gratin_ref}");
    assert!(preview.user.contains("au plus 1 recette(s) nouvelle(s)"));
    assert!(preview.user.contains("Consigne du foyer : léger"));
    assert!(!preview.user.contains(&lunch.to_string()), "seulement les créneaux demandés");

    let answer = json!({
        "meals": [
            { "date": "2026-11-02", "slot_id": "C1", "items": [{ "recipe_id": gratin_ref, "new_recipe": null, "lot_id": null }], "reason": "Des légumes." },
            { "date": "2026-11-03", "slot_id": "C1", "items": [{ "recipe_id": null, "new_recipe": 0, "lot_id": null }], "reason": "Une soupe." },
            { "date": "2026-11-03", "slot_id": lunch, "items": [{ "recipe_id": gratin_ref, "new_recipe": null, "lot_id": null }], "reason": "" },
            { "date": "2026-11-02", "slot_id": "C1", "items": [{ "recipe_id": RecipeId::new(), "new_recipe": null, "lot_id": null }], "reason": "" }
        ],
        "new_recipes": [soup()]
    });
    let client = StaticAiClient::new(vec![answer.to_string()]);
    let proposal =
        ai_planning::generate(&db, &data, &fetcher(), &StaticPhotoSearch::default(), &client, &input, d(TODAY))
            .await
            .unwrap();
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

/// Une recette sans image reçoit une photo libre cherchée d'après son nom, seulement si le réglage est actif ;
/// la clé n'est ni renvoyée ni exportée (R-REC-73).
#[tokio::test]
async fn r_rec_73_free_photo_for_a_recipe_without_image() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    ai::update_settings(&db, &settings(Some("k"), 10)).await.unwrap();
    let input = RecipeIdeaInput { mode: RecipeIdeaMode::Prompt, prompt: Some("une soupe".into()), recipe_id: None };
    let mut with_query = soup();
    with_query["image_query"] = json!(" zucchini soup bowl ");
    let client = StaticAiClient::new(vec![soup().to_string(), soup().to_string(), with_query.to_string()]);
    let image = "https://cdn.pixabay.test/soupe.jpg";
    let photos = StaticPhotoSearch::one(image);
    let mut pages = std::collections::HashMap::new();
    pages.insert(
        image.to_owned(),
        Fetched { body: vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0], content_type: Some("image/jpeg".into()) },
    );
    let fetcher = StaticFetcher { pages };

    let draft = ai::create_recipe(&db, &data, &fetcher, &photos, &client, &input, d(TODAY)).await.unwrap();
    assert!(!draft.has_image, "réglage désactivé par défaut");
    assert!(photos.queries.lock().unwrap().is_empty(), "aucun appel sans le réglage");
    let disabled = super::photos::search(&db, &photos, "soupe").await;
    assert!(matches!(disabled, Err(AppError::Invalid { code: "PHOTOS_DISABLED", .. })));

    let enable = PhotoSettingsInput { enabled: true, api_key: Some("photo-secret".into()) };
    super::photos::update_settings(&db, &enable).await.unwrap();
    let settings = super::photos::get_settings(&db).await.unwrap();
    assert!(settings.ready && settings.has_key);
    let draft = ai::create_recipe(&db, &data, &fetcher, &photos, &client, &input, d(TODAY)).await.unwrap();
    assert!(draft.has_image);
    assert_eq!(*photos.queries.lock().unwrap(), ["Soupe de courgettes|fr"], "seul le nom part");

    // Les mots-clés de l'IA, en anglais, passent avant le nom.
    photos.queries.lock().unwrap().clear();
    let draft = ai::create_recipe(&db, &data, &fetcher, &photos, &client, &input, d(TODAY)).await.unwrap();
    assert!(draft.has_image);
    assert_eq!(*photos.queries.lock().unwrap(), ["zucchini soup bowl|en"]);
    assert!(!export::export(&db).await.unwrap().to_string().contains("photo-secret"), "la clé n'est pas exportée");
}

/// Changer de photo : des photos proposées, ou une adresse saisie, pour un brouillon comme pour une recette ;
/// la photo du brouillon passe à la recette validée (R-REC-73, R-REC-68).
#[tokio::test]
async fn r_rec_73_choose_a_photo() {
    let (db, dir) = setup().await;
    let data = DataDir::new(dir.path());
    let image = "https://cdn.pixabay.test/soupe.jpg";
    let photos = StaticPhotoSearch::one(image);
    let jpeg = || Fetched { body: vec![0xFF, 0xD8, 0xFF, 0xE0, 1, 2], content_type: Some("image/jpeg".into()) };
    let fetcher = StaticFetcher {
        pages: [(image.to_owned(), jpeg()), ("https://exemple.test/ma-photo.jpg".to_owned(), jpeg())].into(),
    };
    super::photos::update_settings(&db, &PhotoSettingsInput { enabled: true, api_key: Some("k".into()) })
        .await
        .unwrap();
    let found = super::photos::search(&db, &photos, "soupe de courgettes").await.unwrap();
    assert_eq!(found[0].image_url, image);
    assert_eq!(*photos.queries.lock().unwrap(), ["soupe de courgettes|fr"]);

    let json_ld = json!({ "@type": "Recipe", "name": "Soupe", "recipeIngredient": ["1 courgette"] }).to_string();
    let start = recipe_import::StartImport { url: None, json_ld: Some(json_ld) };
    let draft = recipe_import::start(&db, &data, &fetcher, &StaticPhotoSearch::default(), &start).await.unwrap();
    let chosen = super::photos::ImageUrlInput { url: image.into() };
    super::photos::choose_for_draft(&db, &data, &fetcher, draft.id, &chosen).await.unwrap();
    let (bytes, kind) = super::images::read_draft(&db, &data, draft.id).await.unwrap();
    assert_eq!((bytes.len(), kind), (6, "image/jpeg"));
    let review = recipe_import::get(&db, draft.id).await.unwrap();
    assert!(review.has_image);
    let created = recipe_import::commit(&db, &data, draft.id, &input_from(&review.payload)).await.unwrap();
    assert!(super::images::read(&db, &data, created).await.is_ok(), "la photo passe à la recette");

    let typed = super::photos::ImageUrlInput { url: "https://exemple.test/ma-photo.jpg".into() };
    super::photos::choose_for_recipe(&db, &data, &fetcher, created, &typed).await.unwrap();
    let invalid = super::photos::ImageUrlInput { url: "file:///etc/passwd".into() };
    let refused = super::photos::choose_for_recipe(&db, &data, &fetcher, created, &invalid).await;
    assert!(matches!(refused, Err(AppError::Invalid { code: "INVALID_URL", .. })));
}

/// Le planning désigne créneaux, recettes et lots par des références courtes, retraduites en identifiants ;
/// un repas illisible ou à la référence inconnue est écarté et signalé sans faire refuser le planning ; une
/// réponse qui n'est pas un planning est refusée avec sa cause (R-PLA-67, R-REC-82).
#[test]
fn r_pla_67_short_references_and_unreadable_meals() {
    let slot = "019a2b3c-0000-7000-8000-000000000001";
    let recipe = "019a2b3c-0000-7000-8000-000000000002";
    let refs: std::collections::HashMap<String, String> =
        [("C1".to_owned(), slot.to_owned()), ("R1".to_owned(), recipe.to_owned())].into();
    let item = |recipe_id: &str| json!([{ "recipe_id": recipe_id, "new_recipe": null, "lot_id": "null" }]);
    let content = json!({
        "meals": [
            { "date": "2026-11-02", "slot_id": "C1", "items": item("R1"), "reason": "" },
            { "date": "2026-11-03", "slot_id": "[c1]", "items": item("Ratatouille"), "reason": "" },
            { "date": "2026-11-04", "slot_id": "C1", "items": "aucun", "reason": "" }
        ],
        "new_recipes": []
    });
    let (plan, rejected) = ai_planning::read_plan(&content.to_string(), &refs).unwrap();
    assert_eq!(plan.meals.len(), 1);
    assert_eq!(plan.meals[0].slot_id.to_string(), slot);
    assert_eq!(plan.meals[0].items[0].recipe_id.map(|r| r.to_string()).as_deref(), Some(recipe));
    assert_eq!(plan.meals[0].items[0].lot_id, None, "« null » en texte : aucun lot");
    assert_eq!(rejected.len(), 2);
    assert_eq!(rejected[0].date, Some(d("2026-11-03")));
    assert_eq!(rejected[0].slot_id.map(|s| s.to_string()).as_deref(), Some(slot));
    assert_eq!(rejected[0].reason, "repas illisible : référence inconnue « Ratatouille » (recipe_id)");
    assert!(rejected[1].reason.starts_with("repas illisible : invalid type"), "{}", rejected[1].reason);

    let truncated = r#"{"meals": [{"date": "2026-11-02""#;
    let error = ai_planning::read_plan(truncated, &refs).unwrap_err().to_string();
    assert!(error.starts_with("réponse de l'IA invalide : le planning ne suit pas le format demandé : EOF"), "{error}");
}

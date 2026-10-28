//! IA (ADR-0014) : réglages, plafond d'appels, données envoyées, création d'une recette (REC-19).
//!
//! Les données envoyées sont construites ici, de façon déterministe : l'aperçu montré avant l'envoi est
//! exactement ce qui part (R-REC-81, R-PLA-66). Les membres du foyer y sont désignés par une lettre.

use foodflow_domain::ai::pseudonym;
use foodflow_domain::ai::recipe::AiRecipe;
use foodflow_domain::planning::household::PreferenceLevel;
use foodflow_domain::recipes::RecipeKind;
use foodflow_domain::shared::ids::RecipeId;
use foodflow_domain::stock::ExpiryStatus;
use foodflow_domain::{Date, DomainError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqliteConnection;
use utoipa::ToSchema;

use super::recipe_import::{self, RecipeDraft, StartImport};
use super::recipes::Kitchen;
use super::stock::{self, Lot, LotQuery};
use crate::clients::ai::{AiClient, AiConfig, AiRequest};
use crate::clients::photos::PhotoSearch;
use crate::clients::web::{Fetcher, check_url};
use crate::config::DataDir;
use crate::db::{self, Db};
use crate::error::{AppError, AppResult};

const ENABLED: &str = "ai.enabled";
const PROVIDER: &str = "ai.provider";
const BASE_URL: &str = "ai.base_url";
const MODEL: &str = "ai.model";
const API_KEY: &str = "ai.api_key";
const MONTHLY_LIMIT: &str = "ai.monthly_limit";
const NEW_RECIPES: &str = "ai.new_recipes_per_plan";

/// Plafond d'appels par mois par défaut (ADR-0014).
pub const DEFAULT_MONTHLY_LIMIT: u32 = 50;
/// Recettes nouvelles par planning par défaut (R-PLA-66).
pub const DEFAULT_NEW_RECIPES: u32 = 3;

/// Fournisseur d'IA : tous deux exposent une API de chat compatible OpenAI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiProvider {
    Mistral,
    Openai,
}

impl AiProvider {
    /// Adresse par défaut de l'API.
    pub fn default_url(self) -> &'static str {
        match self {
            AiProvider::Mistral => "https://api.mistral.ai/v1",
            AiProvider::Openai => "https://api.openai.com/v1",
        }
    }
}

/// Réglages de l'IA, tels que l'interface les voit : la clé n'est jamais renvoyée (ADR-0014).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiSettings {
    pub enabled: bool,
    pub provider: AiProvider,
    pub base_url: String,
    pub model: String,
    /// Une clé est enregistrée.
    pub has_key: bool,
    pub monthly_limit: u32,
    pub new_recipes_per_plan: u32,
    /// Appels déjà faits ce mois-ci.
    pub used_this_month: u32,
    /// Activée, avec un modèle et une clé : utilisable.
    pub ready: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AiSettingsInput {
    pub enabled: bool,
    pub provider: AiProvider,
    /// Vide : l'adresse par défaut du fournisseur.
    pub base_url: Option<String>,
    pub model: String,
    /// Absente : la clé enregistrée est gardée ; vide : elle est effacée.
    pub api_key: Option<String>,
    pub monthly_limit: u32,
    pub new_recipes_per_plan: u32,
}

fn usage_key(today: Date) -> String {
    format!("ai.usage.{}", today.format("%Y-%m"))
}

pub async fn settings(conn: &mut SqliteConnection, today: Date) -> anyhow::Result<AiSettings> {
    let provider: AiProvider = db::settings::get(&mut *conn, PROVIDER).await?.unwrap_or(AiProvider::Mistral);
    let base_url: Option<String> = db::settings::get(&mut *conn, BASE_URL).await?;
    let model: String = db::settings::get(&mut *conn, MODEL).await?.unwrap_or_default();
    let key: Option<String> = db::settings::get(&mut *conn, API_KEY).await?;
    let enabled: bool = db::settings::get(&mut *conn, ENABLED).await?.unwrap_or(false);
    let has_key = key.as_deref().is_some_and(|k| !k.is_empty());
    Ok(AiSettings {
        enabled,
        provider,
        base_url: base_url.filter(|u| !u.is_empty()).unwrap_or_else(|| provider.default_url().to_owned()),
        ready: enabled && has_key && !model.trim().is_empty(),
        model,
        has_key,
        monthly_limit: db::settings::get(&mut *conn, MONTHLY_LIMIT).await?.unwrap_or(DEFAULT_MONTHLY_LIMIT),
        new_recipes_per_plan: db::settings::get(&mut *conn, NEW_RECIPES).await?.unwrap_or(DEFAULT_NEW_RECIPES),
        used_this_month: db::settings::get(&mut *conn, &usage_key(today)).await?.unwrap_or(0),
    })
}

pub async fn get_settings(db: &Db, today: Date) -> AppResult<AiSettings> {
    let mut conn = db.read.acquire().await?;
    Ok(settings(&mut conn, today).await?)
}

pub async fn update_settings(db: &Db, input: &AiSettingsInput) -> AppResult<()> {
    let base_url = input.base_url.as_deref().map(str::trim).filter(|u| !u.is_empty());
    if let Some(url) = base_url {
        check_url(url)
            .map_err(|_| AppError::invalid_field("INVALID_URL", "base_url", "adresse http ou https attendue"))?;
    }
    if input.model.chars().count() > 200 {
        return Err(AppError::invalid_field("TOO_LONG", "model", "200 caractères au maximum"));
    }
    if input.monthly_limit > 10_000 || input.new_recipes_per_plan > 100 {
        return Err(AppError::invalid_field("OUT_OF_RANGE", "monthly_limit", "valeur trop grande"));
    }
    let mut tx = db.write.begin().await?;
    db::settings::set(&mut *tx, ENABLED, &input.enabled).await?;
    db::settings::set(&mut *tx, PROVIDER, &input.provider).await?;
    db::settings::set(&mut *tx, BASE_URL, &base_url.unwrap_or_default()).await?;
    db::settings::set(&mut *tx, MODEL, &input.model.trim()).await?;
    if let Some(key) = &input.api_key {
        db::settings::set(&mut *tx, API_KEY, &key.trim()).await?;
    }
    db::settings::set(&mut *tx, MONTHLY_LIMIT, &input.monthly_limit).await?;
    db::settings::set(&mut *tx, NEW_RECIPES, &input.new_recipes_per_plan).await?;
    tx.commit().await?;
    Ok(())
}

/// Configuration d'appel, si l'IA est prête ; compte l'appel dans le plafond du mois (R-REC-83).
pub async fn reserve_call(conn: &mut SqliteConnection, today: Date) -> AppResult<AiConfig> {
    let current = settings(&mut *conn, today).await?;
    if !current.ready {
        return Err(AppError::invalid(
            "AI_DISABLED",
            "l'IA n'est pas activée : choisir le fournisseur, le modèle et la clé dans les réglages",
        ));
    }
    if current.used_this_month >= current.monthly_limit {
        return Err(AppError::invalid("AI_LIMIT", "plafond d'appels à l'IA atteint pour ce mois (réglages)"));
    }
    db::settings::set(&mut *conn, &usage_key(today), &(current.used_this_month + 1)).await?;
    let api_key: String = db::settings::get(&mut *conn, API_KEY).await?.unwrap_or_default();
    Ok(AiConfig { base_url: current.base_url, model: current.model, api_key })
}

// ===================================================================== Données envoyées

/// Ce que l'IA recevra, montré avant l'envoi (R-REC-81, R-PLA-66).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AiPreview {
    /// Consignes générales.
    pub system: String,
    /// Données et demande, en texte lisible.
    pub user: String,
}

pub const SYSTEM: &str = "Tu es l'assistant culinaire d'un foyer français. Tu réponds en français, avec des unités \
métriques (g, kg, mL, cL, L, cuillère à soupe, cuillère à café, pièce), des ingrédients courants en France et des \
recettes réalistes, de saison quand c'est possible. Les membres du foyer sont désignés par une lettre. Tu respectes \
strictement les refus des membres et le format JSON demandé.";

fn level(level: PreferenceLevel) -> &'static str {
    match level {
        PreferenceLevel::Loves => "adore",
        PreferenceLevel::Likes => "aime",
        PreferenceLevel::Dislikes => "n'aime pas",
        PreferenceLevel::Refuses => "refuse",
    }
}

/// Le foyer, membres désignés par une lettre, avec leur coefficient et leurs préférences (ADR-0007).
pub async fn household_text(
    conn: &mut SqliteConnection,
    kitchen: &Kitchen,
) -> anyhow::Result<(String, Vec<(foodflow_domain::shared::ids::MemberId, String)>)> {
    let members: Vec<_> = db::planning::members(&mut *conn).await?.into_iter().filter(|m| m.active).collect();
    let preferences = db::planning::preferences(&mut *conn).await?;
    let mut letters = Vec::new();
    let mut lines = Vec::new();
    for (index, member) in members.iter().enumerate() {
        let letter = pseudonym(index);
        let mut line = format!("- {letter} : {} équivalent adulte", decimal(member.adult_equivalent.thousandths()));
        let likes: Vec<String> = preferences
            .iter()
            .filter(|p| p.member_id == member.id)
            .map(|p| format!("{} {}", level(p.level), kitchen.target_name(&p.target)))
            .collect();
        if !likes.is_empty() {
            line.push_str(&format!(" ; {}", likes.join(", ")));
        }
        lines.push(line);
        letters.push((member.id, letter));
    }
    let text =
        if lines.is_empty() { "Foyer : non renseigné.".to_owned() } else { format!("Foyer :\n{}", lines.join("\n")) };
    Ok((text, letters))
}

/// Millièmes → « 2,6 ».
pub fn decimal(thousandths: i64) -> String {
    let whole = thousandths / 1_000;
    let rest = (thousandths % 1_000).abs();
    if rest == 0 { whole.to_string() } else { format!("{whole},{}", format!("{rest:03}").trim_end_matches('0')) }
}

fn quantity(lot: &Lot) -> String {
    let Some(q) = lot.quantity else { return String::new() };
    match (lot.recipe_id, lot.measure) {
        (Some(_), _) => format!("{} portion(s)", decimal(q.raw())),
        (None, foodflow_domain::Measure::Mass) => format!("{} g", decimal(q.raw())),
        (None, foodflow_domain::Measure::Volume) => format!("{} mL", decimal(q.raw())),
        (None, foodflow_domain::Measure::Count) => format!("{} pièce(s)", decimal(q.raw())),
    }
}

/// Lots à utiliser en priorité : bientôt périmés, restes et préparations, jugés encore bons (R-PLA-62).
pub fn priority_lots(lots: &[Lot]) -> Vec<&Lot> {
    lots.iter()
        .filter(|l| !l.depleted)
        .filter(|l| l.status == ExpiryStatus::Soon || l.recipe_id.is_some() || l.checked_until.is_some())
        .filter(|l| !matches!(l.status, ExpiryStatus::Expired | ExpiryStatus::PastBestBefore))
        .collect()
}

/// Lots en texte ; avec `with_refs`, chacun a sa référence courte (`L1`, `L2`…, dans l'ordre de la liste).
pub fn lots_text(title: &str, lots: &[&Lot], with_refs: bool) -> String {
    if lots.is_empty() {
        return format!("{title} : aucun.");
    }
    let lines: Vec<String> = lots
        .iter()
        .enumerate()
        .map(|(index, l)| {
            let until =
                l.effective_expiry.map(|e| format!(", avant le {}", e.date.format("%d/%m"))).unwrap_or_default();
            let id = if with_refs { format!(" [lot L{}]", index + 1) } else { String::new() };
            format!("- {} : {}{until}{id}", l.name, quantity(l))
        })
        .collect();
    format!("{title} :\n{}", lines.join("\n"))
}

// ===================================================================== Recette (REC-19)

/// Point de départ d'une recette demandée à l'IA (R-REC-80).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecipeIdeaMode {
    /// Une consigne libre.
    Prompt,
    /// Le stock à écouler.
    Stock,
    /// Une variante d'une recette existante.
    Variant,
    /// Au choix de l'IA, d'après le stock et les goûts du foyer.
    Surprise,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RecipeIdeaInput {
    pub mode: RecipeIdeaMode,
    /// Consigne ; requise pour `PROMPT` et `VARIANT`, facultative sinon.
    pub prompt: Option<String>,
    /// Recette de départ (`VARIANT`).
    pub recipe_id: Option<RecipeId>,
}

/// Schéma JSON d'une recette (R-REC-82).
pub fn recipe_schema() -> Value {
    let kinds: Vec<String> = [
        RecipeKind::Main,
        RecipeKind::Side,
        RecipeKind::Starter,
        RecipeKind::Dessert,
        RecipeKind::Breakfast,
        RecipeKind::Snack,
        RecipeKind::Base,
    ]
    .iter()
    .filter_map(|k| serde_json::to_value(k).ok().and_then(|v| v.as_str().map(str::to_owned)))
    .collect();
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "kind", "description", "portions", "active_minutes", "passive_minutes", "ingredients", "steps", "tags", "image_query"],
        "properties": {
            "name": { "type": "string" },
            "kind": { "type": "string", "enum": kinds },
            "description": { "type": ["string", "null"] },
            "portions": { "type": "integer", "description": "portions adulte" },
            "active_minutes": { "type": "integer" },
            "passive_minutes": { "type": "integer" },
            "ingredients": { "type": "array", "items": { "type": "string" }, "description": "une ligne par ingrédient, quantité et unité d'abord : « 200 g de farine »" },
            "steps": { "type": "array", "items": { "type": "string" }, "description": "une étape par élément, sans numéro" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "image_query": { "type": "string", "description": "3 à 5 mots-clés en anglais pour chercher une photo du plat : « zucchini soup bowl »" }
        }
    })
}

/// La demande d'une recette, construite à l'identique pour l'aperçu et pour l'envoi.
async fn recipe_request(conn: &mut SqliteConnection, input: &RecipeIdeaInput, today: Date) -> AppResult<AiRequest> {
    let prompt = input.prompt.as_deref().map(str::trim).filter(|p| !p.is_empty());
    let kitchen = Kitchen::load(&mut *conn).await?;
    let (household, _) = household_text(&mut *conn, &kitchen).await?;
    let mut parts = vec![household];
    let query = LotQuery { location_id: None, food_id: None, include_depleted: false };
    let ask = match input.mode {
        RecipeIdeaMode::Prompt => {
            let prompt =
                prompt.ok_or_else(|| AppError::invalid_field("REQUIRED", "prompt", "une consigne est requise"))?;
            format!("Propose une recette : {prompt}")
        }
        RecipeIdeaMode::Stock => {
            let all = stock::list_in(&mut *conn, &query, today).await?;
            parts.push(lots_text("À utiliser en priorité", &priority_lots(&all), false));
            format!(
                "Propose une recette qui utilise au mieux ce qui est à consommer en priorité.{}",
                prompt.map(|p| format!(" Consigne : {p}")).unwrap_or_default()
            )
        }
        RecipeIdeaMode::Variant => {
            let id = input
                .recipe_id
                .ok_or_else(|| AppError::invalid_field("REQUIRED", "recipe_id", "choisir une recette"))?;
            let recipe = kitchen.recipes.iter().find(|r| r.id == id).ok_or(AppError::NotFound("recette"))?;
            let ingredients: Vec<String> = kitchen
                .ingredients
                .iter()
                .filter(|i| i.recipe_id == id)
                .map(|i| format!("- {}", kitchen.ingredient_text(i)))
                .collect();
            let steps = db::recipes::steps(&mut *conn, id).await?;
            parts.push(format!(
                "Recette de départ : {}\nIngrédients :\n{}\nÉtapes :\n{}",
                recipe.name,
                ingredients.join("\n"),
                steps.iter().map(|s| format!("- {}", s.text)).collect::<Vec<_>>().join("\n")
            ));
            let prompt = prompt.ok_or_else(|| AppError::invalid_field("REQUIRED", "prompt", "dire quelle variante"))?;
            format!("Propose une variante de cette recette : {prompt}. Donne-lui un nom différent.")
        }
        RecipeIdeaMode::Surprise => {
            let all = stock::list_in(&mut *conn, &query, today).await?;
            let priority = priority_lots(&all);
            let mut others: Vec<&Lot> = all.iter().filter(|l| !priority.iter().any(|p| p.id == l.id)).collect();
            others.truncate(60);
            parts.push(lots_text("À utiliser en priorité", &priority, false));
            parts.push(lots_text("Aussi en stock", &others, false));
            format!(
                "Propose la recette de ton choix, qui plaise au foyer d'après ses goûts et tire parti du stock.{}",
                prompt.map(|p| format!(" Consigne : {p}")).unwrap_or_default()
            )
        }
    };
    parts.push(ask);
    Ok(AiRequest {
        system: SYSTEM.to_owned(),
        user: parts.join("\n\n"),
        schema_name: "recette".to_owned(),
        schema: recipe_schema(),
    })
}

/// Aperçu de ce qui sera envoyé pour une recette (R-REC-81).
pub async fn recipe_preview(db: &Db, input: &RecipeIdeaInput, today: Date) -> AppResult<AiPreview> {
    let mut conn = db.read.acquire().await?;
    let request = recipe_request(&mut conn, input, today).await?;
    Ok(AiPreview { system: request.system, user: request.user })
}

/// La recette reçue, au format schema.org, pour passer par l'import (R-REC-82).
pub fn json_ld(recipe: &AiRecipe) -> Value {
    json!({
        "@context": "https://schema.org",
        "@type": "Recipe",
        "name": recipe.name.trim(),
        "description": recipe.description,
        "recipeCategory": match recipe.kind {
            RecipeKind::Main => "Plat principal",
            RecipeKind::Side => "Accompagnement",
            RecipeKind::Starter => "Entrée",
            RecipeKind::Dessert => "Dessert",
            RecipeKind::Breakfast => "Petit-déjeuner",
            RecipeKind::Snack => "Goûter",
            RecipeKind::Base => "Base",
        },
        "recipeYield": recipe.portions.to_string(),
        "prepTime": format!("PT{}M", recipe.active_minutes),
        "cookTime": format!("PT{}M", recipe.passive_minutes),
        "recipeIngredient": recipe.ingredients,
        "recipeInstructions": recipe.steps.iter().map(|s| json!({ "@type": "HowToStep", "text": s })).collect::<Vec<_>>(),
        "keywords": recipe.tags.join(", "),
    })
}

/// Lit et vérifie une recette reçue.
pub fn parse_recipe(content: &str) -> AppResult<AiRecipe> {
    let recipe: AiRecipe = serde_json::from_str(content)
        .map_err(|e| DomainError::AiFormat(format!("la recette ne suit pas le format demandé : {e}")))?;
    recipe.check()?;
    Ok(recipe)
}

/// Fait de la recette reçue un brouillon, relu comme un import (R-REC-82).
pub async fn draft_from(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    recipe: &AiRecipe,
) -> AppResult<RecipeDraft> {
    let input = StartImport { url: None, json_ld: Some(json_ld(recipe).to_string()) };
    recipe_import::start_with(db, data, fetcher, photos, &input, recipe.photo_query()).await
}

/// Demande une recette à l'IA ; elle arrive en brouillon (REC-19).
pub async fn create_recipe(
    db: &Db,
    data: &DataDir,
    fetcher: &dyn Fetcher,
    photos: &dyn PhotoSearch,
    ai: &dyn AiClient,
    input: &RecipeIdeaInput,
    today: Date,
) -> AppResult<RecipeDraft> {
    let (request, config) = {
        let mut tx = db.write.begin().await?;
        let request = recipe_request(&mut tx, input, today).await?;
        let config = reserve_call(&mut tx, today).await?;
        tx.commit().await?;
        (request, config)
    };
    let content =
        ai.complete(&config, &request).await.map_err(|e| AppError::invalid("AI_UNAVAILABLE", e.to_string()))?;
    let recipe = parse_recipe(&content)?;
    draft_from(db, data, fetcher, photos, &recipe).await
}

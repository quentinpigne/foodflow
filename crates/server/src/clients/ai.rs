//! Client de l'IA (ADR-0014) : une API de chat compatible OpenAI (`POST {adresse}/chat/completions`), que
//! Mistral et OpenAI exposent toutes deux. Appelé seulement à la demande de l'utilisateur (ADR-0007) ; le
//! contenu des requêtes et la clé ne sont jamais journalisés.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::{Value, json};

/// Taille maximale d'une réponse.
const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Où et comment appeler l'IA.
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// Adresse de l'API, sans `/chat/completions` (« https://api.mistral.ai/v1 »).
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

/// Une demande : consignes du système, message, et schéma JSON de la réponse attendue.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system: String,
    pub user: String,
    /// Nom et schéma JSON imposés à la réponse.
    pub schema_name: String,
    pub schema: Value,
}

pub type AiFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'a>>;

pub trait AiClient: Send + Sync {
    /// Envoie la demande et renvoie le contenu de la réponse (du JSON, à valider par le domaine).
    fn complete<'a>(&'a self, config: &'a AiConfig, request: &'a AiRequest) -> AiFuture<'a>;
}

/// Corps de la requête : réponse demandée par schéma (`json_schema`), ou, si le modèle ne le permet pas,
/// en simple objet JSON (`json_object`) ; elle est de toute façon validée ensuite.
pub fn body(config: &AiConfig, request: &AiRequest, strict_schema: bool) -> Value {
    let format = if strict_schema {
        json!({ "type": "json_schema", "json_schema": { "name": request.schema_name, "schema": request.schema, "strict": true } })
    } else {
        json!({ "type": "json_object" })
    };
    json!({
        "model": config.model,
        "temperature": 0.4,
        "messages": [
            { "role": "system", "content": request.system },
            { "role": "user", "content": request.user },
        ],
        "response_format": format,
    })
}

pub struct HttpAiClient {
    client: reqwest::Client,
}

impl HttpAiClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            // Une génération de menus prend du temps.
            .timeout(Duration::from_secs(180))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("Foodflow/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client })
    }

    async fn send(&self, config: &AiConfig, body: &Value) -> anyhow::Result<(reqwest::StatusCode, String)> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let mut response = self
            .client
            .post(url)
            .bearer_auth(&config.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .context("IA injoignable")?;
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_BYTES {
                bail!("réponse de l'IA trop volumineuse");
            }
        }
        Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
    }
}

/// Contenu du premier choix d'une réponse de chat.
pub fn content(response: &str) -> anyhow::Result<String> {
    let value: Value = serde_json::from_str(response).context("réponse de l'IA illisible")?;
    value["choices"][0]["message"]["content"].as_str().map(str::to_owned).context("réponse de l'IA sans contenu")
}

impl AiClient for HttpAiClient {
    fn complete<'a>(&'a self, config: &'a AiConfig, request: &'a AiRequest) -> AiFuture<'a> {
        Box::pin(async move {
            let (status, text) = self.send(config, &body(config, request, true)).await?;
            // Un modèle sans réponse par schéma refuse la requête : on redemande en simple JSON.
            let (status, text) = if status == reqwest::StatusCode::BAD_REQUEST || status.as_u16() == 422 {
                self.send(config, &body(config, request, false)).await?
            } else {
                (status, text)
            };
            match status.as_u16() {
                200..=299 => content(&text),
                401 | 403 => bail!("clé d'accès refusée par le fournisseur d'IA"),
                404 => bail!("adresse ou modèle inconnu du fournisseur d'IA"),
                429 => bail!("limite d'appels du fournisseur d'IA atteinte, réessayer plus tard"),
                code => bail!("le fournisseur d'IA répond {code}"),
            }
        })
    }
}

/// Doublure des tests : renvoie des réponses enregistrées, dans l'ordre, et garde les demandes reçues.
#[cfg(test)]
pub struct StaticAiClient {
    pub responses: std::sync::Mutex<Vec<String>>,
    pub requests: std::sync::Mutex<Vec<AiRequest>>,
}

#[cfg(test)]
impl StaticAiClient {
    pub fn new(responses: Vec<String>) -> Self {
        Self { responses: std::sync::Mutex::new(responses), requests: std::sync::Mutex::new(Vec::new()) }
    }
}

#[cfg(test)]
impl AiClient for StaticAiClient {
    fn complete<'a>(&'a self, _config: &'a AiConfig, request: &'a AiRequest) -> AiFuture<'a> {
        Box::pin(async move {
            self.requests.lock().map_err(|_| anyhow::anyhow!("verrou"))?.push(request.clone());
            let mut responses = self.responses.lock().map_err(|_| anyhow::anyhow!("verrou"))?;
            if responses.is_empty() {
                bail!("aucune réponse enregistrée");
            }
            Ok(responses.remove(0))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_asks_for_a_schema_then_plain_json() {
        let config = AiConfig {
            base_url: "https://api.mistral.ai/v1".into(),
            model: "mistral-small".into(),
            api_key: "k".into(),
        };
        let request = AiRequest {
            system: "s".into(),
            user: "u".into(),
            schema_name: "recette".into(),
            schema: json!({ "type": "object" }),
        };
        let strict = body(&config, &request, true);
        assert_eq!(strict["model"], "mistral-small");
        assert_eq!(strict["response_format"]["type"], "json_schema");
        assert_eq!(strict["messages"][1]["content"], "u");
        assert_eq!(body(&config, &request, false)["response_format"]["type"], "json_object");
        assert!(!strict.to_string().contains("\"k\""), "la clé ne va que dans l'en-tête");
    }

    #[test]
    fn content_reads_the_first_choice() {
        let response = r#"{"choices":[{"message":{"role":"assistant","content":"{\"a\":1}"}}]}"#;
        assert_eq!(content(response).unwrap(), "{\"a\":1}");
        assert!(content("{}").is_err());
    }
}

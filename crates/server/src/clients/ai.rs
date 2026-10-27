//! Client de l'IA (ADR-0014) : une API de chat compatible OpenAI (`POST {adresse}/chat/completions`), que
//! Mistral et OpenAI exposent toutes deux. Appelé seulement à la demande de l'utilisateur (ADR-0007) ; le
//! contenu des requêtes et la clé ne sont jamais journalisés.
//!
//! La réponse est demandée en flux (`stream`) : ses morceaux arrivent au fil de la génération, si bien qu'un
//! proxy qui coupe les connexions inactives (souvent après 60 s) ne l'interrompt pas.

use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde_json::{Value, json};

/// Délai sans rien recevoir : un modèle qui raisonne peut se taire un moment avant d'écrire.
pub const TIMEOUT: Duration = Duration::from_secs(180);
/// Durée totale maximale d'une réponse : un planning de la semaine est long à écrire.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(600);
/// Un échec plus rapide, sans réponse, vient d'une connexion déjà fermée par le fournisseur : la demande n'a
/// pas été traitée, et se refait une fois.
const QUICK_FAILURE: Duration = Duration::from_secs(2);

/// Taille maximale d'une réponse ; en flux, chaque morceau de texte est enveloppé de JSON.
const MAX_BYTES: usize = 16 * 1024 * 1024;

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
        "stream": true,
    })
}

pub struct HttpAiClient {
    client: reqwest::Client,
}

impl HttpAiClient {
    pub fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .read_timeout(TIMEOUT)
            .timeout(TOTAL_TIMEOUT)
            .connect_timeout(Duration::from_secs(10))
            // Les fournisseurs ferment vite les connexions inactives : ne pas en réutiliser une trop ancienne.
            .pool_idle_timeout(Duration::from_secs(20))
            .user_agent(concat!("Foodflow/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client })
    }

    async fn send(&self, config: &AiConfig, body: &Value) -> anyhow::Result<(reqwest::StatusCode, String)> {
        let started = Instant::now();
        let mut response = match self.post(config, body).await {
            Err(error) if !error.is_timeout() && started.elapsed() < QUICK_FAILURE => {
                tracing::warn!(error = describe(&error), "appel à l'IA refait après une connexion fermée");
                self.post(config, body).await
            }
            result => result,
        }
        .map_err(|e| failure(&e, started))?;
        let status = response.status();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| failure(&e, started))? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_BYTES {
                bail!("réponse de l'IA trop volumineuse");
            }
        }
        Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
    }
}

impl HttpAiClient {
    async fn post(&self, config: &AiConfig, body: &Value) -> reqwest::Result<reqwest::Response> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        self.client
            .post(url)
            .bearer_auth(&config.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
    }
}

/// Cause d'une erreur de connexion, de la plus générale à la plus précise.
fn describe(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(" : ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Message d'un appel sans réponse, journalisé avec sa cause et sa durée (jamais le contenu ni la clé).
fn failure(error: &reqwest::Error, started: Instant) -> anyhow::Error {
    let seconds = started.elapsed().as_secs();
    tracing::warn!(error = describe(error), seconds, "appel à l'IA sans réponse");
    if error.is_timeout() {
        anyhow::anyhow!(
            "l'IA n'a rien envoyé pendant {} s, ou n'a pas fini en {} min : réessayer, ou choisir un modèle plus rapide",
            TIMEOUT.as_secs(),
            TOTAL_TIMEOUT.as_secs() / 60
        )
    } else if error.is_connect() {
        anyhow::anyhow!("IA injoignable (connexion impossible) : {}", describe(error))
    } else {
        anyhow::anyhow!("connexion à l'IA interrompue après {seconds} s : {}", describe(error))
    }
}

/// Contenu d'une réponse : en flux (`data: {…}` ligne à ligne, jusqu'à `data: [DONE]`), ou d'un seul bloc si
/// le service ignore la demande de flux.
pub fn reply(response: &str) -> anyhow::Result<String> {
    if response.trim_start().starts_with('{') {
        return content(response);
    }
    let mut text = String::new();
    let mut finish = None;
    for data in response.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim) {
        if data == "[DONE]" {
            break;
        }
        let value: Value = serde_json::from_str(data).context("réponse de l'IA illisible")?;
        if let Some(error) = value.get("error") {
            let message = error.get("message").and_then(Value::as_str).unwrap_or("erreur inconnue");
            bail!("le fournisseur d'IA s'est interrompu : {message}");
        }
        let choice = &value["choices"][0];
        if let Some(part) = choice["delta"]["content"].as_str().or(choice["message"]["content"].as_str()) {
            text.push_str(part);
        }
        if let Some(reason) = choice["finish_reason"].as_str() {
            finish = Some(reason.to_owned());
        }
    }
    finished(&text, finish.as_deref())
}

/// Texte complet d'une réponse : refusé s'il est vide ou tronqué, débarrassé d'un bloc de code Markdown.
fn finished(text: &str, finish_reason: Option<&str>) -> anyhow::Result<String> {
    if finish_reason == Some("length") {
        bail!(
            "réponse de l'IA tronquée : le modèle a atteint sa longueur de réponse maximale (moins de jours, \
             moins de recettes nouvelles, ou un modèle qui répond plus long)"
        );
    }
    let text = text.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .and_then(|t| t.strip_suffix("```"))
        .unwrap_or(text)
        .trim();
    if text.is_empty() {
        bail!("réponse de l'IA sans contenu");
    }
    Ok(text.to_owned())
}

/// Contenu du premier choix d'une réponse de chat d'un seul bloc.
pub fn content(response: &str) -> anyhow::Result<String> {
    let value: Value = serde_json::from_str(response).context("réponse de l'IA illisible")?;
    let choice = &value["choices"][0];
    finished(choice["message"]["content"].as_str().unwrap_or_default(), choice["finish_reason"].as_str())
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
                200..=299 => reply(&text),
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
        assert_eq!(strict["stream"], true, "réponse en flux");
        assert_eq!(body(&config, &request, false)["response_format"]["type"], "json_object");
        assert!(!strict.to_string().contains("\"k\""), "la clé ne va que dans l'en-tête");
    }

    /// Une connexion fermée sans réponse (connexion inactive coupée par le fournisseur) se refait une fois ;
    /// l'échec suivant donne sa cause, jamais un simple « injoignable ». Faux fournisseur sur la boucle locale.
    #[tokio::test]
    async fn closed_connection_is_retried_once() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let config = AiConfig {
            base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: "m".into(),
            api_key: "k".into(),
        };
        let server = tokio::spawn(async move {
            // Première connexion : fermée dès la demande lue ; seconde : une réponse.
            let (mut first, _) = listener.accept().await.unwrap();
            let _ = first.read(&mut [0; 4096]).await;
            drop(first);
            let (mut second, _) = listener.accept().await.unwrap();
            let _ = second.read(&mut [0; 65536]).await;
            let body = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            second.write_all(reply.as_bytes()).await.unwrap();
            // Troisième et quatrième : fermées, l'échec remonte.
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let _ = stream.read(&mut [0; 4096]).await;
            }
        });
        let request = AiRequest { system: "s".into(), user: "u".into(), schema_name: "r".into(), schema: json!({}) };
        let client = HttpAiClient::new().unwrap();
        assert_eq!(client.complete(&config, &request).await.unwrap(), "{}");
        let error = client.complete(&config, &request).await.unwrap_err().to_string();
        assert!(error.starts_with("connexion à l'IA interrompue"), "{error}");
        server.await.unwrap();
    }

    #[test]
    fn reply_joins_a_streamed_answer() {
        let streamed = "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\"}}]}\n\n\
                        data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"a\\\"\"}}]}\n\n\
                        : commentaire de maintien\n\n\
                        data: {\"choices\":[{\"delta\":{\"content\":\":1}\"}}]}\n\n\
                        data: [DONE]\n\n";
        assert_eq!(reply(streamed).unwrap(), "{\"a\":1}");
        let failed = "data: {\"error\":{\"message\":\"surcharge\"}}\n\n";
        assert_eq!(reply(failed).unwrap_err().to_string(), "le fournisseur d'IA s'est interrompu : surcharge");
        assert!(reply("data: [DONE]\n").is_err());
        let cut = "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"a\"},\"finish_reason\":\"length\"}]}\n\n";
        assert!(reply(cut).unwrap_err().to_string().starts_with("réponse de l'IA tronquée"));
        let fenced = r#"{"choices":[{"message":{"content":"```json\n{}\n```"}}]}"#;
        assert_eq!(reply(fenced).unwrap(), "{}", "bloc de code retiré");
        let whole = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        assert_eq!(reply(whole).unwrap(), "{}", "réponse d'un seul bloc acceptée");
    }

    #[test]
    fn content_reads_the_first_choice() {
        let response = r#"{"choices":[{"message":{"role":"assistant","content":"{\"a\":1}"}}]}"#;
        assert_eq!(content(response).unwrap(), "{\"a\":1}");
        assert!(content("{}").is_err());
    }
}

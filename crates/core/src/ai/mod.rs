use std::{
    collections::HashMap,
    fmt, io,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use crate::language_models::LanguageModel;
use crate::settings::AiSettings;
use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};
use serde_json::Value;

mod http;
pub use http::OLLAMA_CONTEXT_TOKENS;
pub use http::{
    list_models, test_cloud_key, HttpBody, HttpClient, HttpProvider, HttpRequest, HttpResponse,
    Method, UreqClient,
};

pub fn context_tokens(kind: ProviderKind) -> usize {
    match kind {
        ProviderKind::Ollama => OLLAMA_CONTEXT_TOKENS as usize,
        ProviderKind::Openai => 128_000,
        ProviderKind::Anthropic => 200_000,
        ProviderKind::Gemini => 1_000_000,
        ProviderKind::Groq => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Ollama,
    Openai,
    Anthropic,
    Groq,
    Gemini,
}
impl ProviderKind {
    pub fn supports_text(self) -> bool {
        self != Self::Groq
    }
    pub fn is_cloud(self) -> bool {
        self != Self::Ollama
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::Openai => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Groq => "Groq",
            Self::Gemini => "Gemini",
        }
    }
}

pub struct ApiKey(String);
impl ApiKey {
    pub fn new(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        (!raw.is_empty()).then(|| Self(raw.into()))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(***)")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OutputSchema {
    pub name: String,
    pub schema: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GenerateRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub schema: Option<OutputSchema>,
    pub max_tokens: u32,
    #[serde(skip_serializing)]
    pub inline_audio: Option<InlineAudio>,
}

#[derive(Clone, PartialEq)]
pub struct InlineAudio {
    pub mime_type: String,
    pub data: String,
}
impl fmt::Debug for InlineAudio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InlineAudio")
            .field("mime_type", &self.mime_type)
            .field("data", &"[redacted]")
            .finish()
    }
}

#[derive(Debug)]
pub enum AiError {
    NotConfigured,
    MissingKey(ProviderKind),
    Unreachable(String),
    KeyStore(String),
    Auth(ProviderKind),
    GeminiLimit,
    RateLimited { retry_after: std::time::Duration },
    Http { status: u16, message: String },
    Refused,
    Truncated,
    InvalidOutput(String),
    Unsupported(String),
    Cancelled,
    Io(io::Error),
}
impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => f.write_str("No AI provider is set up"),
            Self::MissingKey(kind) => write!(f, "Add your {} API key in Settings", kind.label()),
            Self::Unreachable(message) => f.write_str(message),
            Self::KeyStore(message) => write!(f, "Could not read the API key: {message}"),
            Self::Auth(kind) => write!(f, "{} rejected the API key", kind.label()),
            Self::GeminiLimit => {
                f.write_str("Gemini free limit reached, try later or pick another provider")
            }
            Self::RateLimited { .. } => f.write_str("AI provider rate limit reached"),
            Self::Http { status, message } => write!(f, "AI provider error ({status}): {message}"),
            Self::Refused => f.write_str("The model declined to answer"),
            Self::Truncated => f.write_str("The response was cut off (output limit reached)"),
            Self::InvalidOutput(message) => write!(f, "Invalid model output: {message}"),
            Self::Unsupported(message) => f.write_str(message),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Io(error) => write!(f, "AI I/O error: {error}"),
        }
    }
}
impl AiError {
    pub fn unsupported_for_text(kind: ProviderKind) -> Self {
        Self::Unsupported(format!(
            "{} is not supported for Summary or Ask yet",
            kind.label()
        ))
    }
    pub fn diagnostic_kind(&self) -> &'static str {
        match self {
            Self::MissingKey(_) | Self::KeyStore(_) => "key_store",
            Self::Unreachable(_) | Self::Io(_) => "transport",
            Self::Auth(_) => "unauthorized",
            Self::GeminiLimit => "quota",
            Self::RateLimited { .. } => "rate_limit",
            Self::Http { .. } => "http_status",
            Self::InvalidOutput(_) => "invalid_output",
            Self::Cancelled => "cancelled",
            _ => "other",
        }
    }
}
impl std::error::Error for AiError {}
impl Serialize for AiError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl From<io::Error> for AiError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub trait TextProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn model(&self) -> &str;
    fn generate(
        &self,
        request: &GenerateRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> Result<String, AiError>;
}

pub fn generate_json<T: DeserializeOwned>(
    provider: &dyn TextProvider,
    request: &GenerateRequest,
    on_delta: &mut dyn FnMut(&str),
    cancel: &AtomicBool,
) -> Result<T, AiError> {
    if request.schema.is_none() {
        return Err(AiError::InvalidOutput("A JSON schema is required".into()));
    }
    let output = provider.generate(request, on_delta, cancel)?;
    let output = output.trim();
    let output = if let Some(fenced) = output
        .strip_prefix("```json")
        .or_else(|| output.strip_prefix("```"))
    {
        fenced
            .strip_suffix("```")
            .ok_or_else(|| AiError::InvalidOutput("Unclosed JSON fence".into()))?
            .trim()
    } else {
        output
    };
    serde_json::from_str(output).map_err(|error| AiError::InvalidOutput(error.to_string()))
}

pub trait SecretStore: Send + Sync {
    fn get(&self, kind: ProviderKind) -> Result<Option<ApiKey>, String>;
    fn set(&self, kind: ProviderKind, key: &ApiKey) -> Result<(), String>;
    fn delete(&self, kind: ProviderKind) -> Result<(), String>;
}
#[derive(Default)]
pub struct MemorySecretStore(Mutex<HashMap<ProviderKind, String>>);
impl SecretStore for MemorySecretStore {
    fn get(&self, kind: ProviderKind) -> Result<Option<ApiKey>, String> {
        if !kind.is_cloud() {
            return Ok(None);
        }
        Ok(self
            .0
            .lock()
            .map_err(|_| "Secret store unavailable")?
            .get(&kind)
            .and_then(|raw| ApiKey::new(raw)))
    }
    fn set(&self, kind: ProviderKind, key: &ApiKey) -> Result<(), String> {
        if !kind.is_cloud() {
            return Err("Ollama needs no API key".into());
        }
        self.0
            .lock()
            .map_err(|_| "Secret store unavailable")?
            .insert(kind, key.expose().into());
        Ok(())
    }
    fn delete(&self, kind: ProviderKind) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|_| "Secret store unavailable")?
            .remove(&kind);
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AiStatus {
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub configured: bool,
    pub has_openai_key: bool,
    pub has_anthropic_key: bool,
    pub has_groq_key: bool,
    pub has_gemini_key: bool,
    pub summary_provider: Option<ProviderKind>,
    pub summary_model: Option<String>,
    pub summary_configured: bool,
    pub ask_provider: Option<ProviderKind>,
    pub ask_model: Option<String>,
    pub ask_configured: bool,
}
pub fn ai_status(settings: &AiSettings, secrets: &dyn SecretStore) -> AiStatus {
    let has_openai_key = secrets.get(ProviderKind::Openai).ok().flatten().is_some();
    let has_anthropic_key = secrets
        .get(ProviderKind::Anthropic)
        .ok()
        .flatten()
        .is_some();
    let has_groq_key = secrets.get(ProviderKind::Groq).ok().flatten().is_some();
    let has_gemini_key = secrets.get(ProviderKind::Gemini).ok().flatten().is_some();
    let model = settings
        .provider
        .and_then(|kind| settings.model_for(kind))
        .filter(|model| !model.trim().is_empty())
        .map(str::to_owned);
    let configured = match settings.provider {
        Some(ProviderKind::Ollama) => model.is_some(),
        Some(ProviderKind::Openai) => model.is_some() && has_openai_key,
        Some(ProviderKind::Anthropic) => model.is_some() && has_anthropic_key,
        Some(ProviderKind::Gemini) => model.is_some() && has_gemini_key,
        Some(ProviderKind::Groq) => false,
        None => false,
    };
    AiStatus {
        provider: settings.provider,
        model,
        configured,
        has_openai_key,
        has_anthropic_key,
        has_groq_key,
        has_gemini_key,
        summary_provider: None,
        summary_model: None,
        summary_configured: false,
        ask_provider: None,
        ask_model: None,
        ask_configured: false,
    }
}
pub fn model_ready(model: &LanguageModel, secrets: &dyn SecretStore) -> bool {
    model.provider.supports_text()
        && !model.model.trim().is_empty()
        && (!model.provider.is_cloud() || secrets.get(model.provider).ok().flatten().is_some())
}

pub fn configured_language_provider(
    selected: &LanguageModel,
    secrets: &dyn SecretStore,
    http: Arc<dyn HttpClient>,
) -> Result<HttpProvider, AiError> {
    if !selected.provider.supports_text() {
        return Err(AiError::unsupported_for_text(selected.provider));
    }
    if selected.model.trim().is_empty() {
        return Err(AiError::NotConfigured);
    }
    let key = if selected.provider.is_cloud() {
        Some(
            secrets
                .get(selected.provider)
                .map_err(AiError::KeyStore)?
                .ok_or(AiError::MissingKey(selected.provider))?,
        )
    } else {
        None
    };
    Ok(match selected.provider {
        ProviderKind::Ollama => HttpProvider::ollama(http, &selected.model),
        ProviderKind::Openai => {
            HttpProvider::openai(http, &selected.model, key.expect("cloud key"))
        }
        ProviderKind::Anthropic => {
            HttpProvider::anthropic(http, &selected.model, key.expect("cloud key"))
        }
        ProviderKind::Gemini => {
            HttpProvider::gemini(http, &selected.model, key.expect("cloud key"))
        }
        ProviderKind::Groq => unreachable!("text capability was checked"),
    })
}
pub fn configured_provider(
    settings: &AiSettings,
    secrets: &dyn SecretStore,
    http: Arc<dyn HttpClient>,
) -> Result<HttpProvider, AiError> {
    let kind = settings.provider.ok_or(AiError::NotConfigured)?;
    if !kind.supports_text() {
        return Err(AiError::unsupported_for_text(kind));
    }
    let model = settings
        .model_for(kind)
        .filter(|model| !model.trim().is_empty())
        .ok_or(AiError::NotConfigured)?;
    let key = if kind.is_cloud() {
        Some(
            secrets
                .get(kind)
                .map_err(AiError::KeyStore)?
                .ok_or(AiError::MissingKey(kind))?,
        )
    } else {
        None
    };
    Ok(match kind {
        ProviderKind::Ollama => HttpProvider::ollama(http, model),
        ProviderKind::Openai => HttpProvider::openai(http, model, key.expect("cloud key")),
        ProviderKind::Anthropic => HttpProvider::anthropic(http, model, key.expect("cloud key")),
        ProviderKind::Gemini => HttpProvider::gemini(http, model, key.expect("cloud key")),
        ProviderKind::Groq => unreachable!("text capability was checked"),
    })
}

/// Correction uses Gemini even when Summary and Ask use another provider.
pub fn correction_provider(
    settings: &AiSettings,
    secrets: &dyn SecretStore,
    http: Arc<dyn HttpClient>,
) -> Result<Option<HttpProvider>, AiError> {
    let Some(key) = secrets
        .get(ProviderKind::Gemini)
        .map_err(AiError::KeyStore)?
    else {
        return Ok(None);
    };
    let model = settings
        .model_for(ProviderKind::Gemini)
        .filter(|model| !model.trim().is_empty())
        .unwrap_or(crate::settings::DEFAULT_GEMINI_MODEL);
    Ok(Some(HttpProvider::gemini(http, model, key)))
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Disclosure {
    pub provider: ProviderKind,
    pub model: String,
    pub endpoint: String,
    pub system: String,
    pub messages: Vec<Message>,
    pub characters: usize,
}
pub fn disclosure(provider: &HttpProvider, request: &GenerateRequest) -> Disclosure {
    Disclosure {
        provider: provider.kind(),
        model: provider.model().to_owned(),
        endpoint: provider.endpoint(),
        system: request.system.clone(),
        messages: request.messages.clone(),
        characters: request.system.chars().count()
            + request
                .messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum::<usize>(),
    }
}

#[cfg(test)]
mod language_model_tests {
    use super::*;
    use crate::language_models::{LanguageModel, LanguageModelSettings};

    #[derive(Default)]
    struct Capture(Mutex<Vec<(String, String)>>);
    impl HttpClient for Capture {
        fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
            let model = request
                .body
                .as_ref()
                .and_then(HttpBody::json)
                .and_then(|body| body.get("model"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            self.0.lock().unwrap().push((request.url, model));
            Err(AiError::Unreachable("captured".into()))
        }
    }

    #[test]
    fn summary_and_ask_send_requests_to_their_independent_defaults() {
        let secrets = MemorySecretStore::default();
        secrets
            .set(ProviderKind::Anthropic, &ApiKey::new("key").unwrap())
            .unwrap();
        let settings = LanguageModelSettings {
            summary: Some(LanguageModel {
                provider: ProviderKind::Anthropic,
                model: "claude-summary".into(),
            }),
            ask: Some(LanguageModel {
                provider: ProviderKind::Ollama,
                model: "llama-ask".into(),
            }),
            ..Default::default()
        };
        let capture = Arc::new(Capture::default());
        let request = GenerateRequest {
            system: "test".into(),
            messages: vec![Message {
                role: Role::User,
                content: "hello".into(),
            }],
            schema: None,
            max_tokens: 16,
            inline_audio: None,
        };
        for selected in [
            settings.summary.as_ref().unwrap(),
            settings.ask.as_ref().unwrap(),
        ] {
            let provider =
                configured_language_provider(selected, &secrets, capture.clone()).unwrap();
            let _ = provider.generate(&request, &mut |_| {}, &AtomicBool::new(false));
        }
        assert_eq!(
            *capture.0.lock().unwrap(),
            [
                (
                    "https://api.anthropic.com/v1/messages".into(),
                    "claude-summary".into()
                ),
                ("http://localhost:11434/api/chat".into(), "llama-ask".into()),
            ]
        );
    }
}

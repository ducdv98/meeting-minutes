use std::{
    collections::HashMap,
    fmt, io,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

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
}

#[derive(Debug)]
pub enum AiError {
    NotConfigured,
    MissingKey(ProviderKind),
    Unreachable(String),
    KeyStore(String),
    Auth(ProviderKind),
    GeminiLimit,
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
    }
}
pub fn configured_provider(
    settings: &AiSettings,
    secrets: &dyn SecretStore,
    http: Arc<dyn HttpClient>,
) -> Result<HttpProvider, AiError> {
    let kind = settings.provider.ok_or(AiError::NotConfigured)?;
    if kind == ProviderKind::Groq {
        return Err(AiError::Unsupported(format!(
            "{} is not supported for Summary or Ask yet",
            kind.label()
        )));
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
        ProviderKind::Groq => {
            return Err(AiError::Unsupported(format!(
                "{} is not supported for Summary or Ask yet",
                kind.label()
            )))
        }
    })
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

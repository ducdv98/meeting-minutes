use super::{AiError, ApiKey, GenerateRequest, ProviderKind, TextProvider};
use serde_json::{json, Value};
pub const OLLAMA_CONTEXT_TOKENS: u32 = 8192;

use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub enum Method {
    Get,
    Post,
}
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<HttpBody>,
}
pub enum HttpBody {
    Json(Value),
    Multipart { boundary: String, bytes: Vec<u8> },
}
impl HttpBody {
    pub fn multipart_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Multipart { bytes, .. } => Some(bytes),
            Self::Json(_) => None,
        }
    }
    pub fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Multipart { .. } => None,
        }
    }
}
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn BufRead + Send>,
}
impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}
pub trait HttpClient: Send + Sync {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError>;
}
pub struct UreqClient;
impl HttpClient for UreqClient {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(120))
            .build();
        let mut call = match request.method {
            Method::Get => agent.get(&request.url),
            Method::Post => agent.post(&request.url),
        };
        for (name, value) in &request.headers {
            call = call.set(name, value);
        }
        let result = match request.body {
            Some(HttpBody::Json(body)) => call
                .set("content-type", "application/json")
                .send_string(&body.to_string()),
            Some(HttpBody::Multipart { boundary, bytes }) => call
                .set(
                    "content-type",
                    &format!("multipart/form-data; boundary={boundary}"),
                )
                .send_bytes(&bytes),
            None => call.call(),
        };
        let response = match result {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => {
                return Err(AiError::Unreachable(
                    "Could not reach the AI provider".into(),
                ))
            }
        };
        let headers = response
            .headers_names()
            .into_iter()
            .filter_map(|name| response.header(&name).map(|value| (name, value.to_owned())))
            .collect();
        Ok(HttpResponse {
            status: response.status(),
            headers,
            body: Box::new(BufReader::new(response.into_reader())),
        })
    }
}

const OLLAMA_URL: &str = "http://localhost:11434";
const OPENAI_URL: &str = "https://api.openai.com";
const ANTHROPIC_URL: &str = "https://api.anthropic.com";
const GEMINI_URL: &str = "https://generativelanguage.googleapis.com";

pub struct HttpProvider {
    http: Arc<dyn HttpClient>,
    kind: ProviderKind,
    model: String,
    key: Option<ApiKey>,
    pub base_url: String,
}
impl HttpProvider {
    pub fn ollama(http: Arc<dyn HttpClient>, model: impl Into<String>) -> Self {
        Self {
            http,
            kind: ProviderKind::Ollama,
            model: model.into(),
            key: None,
            base_url: OLLAMA_URL.into(),
        }
    }
    pub fn openai(http: Arc<dyn HttpClient>, model: impl Into<String>, key: ApiKey) -> Self {
        Self {
            http,
            kind: ProviderKind::Openai,
            model: model.into(),
            key: Some(key),
            base_url: OPENAI_URL.into(),
        }
    }
    pub fn anthropic(http: Arc<dyn HttpClient>, model: impl Into<String>, key: ApiKey) -> Self {
        Self {
            http,
            kind: ProviderKind::Anthropic,
            model: model.into(),
            key: Some(key),
            base_url: ANTHROPIC_URL.into(),
        }
    }
    pub fn gemini(http: Arc<dyn HttpClient>, model: impl Into<String>, key: ApiKey) -> Self {
        Self {
            http,
            kind: ProviderKind::Gemini,
            model: model.into(),
            key: Some(key),
            base_url: GEMINI_URL.into(),
        }
    }
    pub(super) fn endpoint(&self) -> String {
        if self.kind == ProviderKind::Gemini {
            format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
                self.base_url, self.model
            )
        } else {
            format!("{}{}", self.base_url, endpoint(self.kind))
        }
    }
    fn generate_endpoint(&self, structured: bool) -> String {
        if self.kind == ProviderKind::Gemini && structured {
            format!(
                "{}/v1beta/models/{}:generateContent",
                self.base_url, self.model
            )
        } else {
            self.endpoint()
        }
    }
}
fn gemini_schema(schema: &Value) -> Value {
    let Some(object) = schema.as_object() else {
        return schema.clone();
    };
    let mut supported = serde_json::Map::new();
    for (key, value) in object {
        match key.as_str() {
            "properties" => {
                if let Some(properties) = value.as_object() {
                    supported.insert(
                        key.clone(),
                        Value::Object(
                            properties
                                .iter()
                                .map(|(name, schema)| (name.clone(), gemini_schema(schema)))
                                .collect(),
                        ),
                    );
                }
            }
            "items" => {
                supported.insert(key.clone(), gemini_schema(value));
            }
            "type" | "description" | "format" | "enum" | "required" | "nullable" | "minimum"
            | "maximum" | "minItems" | "maxItems" => {
                supported.insert(key.clone(), value.clone());
            }
            _ => {}
        }
    }
    Value::Object(supported)
}
fn gemini_response_text(value: &Value) -> Result<String, AiError> {
    if value.pointer("/error/code") == Some(&json!(429))
        || value.pointer("/error/status") == Some(&json!("RESOURCE_EXHAUSTED"))
    {
        return Err(AiError::GeminiLimit);
    }
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Err(AiError::Http {
            status: 200,
            message: "Gemini could not generate a response".into(),
        });
    }
    match value
        .pointer("/candidates/0/finishReason")
        .and_then(Value::as_str)
    {
        Some("MAX_TOKENS") => return Err(AiError::Truncated),
        Some("STOP") | None => {}
        Some(_) => return Err(AiError::Refused),
    }
    if value.pointer("/promptFeedback/blockReason").is_some() {
        return Err(AiError::Refused);
    }
    Ok(value
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|part| part["thought"] != true)
        .filter_map(|part| part["text"].as_str())
        .collect())
}
fn endpoint(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Ollama => "/api/chat",
        ProviderKind::Openai => "/v1/chat/completions",
        ProviderKind::Anthropic => "/v1/messages",
        ProviderKind::Groq | ProviderKind::Gemini => "",
    }
}
fn headers(kind: ProviderKind, key: Option<&ApiKey>) -> Result<Vec<(String, String)>, AiError> {
    match kind {
        ProviderKind::Ollama => Ok(vec![]),
        ProviderKind::Openai => Ok(vec![(
            "Authorization".into(),
            format!("Bearer {}", key.ok_or(AiError::MissingKey(kind))?.expose()),
        )]),
        ProviderKind::Anthropic => Ok(vec![
            (
                "x-api-key".into(),
                key.ok_or(AiError::MissingKey(kind))?.expose().into(),
            ),
            ("anthropic-version".into(), "2023-06-01".into()),
        ]),
        ProviderKind::Groq => Ok(vec![(
            "Authorization".into(),
            format!("Bearer {}", key.ok_or(AiError::MissingKey(kind))?.expose()),
        )]),
        ProviderKind::Gemini => Ok(vec![(
            "x-goog-api-key".into(),
            key.ok_or(AiError::MissingKey(kind))?.expose().into(),
        )]),
    }
}
fn error_message(raw: &str, kind: ProviderKind, key: Option<&ApiKey>) -> String {
    let value: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
    let message = match kind {
        ProviderKind::Ollama => value.get("error"),
        _ => value.pointer("/error/message"),
    }
    .and_then(Value::as_str)
    .unwrap_or(raw);
    let redacted = match key {
        Some(key) => message.replace(key.expose(), "***"),
        None => message.into(),
    };
    redacted.chars().take(300).collect()
}
fn checked_response(
    response: HttpResponse,
    kind: ProviderKind,
    key: Option<&ApiKey>,
) -> Result<HttpResponse, AiError> {
    if response.status == 401 || response.status == 403 {
        crate::diagnostics::log_provider_status(kind, response.status);
        return Err(AiError::Auth(kind));
    }
    if !(200..300).contains(&response.status) {
        let status = response.status;
        let retry_after = response
            .header("retry-after")
            .and_then(|value| value.parse::<u64>().ok());
        crate::diagnostics::log_provider_status(kind, status);
        let mut raw = String::new();
        response.body.take(64 * 1024).read_to_string(&mut raw)?;
        if kind == ProviderKind::Gemini && status == 429 {
            if let Some(seconds) = retry_after {
                let reason = raw.to_ascii_lowercase();
                let daily = ["quota", "per day", "daily", "/day"]
                    .iter()
                    .any(|term| reason.contains(term));
                if seconds <= 60 && !daily {
                    return Err(AiError::RateLimited {
                        retry_after: Duration::from_secs(seconds),
                    });
                }
            }
            return Err(AiError::GeminiLimit);
        }
        if kind == ProviderKind::Gemini && raw.contains("RESOURCE_EXHAUSTED") {
            return Err(AiError::GeminiLimit);
        }
        return Err(AiError::Http {
            status,
            message: error_message(&raw, kind, key),
        });
    }
    Ok(response)
}
fn unreachable(error: AiError, kind: ProviderKind) -> AiError {
    if kind == ProviderKind::Ollama && matches!(error, AiError::Unreachable(_)) {
        AiError::Unreachable("Ollama is not running at localhost:11434".into())
    } else {
        error
    }
}
pub fn test_cloud_key(
    http: &dyn HttpClient,
    kind: ProviderKind,
    key: &ApiKey,
) -> Result<(), AiError> {
    let url = match kind {
        ProviderKind::Groq => "https://api.groq.com/openai/v1/models",
        ProviderKind::Gemini => "https://generativelanguage.googleapis.com/v1beta/models",
        _ => {
            return Err(AiError::Unsupported(format!(
                "{} has no cloud key test",
                kind.label()
            )))
        }
    };
    let response = http
        .send(HttpRequest {
            method: Method::Get,
            url: url.into(),
            headers: headers(kind, Some(key))?,
            body: None,
        })
        .inspect_err(|error| crate::diagnostics::log_provider_error(kind, error))?;
    checked_response(response, kind, Some(key))?;
    Ok(())
}
pub fn list_models(
    http: &dyn HttpClient,
    kind: ProviderKind,
    key: Option<&ApiKey>,
) -> Result<Vec<String>, AiError> {
    let url = match kind {
        ProviderKind::Ollama => format!("{OLLAMA_URL}/api/tags"),
        ProviderKind::Openai => format!("{OPENAI_URL}/v1/models"),
        ProviderKind::Anthropic => format!("{ANTHROPIC_URL}/v1/models?limit=100"),
        ProviderKind::Gemini => format!("{GEMINI_URL}/v1beta/models"),
        ProviderKind::Groq => {
            return Err(AiError::Unsupported(format!(
                "{} models are not available for Summary or Ask yet",
                kind.label()
            )))
        }
    };
    let response = http
        .send(HttpRequest {
            method: Method::Get,
            url,
            headers: headers(kind, key)?,
            body: None,
        })
        .map_err(|error| {
            crate::diagnostics::log_provider_error(kind, &error);
            unreachable(error, kind)
        })?;
    let mut response = checked_response(response, kind, key)?;
    let mut raw = String::new();
    response.body.read_to_string(&mut raw)?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|error| AiError::InvalidOutput(error.to_string()))?;
    let field = if matches!(kind, ProviderKind::Ollama | ProviderKind::Gemini) {
        "models"
    } else {
        "data"
    };
    let name = if matches!(kind, ProviderKind::Ollama | ProviderKind::Gemini) {
        "name"
    } else {
        "id"
    };
    let mut models: Vec<String> = value[field]
        .as_array()
        .ok_or_else(|| AiError::InvalidOutput("Missing models array".into()))?
        .iter()
        .filter(|item| {
            kind != ProviderKind::Gemini
                || item["supportedGenerationMethods"]
                    .as_array()
                    .is_some_and(|methods| methods.iter().any(|method| method == "generateContent"))
        })
        .filter_map(|item| item[name].as_str())
        .filter_map(|id| {
            if kind == ProviderKind::Gemini {
                id.strip_prefix("models/")
            } else {
                Some(id)
            }
        })
        .filter(|id| {
            if kind != ProviderKind::Openai {
                return true;
            }
            let allowed = id.starts_with("gpt-")
                || id
                    .strip_prefix('o')
                    .and_then(|rest| rest.chars().next())
                    .is_some_and(|char| char.is_ascii_digit());
            allowed
                && ![
                    "audio",
                    "realtime",
                    "transcribe",
                    "tts",
                    "image",
                    "search",
                    "embedding",
                ]
                .iter()
                .any(|word| id.contains(word))
        })
        .map(str::to_owned)
        .collect();
    if kind != ProviderKind::Anthropic {
        models.sort();
    }
    Ok(models)
}

impl TextProvider for HttpProvider {
    fn kind(&self) -> ProviderKind {
        self.kind
    }
    fn model(&self) -> &str {
        &self.model
    }
    fn generate(
        &self,
        request: &GenerateRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> Result<String, AiError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        let body = match self.kind {
            ProviderKind::Ollama => {
                let mut messages = vec![json!({"role":"system", "content":request.system})];
                messages.extend(request.messages.iter().map(|message| json!(message)));
                let mut body = json!({"model":self.model,"messages":messages,"stream":true,"options":{"num_predict":request.max_tokens,"num_ctx":OLLAMA_CONTEXT_TOKENS}});
                if let Some(schema) = &request.schema {
                    body["format"] = schema.schema.clone();
                }
                body
            }
            ProviderKind::Openai => {
                let mut messages = vec![json!({"role":"system", "content":request.system})];
                messages.extend(request.messages.iter().map(|message| json!(message)));
                let mut body = json!({"model":self.model,"messages":messages,"stream":true,"max_completion_tokens":request.max_tokens});
                if let Some(schema) = &request.schema {
                    body["response_format"] = json!({"type":"json_schema","json_schema":{"name":schema.name,"schema":schema.schema,"strict":true}});
                }
                body
            }
            ProviderKind::Anthropic => {
                let mut body = json!({"model":self.model,"max_tokens":request.max_tokens,"system":request.system,"messages":request.messages,"stream":true});
                if let Some(schema) = &request.schema {
                    body["output_config"] =
                        json!({"format":{"type":"json_schema","schema":schema.schema}});
                }
                body
            }
            ProviderKind::Gemini => {
                let mut contents: Vec<Value> = request.messages.iter().map(|message| json!({"role":if message.role == super::Role::Assistant { "model" } else { "user" }, "parts":[{"text":message.content}]})).collect();
                if let Some(audio) = &request.inline_audio {
                    if let Some(user) = contents
                        .iter_mut()
                        .find(|content| content["role"] == "user")
                    {
                        user["parts"].as_array_mut().unwrap().push(
                            json!({"inline_data":{"mime_type":audio.mime_type,"data":audio.data}}),
                        );
                    }
                }
                let mut body = json!({"systemInstruction":{"parts":[{"text":request.system}]},"contents":contents,"generationConfig":{"maxOutputTokens":request.max_tokens}});
                if let Some(schema) = &request.schema {
                    body["generationConfig"]["responseMimeType"] = json!("application/json");
                    body["generationConfig"]["responseSchema"] = gemini_schema(&schema.schema);
                }
                body
            }
            ProviderKind::Groq => {
                return Err(AiError::Unsupported(format!(
                    "{} is not supported for Summary or Ask yet",
                    self.kind.label()
                )))
            }
        };
        let response = self
            .http
            .send(HttpRequest {
                method: Method::Post,
                url: self.generate_endpoint(request.schema.is_some()),
                headers: headers(self.kind, self.key.as_ref())?,
                body: Some(HttpBody::Json(body)),
            })
            .map_err(|error| {
                crate::diagnostics::log_provider_error(self.kind, &error);
                unreachable(error, self.kind)
            })?;
        let mut response = checked_response(response, self.kind, self.key.as_ref())?;
        if self.kind == ProviderKind::Gemini && request.schema.is_some() {
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
            let mut raw = String::new();
            response.body.read_to_string(&mut raw)?;
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
            let value: Value = serde_json::from_str(&raw)
                .map_err(|error| AiError::InvalidOutput(error.to_string()))?;
            let output = gemini_response_text(&value)?;
            if !output.is_empty() {
                on_delta(&output);
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
            return Ok(output);
        }
        let mut output = String::new();
        let mut line = String::new();
        let mut event = String::new();
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
            line.clear();
            if response.body.read_line(&mut line)? == 0 {
                break;
            }
            let trimmed = line.trim();
            let data = if self.kind == ProviderKind::Ollama {
                trimmed
            } else {
                if let Some(name) = trimmed.strip_prefix("event:") {
                    event = name.trim().into();
                    continue;
                }
                let Some(data) = trimmed.strip_prefix("data:") else {
                    continue;
                };
                data.trim()
            };
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                break;
            }
            let value: Value = serde_json::from_str(data)
                .map_err(|error| AiError::InvalidOutput(error.to_string()))?;
            if value.get("error").is_some_and(|error| !error.is_null())
                || event == "error"
                || value["type"] == "error"
            {
                if self.kind == ProviderKind::Gemini
                    && (value.pointer("/error/code") == Some(&json!(429))
                        || value.pointer("/error/status") == Some(&json!("RESOURCE_EXHAUSTED")))
                {
                    return Err(AiError::GeminiLimit);
                }
                return Err(AiError::Http {
                    status: 200,
                    message: error_message(data, self.kind, self.key.as_ref()),
                });
            }
            let delta = match self.kind {
                ProviderKind::Ollama => {
                    if value["done_reason"] == "length" {
                        return Err(AiError::Truncated);
                    }
                    value.pointer("/message/content").and_then(Value::as_str)
                }
                ProviderKind::Openai => {
                    if value
                        .pointer("/choices/0/delta/refusal")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty())
                    {
                        return Err(AiError::Refused);
                    }
                    if value.pointer("/choices/0/finish_reason") == Some(&json!("length")) {
                        return Err(AiError::Truncated);
                    }
                    value
                        .pointer("/choices/0/delta/content")
                        .and_then(Value::as_str)
                }
                ProviderKind::Anthropic => {
                    if event == "message_delta" {
                        if value.pointer("/delta/stop_reason") == Some(&json!("refusal")) {
                            return Err(AiError::Refused);
                        }
                        if value.pointer("/delta/stop_reason") == Some(&json!("max_tokens")) {
                            return Err(AiError::Truncated);
                        }
                    }
                    if event == "message_stop" {
                        break;
                    }
                    if event == "content_block_delta"
                        && value.pointer("/delta/type") == Some(&json!("text_delta"))
                    {
                        value.pointer("/delta/text").and_then(Value::as_str)
                    } else {
                        None
                    }
                }
                ProviderKind::Gemini => {
                    let delta = gemini_response_text(&value)?;
                    if !delta.is_empty() {
                        on_delta(&delta);
                        output.push_str(&delta);
                    }
                    continue;
                }
                ProviderKind::Groq => {
                    return Err(AiError::Unsupported(format!(
                        "{} is not supported for Summary or Ask yet",
                        self.kind.label()
                    )))
                }
            };
            if let Some(delta) = delta {
                if !delta.is_empty() {
                    on_delta(delta);
                    output.push_str(delta);
                }
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        Ok(output)
    }
}

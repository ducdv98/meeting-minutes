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
    pub body: Option<Value>,
}
pub struct HttpResponse {
    pub status: u16,
    pub body: Box<dyn BufRead + Send>,
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
            Some(body) => call
                .set("content-type", "application/json")
                .send_string(&body.to_string()),
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
        Ok(HttpResponse {
            status: response.status(),
            body: Box::new(BufReader::new(response.into_reader())),
        })
    }
}

const OLLAMA_URL: &str = "http://localhost:11434";
const OPENAI_URL: &str = "https://api.openai.com";
const ANTHROPIC_URL: &str = "https://api.anthropic.com";

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
    pub(super) fn endpoint(&self) -> String {
        format!("{}{}", self.base_url, endpoint(self.kind))
    }
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
        crate::diagnostics::log_provider_status(kind, status);
        let mut raw = String::new();
        response.body.take(64 * 1024).read_to_string(&mut raw)?;
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
        ProviderKind::Groq | ProviderKind::Gemini => {
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
    let field = if kind == ProviderKind::Ollama {
        "models"
    } else {
        "data"
    };
    let name = if kind == ProviderKind::Ollama {
        "name"
    } else {
        "id"
    };
    let mut models: Vec<String> = value[field]
        .as_array()
        .ok_or_else(|| AiError::InvalidOutput("Missing models array".into()))?
        .iter()
        .filter_map(|item| item[name].as_str())
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
            ProviderKind::Groq | ProviderKind::Gemini => {
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
                url: self.endpoint(),
                headers: headers(self.kind, self.key.as_ref())?,
                body: Some(body),
            })
            .map_err(|error| {
                crate::diagnostics::log_provider_error(self.kind, &error);
                unreachable(error, self.kind)
            })?;
        let mut response = checked_response(response, self.kind, self.key.as_ref())?;
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
                ProviderKind::Groq | ProviderKind::Gemini => {
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

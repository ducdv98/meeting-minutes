use meeting_minutes_core::ai::{ai_status, ApiKey, MemorySecretStore, ProviderKind};
use meeting_minutes_core::settings::AiSettings;

#[test]
fn key_is_trimmed_and_hidden_and_empty_key_is_rejected() {
    let key = ApiKey::new("  secret-value  ").unwrap();
    assert_eq!(key.expose(), "secret-value");
    assert!(!format!("{key:?}").contains("secret-value"));
    assert!(ApiKey::new("  ").is_none());
}

#[test]
fn cloud_transcription_keys_are_stored_independently() {
    let store = MemorySecretStore::default();
    for kind in [ProviderKind::Groq, ProviderKind::Gemini] {
        assert!(kind.is_cloud());
        let key = ApiKey::new(&format!("{kind:?}-secret")).unwrap();
        use meeting_minutes_core::ai::SecretStore;
        store.set(kind, &key).unwrap();
        assert_eq!(store.get(kind).unwrap().unwrap().expose(), key.expose());
    }
    let status = ai_status(&AiSettings::default(), &store);
    assert!(status.has_groq_key && status.has_gemini_key);
    store.delete(ProviderKind::Groq).unwrap();
    assert!(store.get(ProviderKind::Groq).unwrap().is_none());
    assert!(store.get(ProviderKind::Gemini).unwrap().is_some());
}

#[test]
fn provider_status_requires_model_and_cloud_key() {
    let secrets = MemorySecretStore::default();
    let mut settings = AiSettings::default();
    assert!(!ai_status(&settings, &secrets).configured);
    settings.provider = Some(ProviderKind::Ollama);
    assert!(!ai_status(&settings, &secrets).configured);
    settings.ollama_model = Some("llama3.2".into());
    assert!(ai_status(&settings, &secrets).configured);
}

#[test]
fn gemini_cannot_be_selected_for_summary_until_text_support_exists() {
    let secrets = MemorySecretStore::default();
    let settings = AiSettings {
        provider: Some(ProviderKind::Gemini),
        ..AiSettings::default()
    };
    let error = configured_provider(&settings, &secrets, Arc::new(FakeHttp::default()))
        .err()
        .unwrap()
        .to_string();
    assert_eq!(error, "Gemini is not supported for Summary or Ask yet");
}

#[test]
fn provider_context_sizes_match_generation_budgets() {
    use meeting_minutes_core::ai::{context_tokens, OLLAMA_CONTEXT_TOKENS};
    assert_eq!(
        context_tokens(ProviderKind::Ollama),
        OLLAMA_CONTEXT_TOKENS as usize
    );
    assert_eq!(context_tokens(ProviderKind::Openai), 128_000);
    assert_eq!(context_tokens(ProviderKind::Anthropic), 200_000);
}

use meeting_minutes_core::ai::{
    configured_provider, disclosure, generate_json, list_models, test_cloud_key, AiError,
    GenerateRequest, HttpClient, HttpProvider, HttpRequest, HttpResponse, Message, Method,
    OutputSchema, Role, SecretStore, TextProvider,
};

#[test]
fn cloud_key_test_uses_one_authenticated_get_and_redacts_errors() {
    for (kind, url, header, value) in [
        (
            ProviderKind::Groq,
            "https://api.groq.com/openai/v1/models",
            "Authorization",
            "Bearer secret-123",
        ),
        (
            ProviderKind::Gemini,
            "https://generativelanguage.googleapis.com/v1beta/models",
            "x-goog-api-key",
            "secret-123",
        ),
    ] {
        let http = FakeHttp::default();
        let key = ApiKey::new("secret-123").unwrap();
        http.reply(200, "{}");
        test_cloud_key(&http, kind, &key).unwrap();
        let request = http.request();
        assert!(matches!(request.method, Method::Get));
        assert_eq!(request.url, url);
        assert!(request.body.is_none());
        assert_eq!(request.headers, vec![(header.into(), value.into())]);
        http.reply(400, r#"{"error":{"message":"secret-123 was rejected"}}"#);
        let error = test_cloud_key(&http, kind, &key).unwrap_err().to_string();
        assert!(error.contains("*** was rejected"));
        assert!(!error.contains("secret-123"));
    }
}
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::VecDeque,
    io::{BufReader, Cursor},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
struct FakeHttp {
    requests: Mutex<Vec<HttpRequest>>,
    responses: Mutex<VecDeque<Result<(u16, String), AiError>>>,
}
impl FakeHttp {
    fn reply(&self, status: u16, body: &str) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Ok((status, body.into())));
    }
    fn request(&self) -> HttpRequest {
        self.requests.lock().unwrap().pop().unwrap()
    }
}
impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        self.requests.lock().unwrap().push(request);
        let (status, body) = self.responses.lock().unwrap().pop_front().unwrap()?;
        Ok(HttpResponse {
            status,
            body: Box::new(BufReader::new(Cursor::new(body.into_bytes()))),
        })
    }
}
fn prompt() -> GenerateRequest {
    GenerateRequest {
        system: "Be concise".into(),
        messages: vec![Message {
            role: Role::User,
            content: "Hi".into(),
        }],
        schema: None,
        max_tokens: 16,
    }
}
fn schema() -> OutputSchema {
    OutputSchema {
        name: "answer".into(),
        schema: json!({"type":"object","properties":{"ok":{"type":"boolean"}},"additionalProperties":false}),
    }
}
fn generate(provider: &dyn TextProvider) -> Result<(String, Vec<String>), AiError> {
    let mut deltas: Vec<String> = Vec::new();
    let output = provider.generate(
        &prompt(),
        &mut |delta| deltas.push(delta.into()),
        &AtomicBool::new(false),
    )?;
    Ok((output, deltas))
}
#[test]
fn ollama_lists_models_and_streams_chat() {
    let http = Arc::new(FakeHttp::default());
    http.reply(200, r#"{"models":[{"name":"z"},{"name":"a"}]}"#);
    assert_eq!(
        list_models(http.as_ref(), ProviderKind::Ollama, None).unwrap(),
        ["a", "z"]
    );
    assert_eq!(http.request().url, "http://localhost:11434/api/tags");
    http.reply(200, "{\"message\":{\"content\":\"Hel\"},\"done\":false}\n{\"message\":{\"content\":\"lo\"},\"done\":false}\n{\"done\":true,\"done_reason\":\"stop\"}\n");
    let provider = HttpProvider::ollama(http.clone(), "llama3.2");
    assert_eq!(
        generate(&provider).unwrap(),
        ("Hello".into(), vec!["Hel".into(), "lo".into()])
    );
    let request = http.request();
    assert!(matches!(request.method, Method::Post));
    assert_eq!(request.url, "http://localhost:11434/api/chat");
    let body = request.body.unwrap();
    assert_eq!(
        body["messages"][0],
        json!({"role":"system","content":"Be concise"})
    );
    assert_eq!(body["messages"][1], json!({"role":"user","content":"Hi"}));
    assert_eq!(body["options"]["num_predict"], 16);
    assert_eq!(body["options"]["num_ctx"], 8192);
    assert!(body.get("format").is_none());
    http.reply(200, "{\"done\":true}\n");
    let mut structured = prompt();
    structured.schema = Some(schema());
    provider
        .generate(&structured, &mut |_| {}, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(http.request().body.unwrap()["format"], schema().schema);
    http.reply(200, "{\"done\":true,\"done_reason\":\"length\"}\n");
    assert!(matches!(generate(&provider), Err(AiError::Truncated)));
    http.reply(200, "{\"error\":\"bad model\"}\n");
    assert!(
        matches!(generate(&provider), Err(AiError::Http { status: 200, message }) if message == "bad model")
    );
}
#[test]
fn ollama_unreachable_has_actionable_message() {
    let http = FakeHttp::default();
    http.responses
        .lock()
        .unwrap()
        .push_back(Err(AiError::Unreachable("refused".into())));
    assert_eq!(
        list_models(&http, ProviderKind::Ollama, None)
            .unwrap_err()
            .to_string(),
        "Ollama is not running at localhost:11434"
    );
}
#[test]
fn openai_filters_models_streams_and_redacts_errors() {
    let http = Arc::new(FakeHttp::default());
    assert!(matches!(
        list_models(http.as_ref(), ProviderKind::Openai, None),
        Err(AiError::MissingKey(ProviderKind::Openai))
    ));
    let key = ApiKey::new("secret-123").unwrap();
    http.reply(200, r#"{"data":[{"id":"gpt-4o"},{"id":"gpt-4o-audio"},{"id":"o3"},{"id":"text-embedding"},{"id":"whisper"}]}"#);
    assert_eq!(
        list_models(http.as_ref(), ProviderKind::Openai, Some(&key)).unwrap(),
        ["gpt-4o", "o3"]
    );
    assert!(http
        .request()
        .headers
        .iter()
        .any(|(name, value)| name == "Authorization" && value == "Bearer secret-123"));
    http.reply(200, ": comment\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\ndata: [DONE]\n");
    let provider = HttpProvider::openai(http.clone(), "gpt-4o", key);
    let mut request = prompt();
    request.schema = Some(schema());
    let mut deltas: Vec<String> = Vec::new();
    assert_eq!(
        provider
            .generate(
                &request,
                &mut |delta| deltas.push(delta.into()),
                &AtomicBool::new(false)
            )
            .unwrap(),
        "Hello"
    );
    assert_eq!(deltas, ["Hel", "lo"]);
    let sent = http.request();
    assert_eq!(sent.url, "https://api.openai.com/v1/chat/completions");
    assert!(sent
        .headers
        .iter()
        .any(|(name, value)| name == "Authorization" && value == "Bearer secret-123"));
    assert_eq!(
        sent.body.unwrap()["response_format"]["json_schema"]["strict"],
        true
    );
    http.reply(
        200,
        "data: {\"choices\":[{\"delta\":{\"refusal\":\"No\"}}]}\n",
    );
    assert!(matches!(generate(&provider), Err(AiError::Refused)));
    http.reply(
        200,
        "data: {\"choices\":[{\"finish_reason\":\"length\",\"delta\":{}}]}\n",
    );
    assert!(matches!(generate(&provider), Err(AiError::Truncated)));
    http.reply(401, r#"{"error":{"message":"secret-123 rejected"}}"#);
    let error = generate(&provider).unwrap_err();
    assert!(matches!(error, AiError::Auth(ProviderKind::Openai)));
    assert!(!error.to_string().contains("secret-123"));
    http.reply(500, r#"{"error":{"message":"secret-123 failed"}}"#);
    assert!(
        matches!(generate(&provider), Err(AiError::Http { message, .. }) if message == "*** failed")
    );
}
#[test]
fn anthropic_ignores_thinking_and_honors_stop_reasons() {
    let http = Arc::new(FakeHttp::default());
    http.reply(200, r#"{"data":[{"id":"claude-new"},{"id":"claude-old"}]}"#);
    let key = ApiKey::new("secret").unwrap();
    assert_eq!(
        list_models(http.as_ref(), ProviderKind::Anthropic, Some(&key)).unwrap(),
        ["claude-new", "claude-old"]
    );
    let models_request = http.request();
    assert_eq!(
        models_request.url,
        "https://api.anthropic.com/v1/models?limit=100"
    );
    assert!(models_request
        .headers
        .iter()
        .any(|(name, value)| name == "x-api-key" && value == "secret"));
    http.reply(200, "event: content_block_delta\ndata: {\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hidden\"}}\n\nevent: content_block_delta\ndata: {\"delta\":{\"type\":\"text_delta\",\"text\":\"ready\"}}\n\nevent: message_stop\ndata: {}\n");
    let provider = HttpProvider::anthropic(http.clone(), "claude-opus-5-5", key);
    let mut request = prompt();
    request.schema = Some(schema());
    assert_eq!(
        provider
            .generate(&request, &mut |_| {}, &AtomicBool::new(false))
            .unwrap(),
        "ready"
    );
    let sent = http.request();
    assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
    assert!(sent
        .headers
        .iter()
        .any(|(name, value)| name == "x-api-key" && value == "secret"));
    assert!(sent
        .headers
        .iter()
        .any(|(name, value)| name == "anthropic-version" && value == "2023-06-01"));
    assert_eq!(
        sent.body.unwrap()["output_config"]["format"]["type"],
        "json_schema"
    );
    http.reply(200, "event: message_stop\ndata: {}\n");
    generate(&provider).unwrap();
    assert!(http.request().body.unwrap().get("output_config").is_none());
    http.reply(
        200,
        "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"refusal\"}}\n",
    );
    assert!(matches!(generate(&provider), Err(AiError::Refused)));
    http.reply(
        200,
        "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"max_tokens\"}}\n",
    );
    assert!(matches!(generate(&provider), Err(AiError::Truncated)));
    http.reply(
        200,
        "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"failed\"}}\n",
    );
    assert!(
        matches!(generate(&provider), Err(AiError::Http { status: 200, message }) if message == "failed")
    );
}
struct FakeProvider {
    output: String,
    error: Option<fn() -> AiError>,
}
impl TextProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }
    fn model(&self) -> &str {
        "fake"
    }
    fn generate(
        &self,
        _: &GenerateRequest,
        on_delta: &mut dyn FnMut(&str),
        _: &AtomicBool,
    ) -> Result<String, AiError> {
        for part in self.output.split('|') {
            on_delta(part);
        }
        match self.error {
            Some(error) => Err(error()),
            None => Ok(self.output.replace('|', "")),
        }
    }
}
#[derive(Debug, Deserialize, PartialEq)]
struct Answer {
    ok: bool,
}
#[test]
fn structured_output_accepts_json_and_a_single_fence() {
    let mut request = prompt();
    request.schema = Some(schema());
    let cancel = AtomicBool::new(false);
    for output in ["{\"ok\":true}", "```json\n{\"ok\":true}\n```"] {
        let provider = FakeProvider {
            output: output.into(),
            error: None,
        };
        assert_eq!(
            generate_json::<Answer>(&provider, &request, &mut |_| {}, &cancel).unwrap(),
            Answer { ok: true }
        );
    }
    let provider = FakeProvider {
        output: "invalid".into(),
        error: None,
    };
    assert!(matches!(
        generate_json::<Answer>(&provider, &request, &mut |_| {}, &cancel),
        Err(AiError::InvalidOutput(_))
    ));
    request.schema = None;
    assert!(matches!(
        generate_json::<Answer>(&provider, &request, &mut |_| {}, &cancel),
        Err(AiError::InvalidOutput(_))
    ));
}
#[test]
fn structured_output_streams_deltas_and_passes_provider_errors_up() {
    let mut request = prompt();
    request.schema = Some(schema());
    let cancel = AtomicBool::new(false);
    let provider = FakeProvider {
        output: "{\"ok\":|false}".into(),
        error: None,
    };
    let mut deltas: Vec<String> = Vec::new();
    assert_eq!(
        generate_json::<Answer>(
            &provider,
            &request,
            &mut |delta| deltas.push(delta.into()),
            &cancel
        )
        .unwrap(),
        Answer { ok: false }
    );
    assert_eq!(deltas, ["{\"ok\":", "false}"]);
    let provider = FakeProvider {
        output: "{\"ok\"".into(),
        error: Some(|| AiError::Truncated),
    };
    assert!(matches!(
        generate_json::<Answer>(&provider, &request, &mut |_| {}, &cancel),
        Err(AiError::Truncated)
    ));
}
#[test]
fn cancellation_stops_after_first_delta() {
    let http = Arc::new(FakeHttp::default());
    http.reply(
        200,
        "{\"message\":{\"content\":\"first\"}}\n{\"message\":{\"content\":\"second\"}}\n",
    );
    let provider = HttpProvider::ollama(http, "model");
    let cancel = AtomicBool::new(false);
    let mut deltas: Vec<String> = Vec::new();
    let result = provider.generate(
        &prompt(),
        &mut |delta| {
            deltas.push(delta.into());
            cancel.store(true, Ordering::SeqCst);
        },
        &cancel,
    );
    assert!(matches!(result, Err(AiError::Cancelled)));
    assert_eq!(deltas, ["first"]);
}
#[test]
fn status_settings_disclosure_and_key_storage_do_not_persist_secret() {
    let secrets = MemorySecretStore::default();
    let key = ApiKey::new("do-not-persist").unwrap();
    secrets.set(ProviderKind::Openai, &key).unwrap();
    let mut settings = AiSettings {
        provider: Some(ProviderKind::Openai),
        openai_model: Some("gpt-4o".into()),
        ..Default::default()
    };
    assert!(ai_status(&settings, &secrets).configured);
    assert!(!ai_status(&settings, &MemorySecretStore::default()).configured);
    assert!(matches!(
        configured_provider(
            &settings,
            &MemorySecretStore::default(),
            Arc::new(FakeHttp::default())
        ),
        Err(AiError::MissingKey(ProviderKind::Openai))
    ));
    let provider = configured_provider(&settings, &secrets, Arc::new(FakeHttp::default())).unwrap();
    let shown = disclosure(&provider, &prompt());
    assert_eq!(shown.endpoint, "https://api.openai.com/v1/chat/completions");
    assert_eq!(shown.characters, 12);
    assert_eq!(shown.messages, prompt().messages);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");
    let full = meeting_minutes_core::settings::Settings {
        ai: settings.clone(),
        ..Default::default()
    };
    full.save(&path).unwrap();
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("do-not-persist"));
    settings.provider = Some(ProviderKind::Anthropic);
    assert!(!ai_status(&settings, &secrets).configured);
    assert_eq!(
        ai_status(&settings, &secrets).model.as_deref(),
        Some("claude-opus-5-5")
    );
    assert!(!ai_status(&AiSettings::default(), &secrets).configured);
    let legacy: meeting_minutes_core::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(legacy.ai, AiSettings::default());
}

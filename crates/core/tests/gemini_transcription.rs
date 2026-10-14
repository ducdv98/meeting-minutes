use base64::{engine::general_purpose::STANDARD, Engine as _};
use meeting_minutes_core::{
    ai::{AiError, ApiKey, HttpClient, HttpRequest, HttpResponse},
    cloud::{gemini::GeminiEngine, CloudEngine, EngineError},
    transcript::Language,
};
use serde_json::json;
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};

struct FakeHttp {
    requests: Mutex<Vec<HttpRequest>>,
    replies: Mutex<VecDeque<Result<HttpResponse, AiError>>>,
}
impl FakeHttp {
    fn new(replies: Vec<Result<HttpResponse, AiError>>) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into()),
        })
    }
}
impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        self.requests.lock().unwrap().push(request);
        self.replies.lock().unwrap().pop_front().unwrap()
    }
}
fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> Result<HttpResponse, AiError> {
    Ok(HttpResponse {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        body: Box::new(Cursor::new(body.as_bytes().to_vec())),
    })
}
fn answer(sentences: serde_json::Value) -> Result<HttpResponse, AiError> {
    reply(200, &[], &json!({"candidates":[{"content":{"parts":[{"text":json!({"sentences":sentences}).to_string()}]},"finishReason":"STOP"}]}).to_string())
}
fn engine(http: Arc<FakeHttp>) -> GeminiEngine {
    GeminiEngine::new(http, ApiKey::new("secret").unwrap(), "gemini-2.5-flash")
}

#[test]
fn sends_inline_flac_and_repairs_sentence_times() {
    let http = FakeHttp::new(vec![answer(json!([
        {"start_s":0.2,"end_s":0.8,"text":" First sentence. "},
        {"start_s":0.5,"end_s":1.5,"text":"Second sentence."},
        {"start_s":1.8,"end_s":2.4,"text":"Last sentence."},
        {"start_s":1.9,"end_s":2.0,"text":"  "}
    ]))]);
    let mut engine = engine(http.clone());
    let segments = engine
        .transcribe_span(
            &vec![0.1; 2 * 16_000],
            Language::Vi,
            Some("Mai, Q3"),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (200, 800, "First sentence."),
            (800, 1500, "Second sentence."),
            (1800, 2000, "Last sentence.")
        ]
    );
    assert!(segments.iter().all(|s| s.words.is_empty()));
    let sent = http.requests.lock().unwrap().pop().unwrap();
    assert!(sent.url.ends_with("/gemini-2.5-flash:generateContent"));
    assert_eq!(sent.headers, [("x-goog-api-key".into(), "secret".into())]);
    let body = sent.body.unwrap();
    let body = body.json().unwrap();
    assert!(body["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Vietnamese"));
    assert!(body["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Mai, Q3"));
    assert_eq!(
        body["contents"][0]["parts"][1]["inline_data"]["mime_type"],
        "audio/flac"
    );
    let audio = STANDARD
        .decode(
            body["contents"][0]["parts"][1]["inline_data"]["data"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let mut reader = claxon::FlacReader::new(Cursor::new(audio)).unwrap();
    assert_eq!(reader.samples().count(), 2 * 16_000);
    assert_eq!(
        body["generationConfig"]["responseMimeType"],
        "application/json"
    );
}

#[test]
fn invalid_output_or_audio_claim_beyond_span_falls_back() {
    for response in [
        reply(
            200,
            &[],
            r#"{"candidates":[{"content":{"parts":[{"text":"not json"}]}}]}"#,
        ),
        answer(json!([{"start_s":0.0,"end_s":31.0,"text":"Too long."}])),
    ] {
        let mut engine = engine(FakeHttp::new(vec![response]));
        assert_eq!(
            engine.transcribe_span(
                &vec![0.1; 16_000],
                Language::En,
                None,
                &AtomicBool::new(false)
            ),
            Err(EngineError::InvalidResponse)
        );
    }
}

#[test]
fn maps_quota_rate_auth_and_network_errors() {
    for (response, expected) in [
        (
            reply(429, &[], r#"{"error":{"message":"Quota exhausted"}}"#),
            EngineError::QuotaExhausted,
        ),
        (
            reply(
                429,
                &[("retry-after", "3")],
                r#"{"error":{"message":"Requests per day exhausted"}}"#,
            ),
            EngineError::QuotaExhausted,
        ),
        (
            reply(
                429,
                &[("retry-after", "3")],
                r#"{"error":{"message":"rate limit"}}"#,
            ),
            EngineError::RateLimited {
                retry_after: Duration::from_secs(3),
            },
        ),
        (reply(401, &[], "{}"), EngineError::Unauthorized),
        (
            Err(AiError::Unreachable("offline".into())),
            EngineError::Offline,
        ),
    ] {
        let mut engine = engine(FakeHttp::new(vec![response]));
        assert_eq!(
            engine.transcribe_span(&[0.1; 160], Language::En, None, &AtomicBool::new(false)),
            Err(expected)
        );
    }
}

#[test]
fn preserves_http_transport_and_malformed_response_error_kinds() {
    for (response, expected) in [
        (reply(503, &[], "{}"), EngineError::HttpStatus(503)),
        (
            Err(AiError::Io(std::io::Error::other("private detail"))),
            EngineError::Transport,
        ),
        (reply(200, &[], "not json"), EngineError::InvalidResponse),
    ] {
        let mut engine = engine(FakeHttp::new(vec![response]));
        assert_eq!(
            engine.transcribe_span(&[0.1; 160], Language::En, None, &AtomicBool::new(false)),
            Err(expected)
        );
    }
}

#[test]
fn long_cloud_span_is_sent_as_pieces_and_times_use_span_origin() {
    let http = FakeHttp::new(vec![
        answer(json!([{"start_s":1.0,"end_s":2.0,"text":"Before pause."}])),
        answer(json!([{"start_s":0.0,"end_s":1.0,"text":"After pause."}])),
    ]);
    let mut samples = vec![0.1; 31 * 16_000];
    samples[25 * 16_000..25 * 16_000 + 3200].fill(0.0);
    let mut engine = engine(http.clone());
    let segments = engine
        .transcribe_span(&samples, Language::En, None, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(http.requests.lock().unwrap().len(), 2);
    assert_eq!(segments[0].start_ms, 1000);
    assert!((25_000..26_000).contains(&segments[1].start_ms));
    assert_eq!(segments[1].end_ms - segments[1].start_ms, 1000);
}

#[test]
fn real_gemini_429_bodies_tell_per_minute_from_daily_limits() {
    let per_minute = r#"{"error":{"code":429,"message":"You exceeded your current quota, please check your plan and billing details.","status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaMetric":"generativelanguage.googleapis.com/generate_content_free_tier_requests","quotaId":"GenerateRequestsPerMinutePerProjectPerModel-FreeTier"}]},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"37s"}]}}"#;
    let per_day = r#"{"error":{"code":429,"message":"You exceeded your current quota, please check your plan and billing details.","status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaMetric":"generativelanguage.googleapis.com/generate_content_free_tier_requests","quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier"}]},{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"9s"}]}}"#;
    for (body, expected) in [
        (
            per_minute,
            EngineError::RateLimited {
                retry_after: Duration::from_secs(37),
            },
        ),
        (per_day, EngineError::QuotaExhausted),
    ] {
        let mut engine = engine(FakeHttp::new(vec![reply(429, &[], body)]));
        assert_eq!(
            engine.transcribe_span(&[0.1; 160], Language::En, None, &AtomicBool::new(false)),
            Err(expected)
        );
    }
}

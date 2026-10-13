use meeting_minutes_core::{
    ai::{AiError, ApiKey, HttpClient, HttpRequest, HttpResponse},
    cloud::groq::{EngineError, GroqEngine},
    processing::SpeechEngine,
    transcript::Language,
};
use std::{
    io::Cursor,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};

struct FakeHttp {
    request: Mutex<Option<HttpRequest>>,
    response: Mutex<Option<Result<HttpResponse, AiError>>>,
}

impl FakeHttp {
    fn reply(status: u16, headers: &[(&str, &str)], body: &str) -> Arc<Self> {
        Arc::new(Self {
            request: Mutex::new(None),
            response: Mutex::new(Some(Ok(HttpResponse {
                status,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                body: Box::new(Cursor::new(body.as_bytes().to_vec())),
            }))),
        })
    }
}

impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        *self.request.lock().unwrap() = Some(request);
        self.response.lock().unwrap().take().unwrap()
    }
}

#[test]
fn sends_meeting_language_vocabulary_and_maps_word_times() {
    let http = FakeHttp::reply(
        200,
        &[],
        r#"{"language":"english","segments":[{"start":0.25,"end":1.5,"text":" Hello world."}],"words":[{"start":0.25,"end":0.6,"word":" Hello"},{"start":0.7,"end":1.5,"word":" world."}]}"#,
    );
    let engine = GroqEngine::with_boundary(
        http.clone(),
        ApiKey::new("secret").unwrap(),
        "test-boundary",
    );
    let segments = engine
        .transcribe_typed(
            &vec![0.1; 16000],
            Some(Language::En),
            Some("Alice, roadmap"),
            &AtomicBool::new(false),
        )
        .unwrap();
    let request = http.request.lock().unwrap().take().unwrap();
    assert!(request.url.ends_with("/openai/v1/audio/transcriptions"));
    assert!(request
        .headers
        .iter()
        .any(|(name, value)| name == "Authorization" && value == "Bearer secret"));
    let body = request.body.unwrap();
    let raw = body.multipart_bytes().unwrap();
    let upload = String::from_utf8_lossy(raw);
    for field in [
        "whisper-large-v3",
        "verbose_json",
        "timestamp_granularities[]",
        "word",
        "segment",
        "language",
        "en",
        "prompt",
        "Alice, roadmap",
        "audio/flac",
    ] {
        assert!(upload.contains(field), "missing {field}");
    }
    assert!(raw.windows(4).any(|bytes| bytes == b"fLaC"));
    assert_eq!(segments[0].start_ms, 250);
    assert_eq!(segments[0].end_ms, 1500);
    assert_eq!(segments[0].words[1].start_ms, 700);
    assert_eq!(segments[0].words[1].text, " world.");
    assert_eq!(engine.backend(), "Groq");
    assert_eq!(engine.device(), Some("whisper-large-v3"));
}

#[test]
fn classifies_rate_limit_and_daily_quota() {
    for (headers, body, expected) in [
        (
            vec![("retry-after", "12")],
            r#"{"error":{"message":"rate limit"}}"#,
            EngineError::RateLimited {
                retry_after: Duration::from_secs(12),
            },
        ),
        (
            vec![],
            r#"{"error":{"message":"tokens per day exhausted"}}"#,
            EngineError::QuotaExhausted,
        ),
        (
            vec![("x-ratelimit-limit-audio-seconds-day", "100")],
            "{}",
            EngineError::QuotaExhausted,
        ),
    ] {
        let http = FakeHttp::reply(429, &headers, body);
        let engine = GroqEngine::with_boundary(http, ApiKey::new("secret").unwrap(), "test");
        assert_eq!(
            engine
                .transcribe_typed(
                    &[0.1; 160],
                    Some(Language::En),
                    None,
                    &AtomicBool::new(false)
                )
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn classifies_auth_offline_and_other_errors() {
    for (status, expected) in [
        (401, EngineError::Unauthorized),
        (403, EngineError::Unauthorized),
        (500, EngineError::Other),
    ] {
        let http = FakeHttp::reply(status, &[], "{}");
        let engine = GroqEngine::with_boundary(http, ApiKey::new("secret").unwrap(), "test");
        assert_eq!(
            engine
                .transcribe_typed(
                    &[0.1; 160],
                    Some(Language::En),
                    None,
                    &AtomicBool::new(false)
                )
                .unwrap_err(),
            expected
        );
    }
    let http = Arc::new(FakeHttp {
        request: Mutex::new(None),
        response: Mutex::new(Some(Err(AiError::Unreachable("dns".into())))),
    });
    let engine = GroqEngine::with_boundary(http, ApiKey::new("secret").unwrap(), "test");
    assert_eq!(
        engine
            .transcribe_typed(
                &[0.1; 160],
                Some(Language::En),
                None,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        EngineError::Offline
    );
}

#[test]
fn detects_meeting_language_without_sending_language_field() {
    let http = FakeHttp::reply(200, &[], r#"{"language":"vietnamese","segments":[]}"#);
    let mut engine =
        GroqEngine::with_boundary(http.clone(), ApiKey::new("secret").unwrap(), "test");
    assert_eq!(engine.detect_language(&[0.1; 160]).unwrap(), Language::Vi);
    let request = http.request.lock().unwrap().take().unwrap();
    assert!(
        !String::from_utf8_lossy(request.body.unwrap().multipart_bytes().unwrap())
            .contains("name=\"language\"")
    );
}

#[test]
fn trims_meeting_vocabulary_to_groq_prompt_budget() {
    let http = FakeHttp::reply(200, &[], r#"{"language":"english","segments":[]}"#);
    let engine = GroqEngine::with_boundary(http.clone(), ApiKey::new("secret").unwrap(), "test");
    let vocabulary = "A".repeat(1000);
    engine
        .transcribe_typed(
            &[0.1; 160],
            Some(Language::En),
            Some(&vocabulary),
            &AtomicBool::new(false),
        )
        .unwrap();
    let request = http.request.lock().unwrap().take().unwrap();
    let body =
        String::from_utf8_lossy(request.body.unwrap().multipart_bytes().unwrap()).into_owned();
    assert!(body.contains(&"A".repeat(80)));
    assert!(!body.contains(&"A".repeat(81)));
}

#[test]
#[ignore = "requires GROQ_API_KEY and FLEURS_CLIP (16 kHz mono FLAC) in the environment"]
fn transcribes_live_fleurs_clip() {
    let key = ApiKey::new(&std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY")).unwrap();
    let path = std::env::var("FLEURS_CLIP").expect("FLEURS_CLIP");
    let mut reader = claxon::FlacReader::open(path).unwrap();
    assert_eq!(reader.streaminfo().sample_rate, 16_000);
    assert_eq!(reader.streaminfo().channels, 1);
    let samples: Vec<f32> = reader
        .samples()
        .map(|sample| sample.unwrap() as f32 / 32768.0)
        .collect();
    let engine = meeting_minutes_core::cloud::groq::with_default_http(key);
    let segments = engine
        .transcribe_typed(&samples, None, None, &AtomicBool::new(false))
        .unwrap();
    assert!(!segments.is_empty());
}

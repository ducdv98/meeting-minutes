use meeting_minutes_core::{
    ai::{AiError, HttpBody, HttpClient, HttpRequest, HttpResponse, Method},
    capture::Source,
    cloud::{
        managed::{
            ManagedEngine, ManagedKey, ManagedKeyState, ManagedKeyStore, MemoryManagedKeyStore,
        },
        CloudEngine, EngineError,
    },
    transcript::{EngineName, FallbackReason, Language, Transcript},
};
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Debug)]
struct Upload {
    fields: HashMap<String, String>,
    names: Vec<String>,
    bytes: Vec<u8>,
    boundary: String,
    auth: String,
    file_is_flac: bool,
}

#[derive(Clone, Copy)]
enum Reply {
    Ok,
    Language(&'static str),
    Remaining(f64),
    Empty,
    Error(u16, &'static str),
    Transport,
}

struct FakeHttp {
    uploads: Mutex<Vec<Upload>>,
    replies: Mutex<VecDeque<Reply>>,
}

impl FakeHttp {
    fn new(replies: impl IntoIterator<Item = Reply>, _refresh_denied: bool) -> Arc<Self> {
        Arc::new(Self {
            uploads: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into_iter().collect()),
        })
    }

    fn uploads(&self) -> Vec<Upload> {
        self.uploads.lock().unwrap().clone()
    }
}

fn response(status: u16, body: String) -> Result<HttpResponse, AiError> {
    Ok(HttpResponse {
        status,
        headers: Vec::new(),
        body: Box::new(Cursor::new(body.into_bytes())),
    })
}

impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        assert!(matches!(request.method, Method::Post));
        assert!(request.url.ends_with("/v1/transcriptions"));
        let auth = request
            .headers
            .iter()
            .find(|(name, _)| name == "Authorization")
            .unwrap()
            .1
            .clone();
        let Some(HttpBody::Multipart { boundary, bytes }) = request.body else {
            panic!("expected multipart upload");
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut fields = HashMap::new();
        let mut names = Vec::new();
        let mut file_is_flac = false;
        for part in text.split(&format!("--{boundary}\r\n")).skip(1) {
            let Some((header, value)) = part.split_once("\r\n\r\n") else {
                continue;
            };
            let name = header
                .split("name=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            names.push(name.to_owned());
            if name == "file" {
                file_is_flac = header.contains("filename=\"span.flac\"")
                    && header.contains("Content-Type: audio/flac")
                    && value.as_bytes().starts_with(b"fLaC");
            } else {
                fields.insert(name.into(), value.split("\r\n").next().unwrap().into());
            }
        }
        let upload = Upload {
            fields,
            names,
            bytes,
            boundary,
            auth,
            file_is_flac,
        };
        self.uploads.lock().unwrap().push(upload.clone());
        match self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Reply::Ok)
        {
            reply @ (Reply::Ok | Reply::Empty | Reply::Language(_) | Reply::Remaining(_)) => {
                let offset: u64 = upload.fields["offset_ms"].parse().unwrap();
                let duration: u64 = upload.fields["duration_ms"].parse().unwrap();
                let segments = if matches!(reply, Reply::Empty) {
                    Vec::new()
                } else {
                    vec![json!({"start_ms":offset+100,"end_ms":offset+200,"text":"speech"})]
                };
                let mut body = json!({
                    "request_id": upload.fields["request_id"],
                    "status": "completed",
                    "model": "mm-transcription-internal",
                    "duration_ms": duration + 37,
                    "source": upload.fields["source"],
                    "segments": segments
                });
                if let Reply::Language(language) = reply {
                    body["language"] = json!(language);
                }
                if let Reply::Remaining(minutes) = reply {
                    body["minutes_remaining"] = json!(minutes);
                }
                response(200, body.to_string())
            }
            Reply::Error(status, body) => response(status, body.into()),
            Reply::Transport => Err(AiError::Unreachable("offline".into())),
        }
    }
}

fn engine(http: Arc<FakeHttp>, source: Option<Source>) -> ManagedEngine {
    let store = Arc::new(MemoryManagedKeyStore::default());
    store
        .set(&ManagedKey::new("managed-secret").unwrap())
        .unwrap();
    ManagedEngine::with_boundary(
        http,
        Arc::new(ManagedKeyState::new(store)),
        "https://gateway.test",
        source,
        "test-boundary",
    )
}

fn transcribe(
    engine: &mut ManagedEngine,
) -> Result<Vec<meeting_minutes_core::transcript::RawSegment>, EngineError> {
    engine.transcribe_span(
        &[0.1; 160],
        Language::En,
        Some("private vocabulary"),
        &AtomicBool::new(false),
    )
}

#[test]
fn managed_detects_vietnamese_from_the_opening() {
    let http = FakeHttp::new([Reply::Language("vi")], false);
    let mut engine = engine(http.clone(), None);
    let audio = vec![0.1; 31 * 16_000];

    assert_eq!(
        engine
            .detect_language(&audio, &AtomicBool::new(false))
            .unwrap(),
        Some(Language::Vi)
    );
    let uploads = http.uploads();
    assert_eq!(uploads.len(), 1);
    assert_eq!(uploads[0].fields["language"], "auto");
    assert_eq!(uploads[0].fields["duration_ms"], "30000");
}

#[test]
fn managed_missing_or_unsupported_detection_falls_back() {
    for reply in [Reply::Ok, Reply::Language("xx")] {
        let http = FakeHttp::new([reply], false);
        let mut engine = engine(http.clone(), None);
        assert_eq!(
            engine
                .detect_language(&[0.1; 160], &AtomicBool::new(false))
                .unwrap(),
            None
        );
        assert_eq!(http.uploads()[0].fields["language"], "auto");
    }
}

#[test]
fn managed_reuses_detected_opening_and_sends_language_for_remaining_audio() {
    let http = FakeHttp::new([Reply::Language("vi"), Reply::Ok], false);
    let mut engine = engine(http.clone(), None);
    let mut audio = vec![0.0; 16_000];
    audio.extend(vec![0.1; 31 * 16_000]);
    audio[31 * 16_000 - 1600..31 * 16_000].fill(0.0);
    let cancel = AtomicBool::new(false);

    assert_eq!(
        engine.detect_language(&audio[16_000..], &cancel).unwrap(),
        Some(Language::Vi)
    );
    let segments = engine
        .transcribe_span(&audio, Language::Vi, None, &cancel)
        .unwrap();

    let uploads = http.uploads();
    assert_eq!(uploads.len(), 2);
    assert_eq!(uploads[0].fields["duration_ms"], "30000");
    assert_eq!(uploads[1].fields["offset_ms"], "31000");
    assert_eq!(uploads[1].fields["duration_ms"], "1000");
    assert_eq!(uploads[1].fields["language"], "vi");
    assert_eq!(segments[0].start_ms, 1100);
    assert_eq!(segments[1].start_ms, 31100);
}

#[test]
fn managed_transcribes_continuous_speech_across_detection_boundary() {
    let http = FakeHttp::new([Reply::Language("vi"), Reply::Ok], false);
    let mut engine = engine(http.clone(), None);
    let audio = vec![0.1; 31 * 16_000];
    let cancel = AtomicBool::new(false);

    assert_eq!(
        engine.detect_language(&audio, &cancel).unwrap(),
        Some(Language::Vi)
    );
    let segments = engine
        .transcribe_span(&audio, Language::Vi, None, &cancel)
        .unwrap();

    let uploads = http.uploads();
    assert_eq!(uploads.len(), 2);
    assert_eq!(uploads[1].fields["offset_ms"], "30000");
    assert_eq!(uploads[1].fields["duration_ms"], "1000");
    assert_eq!(uploads[1].fields["language"], "vi");
    assert_eq!(segments.len(), 2);
    assert_eq!(
        uploads
            .iter()
            .map(|upload| upload.fields["duration_ms"].parse::<u64>().unwrap())
            .sum::<u64>(),
        31000
    );
}

#[test]
fn disabling_managed_stops_the_next_upload() {
    let http = FakeHttp::new([], false);
    let enabled = Arc::new(AtomicBool::new(true));
    let mut engine = engine(http.clone(), None);
    engine.set_enabled_check({
        let enabled = enabled.clone();
        move || enabled.load(Ordering::SeqCst)
    });
    transcribe(&mut engine).unwrap();
    enabled.store(false, Ordering::SeqCst);
    assert_eq!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedDisabled
    );
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn queued_managed_work_checks_consent_before_its_first_upload() {
    let http = FakeHttp::new([], false);
    let enabled = Arc::new(AtomicBool::new(true));
    let mut engine = engine(http.clone(), None);
    engine.set_enabled_check({
        let enabled = enabled.clone();
        move || enabled.load(Ordering::SeqCst)
    });
    enabled.store(false, Ordering::SeqCst);
    assert_eq!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedDisabled
    );
    assert!(http.uploads().is_empty());
}

#[test]
fn disabling_during_key_read_stops_the_upload() {
    let http = FakeHttp::new([], false);
    let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut engine = engine(http.clone(), None);
    engine.set_enabled_check({
        let checks = checks.clone();
        move || checks.fetch_add(1, Ordering::SeqCst) == 0
    });
    assert_eq!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedDisabled
    );
    assert!(http.uploads().is_empty());
}

#[test]
fn source_can_change_on_one_managed_engine() {
    let http = FakeHttp::new([], false);
    let mut engine = engine(http.clone(), None);
    for source in [Some(Source::Microphone), Some(Source::SystemAudio), None] {
        engine.set_source(source);
        transcribe(&mut engine).unwrap();
    }
    let sources = http
        .uploads()
        .into_iter()
        .map(|upload| upload.fields["source"].clone())
        .collect::<Vec<_>>();
    assert_eq!(sources, ["microphone", "system", "unknown"]);
}

#[test]
fn seven_minutes_becomes_three_bounded_pieces_with_span_relative_segments() {
    let http = FakeHttp::new([], false);
    let mut engine = engine(http.clone(), Some(Source::Microphone));
    let segments = engine
        .transcribe_span(
            &vec![0.0; 7 * 60 * 16_000],
            Language::Vi,
            Some("private vocabulary"),
            &AtomicBool::new(false),
        )
        .unwrap();
    let uploads = http.uploads();
    assert_eq!(uploads.len(), 3);
    let mut previous_end = 0;
    for (index, upload) in uploads.iter().enumerate() {
        let offset: u64 = upload.fields["offset_ms"].parse().unwrap();
        let duration: u64 = upload.fields["duration_ms"].parse().unwrap();
        assert_eq!(offset, previous_end);
        assert!(duration <= 170_000);
        previous_end = offset + duration;
        assert_eq!(segments[index].start_ms, offset + 100);
        assert_eq!(segments[index].end_ms, offset + 200);
        assert!(segments[index].words.is_empty());
        assert_eq!(upload.fields["language"], "vi");
        assert_eq!(upload.fields["source"], "microphone");
        assert!(upload.file_is_flac);
        assert_eq!(upload.auth, "Bearer managed-secret");
        assert_eq!(
            Uuid::parse_str(&upload.fields["request_id"])
                .unwrap()
                .get_version_num(),
            4
        );
    }
    assert_eq!(previous_end, 420_000);
    assert_ne!(
        uploads[0].fields["request_id"],
        uploads[1].fields["request_id"]
    );
    assert_ne!(
        uploads[1].fields["request_id"],
        uploads[2].fields["request_id"]
    );
}

#[test]
fn multipart_has_exactly_the_gateway_fields_and_unknown_source() {
    let http = FakeHttp::new([], false);
    transcribe(&mut engine(http.clone(), None)).unwrap();
    let upload = &http.uploads()[0];
    let mut names = upload.names.clone();
    names.sort();
    assert_eq!(
        names,
        [
            "duration_ms",
            "file",
            "language",
            "model",
            "offset_ms",
            "request_id",
            "source"
        ]
    );
    assert_eq!(upload.fields["source"], "unknown");
    assert_eq!(upload.fields["language"], "en");
    assert_eq!(upload.fields["model"], "mm-transcription-internal");
    assert_eq!(upload.fields["duration_ms"], "10");
    assert_eq!(upload.fields["offset_ms"], "0");
}

#[test]
fn empty_segments_are_success() {
    let http = FakeHttp::new([Reply::Empty], false);
    let segments = transcribe(&mut engine(http, Some(Source::SystemAudio))).unwrap();
    assert!(segments.is_empty());
}

#[test]
fn invalid_key_is_not_retried_and_is_skipped_until_it_changes() {
    let http = FakeHttp::new([Reply::Error(401, "{}"), Reply::Ok], false);
    let store = Arc::new(MemoryManagedKeyStore::default());
    store.set(&ManagedKey::new("first-key").unwrap()).unwrap();
    let keys = Arc::new(ManagedKeyState::new(store.clone()));
    let mut first = ManagedEngine::with_boundary(
        http.clone(),
        keys.clone(),
        "https://gateway.test",
        None,
        "boundary",
    );
    assert_eq!(
        transcribe(&mut first).unwrap_err(),
        EngineError::ManagedKeyInvalid
    );
    assert!(keys.is_invalid());
    assert_eq!(http.uploads().len(), 1);
    let mut later = ManagedEngine::with_boundary(
        http.clone(),
        keys.clone(),
        "https://gateway.test",
        None,
        "boundary",
    );
    assert_eq!(
        transcribe(&mut later).unwrap_err(),
        EngineError::ManagedKeyInvalid
    );
    assert_eq!(http.uploads().len(), 1);
    store.set(&ManagedKey::new("edited-key").unwrap()).unwrap();
    assert!(!keys.is_invalid());
    transcribe(&mut later).unwrap();
    assert_eq!(http.uploads().len(), 2);
    assert_eq!(http.uploads()[1].auth, "Bearer edited-key");
}

#[test]
fn explicit_revocation_is_invalid_and_allowance_is_retained_when_reported() {
    let http = FakeHttp::new([Reply::Remaining(12.5)], false);
    let store = Arc::new(MemoryManagedKeyStore::default());
    store
        .set(&ManagedKey::new("managed-secret").unwrap())
        .unwrap();
    let keys = Arc::new(ManagedKeyState::new(store.clone()));
    let mut engine =
        ManagedEngine::with_boundary(http, keys.clone(), "https://gateway.test", None, "boundary");
    transcribe(&mut engine).unwrap();
    assert_eq!(keys.remaining_minutes(), Some(12.5));
    store.set(&ManagedKey::new("edited-key").unwrap()).unwrap();
    assert!(keys.key().unwrap().is_some());
    assert_eq!(keys.remaining_minutes(), None);

    let http = FakeHttp::new(
        [Reply::Error(
            403,
            r#"{"error":{"code":"KEY_REVOKED","retryable":false}}"#,
        )],
        false,
    );
    let mut engine = ManagedEngine::with_boundary(
        http.clone(),
        keys.clone(),
        "https://gateway.test",
        None,
        "boundary",
    );
    assert_eq!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedKeyInvalid
    );
    assert_eq!(http.uploads().len(), 1);
    assert_eq!(keys.remaining_minutes(), None);
}

#[test]
fn retry_after_waits_exactly_and_reuses_request() {
    let http = FakeHttp::new(
        [
            Reply::Error(
                503,
                r#"{"error":{"code":"PROVIDER_UNAVAILABLE","retryable":true,"retry_after_ms":1234}}"#,
            ),
            Reply::Ok,
        ],
        false,
    );
    let waits = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine(http.clone(), None);
    let recorded = waits.clone();
    engine.set_sleeper(move |duration, _: &AtomicBool| {
        recorded.lock().unwrap().push(duration);
        true
    });
    transcribe(&mut engine).unwrap();
    assert_eq!(*waits.lock().unwrap(), vec![Duration::from_millis(1234)]);
    let uploads = http.uploads();
    assert_eq!(
        uploads[0].fields["request_id"],
        uploads[1].fields["request_id"]
    );
    assert_eq!(uploads[0].bytes, uploads[1].bytes);
    assert_eq!(uploads[0].boundary, uploads[1].boundary);
}

#[test]
fn operator_capacity_moves_to_next_provider_without_retry() {
    let error = r#"{"error":{"code":"CAPACITY_EXHAUSTED","retryable":true}}"#;
    let http = FakeHttp::new(
        [
            Reply::Error(503, error),
            Reply::Error(503, error),
            Reply::Error(503, error),
            Reply::Error(503, error),
        ],
        false,
    );
    let mut engine = engine(http.clone(), None);
    engine.set_sleeper(|_: Duration, _: &AtomicBool| true);
    assert!(matches!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedCapacityExhausted(_)
    ));
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn account_admission_errors_do_not_retry_managed_transcription() {
    for (status, body, expected) in [
        (
            402,
            r#"{"error":{"code":"ALLOWANCE_EXHAUSTED","retryable":false}}"#,
            EngineError::ManagedAllowanceExhausted,
        ),
        (
            403,
            r#"{"error":{"code":"WAITLISTED","retryable":false}}"#,
            EngineError::ManagedWaitlisted,
        ),
        (
            403,
            r#"{"error":{"code":"ACCOUNT_SUSPENDED","retryable":false}}"#,
            EngineError::ManagedSuspended,
        ),
        (
            403,
            r#"{"error":{"code":"ACCOUNT_REQUIRED","retryable":false}}"#,
            EngineError::ManagedAccountRequired,
        ),
    ] {
        let http = FakeHttp::new([Reply::Error(status, body)], false);
        assert_eq!(
            transcribe(&mut engine(http.clone(), None)).unwrap_err(),
            expected
        );
        assert_eq!(http.uploads().len(), 1);
    }
}

#[test]
fn invalid_audio_is_other_without_retry() {
    let http = FakeHttp::new(
        [Reply::Error(
            400,
            r#"{"error":{"code":"INVALID_AUDIO","retryable":false}}"#,
        )],
        false,
    );
    assert_eq!(
        transcribe(&mut engine(http.clone(), None)).unwrap_err(),
        EngineError::Other
    );
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn non_json_502_and_transport_are_retried() {
    let http = FakeHttp::new(
        [
            Reply::Error(502, "bad gateway"),
            Reply::Transport,
            Reply::Ok,
        ],
        false,
    );
    let mut engine = engine(http.clone(), None);
    engine.set_sleeper(|_: Duration, _: &AtomicBool| true);
    transcribe(&mut engine).unwrap();
    let uploads = http.uploads();
    assert_eq!(uploads.len(), 3);
    assert!(uploads
        .windows(2)
        .all(|pair| pair[0].bytes == pair[1].bytes));
}

#[test]
fn cancellation_during_wait_stops_retry() {
    let http = FakeHttp::new(
        [Reply::Error(
            503,
            r#"{"error":{"code":"PROVIDER_UNAVAILABLE","retryable":true}}"#,
        )],
        false,
    );
    let cancel = AtomicBool::new(false);
    let mut engine = engine(http.clone(), None);
    engine.set_sleeper(|_, cancel: &AtomicBool| {
        cancel.store(true, Ordering::SeqCst);
        false
    });
    assert_eq!(
        engine
            .transcribe_span(&[0.1; 160], Language::En, None, &cancel)
            .unwrap_err(),
        EngineError::Cancelled
    );
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn managed_engine_name_and_fallback_round_trip_with_old_transcripts() {
    let old = json!({"schema_version":1,"revision":1,"created_at":"2026-01-01","model":["Groq"],"language":"en","language_detected":false,"duration_ms":0,"segments":[]});
    let old: Transcript = serde_json::from_value(old).unwrap();
    assert_eq!(old.model, vec![EngineName::Groq]);
    let mut managed = old;
    managed.model = vec![EngineName::Managed];
    managed.fallback_reason = Some(FallbackReason::Quota(EngineName::Managed));
    let saved = serde_json::to_value(&managed).unwrap();
    assert_eq!(saved["model"][0], "Managed transcription");
    assert_eq!(saved["fallback_reason"], "Managed transcription is busy");
    assert_eq!(EngineName::Managed.label(), "Managed transcription");
    assert_eq!(
        serde_json::from_value::<Transcript>(saved).unwrap(),
        managed
    );
    assert_eq!(
        serde_json::from_str::<EngineName>("\"managed\"").unwrap(),
        EngineName::Managed
    );
    for reason in [
        FallbackReason::ManagedAllowance,
        FallbackReason::ManagedKeyInvalid,
        FallbackReason::ManagedCapacity("2026-11-01".into()),
        FallbackReason::ManagedWaitlisted,
        FallbackReason::ManagedSuspended,
        FallbackReason::ManagedAccountRequired,
        FallbackReason::ManagedDisabled,
    ] {
        let saved = serde_json::to_string(&reason).unwrap();
        assert_eq!(
            serde_json::from_str::<FallbackReason>(&saved).unwrap(),
            reason
        );
    }
    assert_eq!(
        FallbackReason::ManagedKeyInvalid.kind(),
        "managed_key_invalid"
    );
    assert_eq!(
        FallbackReason::ManagedAccountRequired.kind(),
        "managed_account_required"
    );
    assert_eq!(
        FallbackReason::ManagedCapacity("2026-11-01".into()).reset_date(),
        Some("2026-11-01")
    );
    assert_eq!(
        serde_json::from_str::<FallbackReason>("\"Managed allowance exhausted\"")
            .unwrap()
            .kind(),
        "managed_allowance"
    );
}

#[test]
fn capacity_retry_after_does_not_pause_processing() {
    let http = FakeHttp::new(
        [
            Reply::Error(
                503,
                r#"{"error":{"code":"CAPACITY_EXHAUSTED","retryable":true,"retry_after_ms":36000000}}"#,
            ),
            Reply::Ok,
        ],
        false,
    );
    let waits = Arc::new(Mutex::new(Vec::new()));
    let mut engine = engine(http.clone(), None);
    let recorded = waits.clone();
    engine.set_sleeper(move |duration, _: &AtomicBool| {
        recorded.lock().unwrap().push(duration);
        true
    });
    assert!(matches!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedCapacityExhausted(_)
    ));
    assert!(waits.lock().unwrap().is_empty());
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn disconnecting_during_a_retry_wait_sends_nothing_more() {
    let http = FakeHttp::new(
        [
            Reply::Error(
                503,
                r#"{"error":{"code":"PROVIDER_UNAVAILABLE","retryable":true}}"#,
            ),
            Reply::Ok,
        ],
        false,
    );
    let store = Arc::new(MemoryManagedKeyStore::default());
    store
        .set(&ManagedKey::new("managed-secret").unwrap())
        .unwrap();
    let mut engine = ManagedEngine::with_boundary(
        http.clone(),
        Arc::new(ManagedKeyState::new(store.clone())),
        "https://gateway.test",
        None,
        "test-boundary",
    );
    engine.set_sleeper(move |_, _: &AtomicBool| {
        store.delete().unwrap();
        true
    });
    assert_eq!(
        transcribe(&mut engine).unwrap_err(),
        EngineError::ManagedDisabled
    );
    assert_eq!(http.uploads().len(), 1);
}

#[test]
fn a_reply_that_keeps_failing_moves_on_but_no_reply_at_all_is_offline() {
    let http = FakeHttp::new([Reply::Error(502, "<html>bad gateway</html>"); 4], false);
    let mut replied = engine(http.clone(), None);
    replied.set_sleeper(|_: Duration, _: &AtomicBool| true);
    assert_eq!(
        transcribe(&mut replied).unwrap_err(),
        EngineError::QuotaExhausted
    );
    assert_eq!(http.uploads().len(), 4);
    let http = FakeHttp::new(
        [
            Reply::Transport,
            Reply::Transport,
            Reply::Transport,
            Reply::Transport,
        ],
        false,
    );
    let mut engine = engine(http, None);
    engine.set_sleeper(|_: Duration, _: &AtomicBool| true);
    assert_eq!(transcribe(&mut engine).unwrap_err(), EngineError::Offline);
}

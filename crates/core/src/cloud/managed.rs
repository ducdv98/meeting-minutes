use crate::{
    ai::{AiError, HttpBody, HttpClient, HttpRequest, HttpResponse, Method},
    capture::Source,
    cloud::{encode_flac, CloudEngine, EngineError},
    diagnostics::{log_event, LogEvent},
    processing::cloud_span_bounds,
    transcript::{shift_segment_times, Language, RawSegment},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fmt,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub const GATEWAY_URL: &str = "https://transcribe.meeting-minutes.tech";
const REFRESH_MARGIN: Duration = Duration::from_secs(60);

pub fn base_url() -> String {
    base_url_from(std::env::var("MEETING_MINUTES_GATEWAY_URL").ok())
}

pub fn base_url_from(override_url: Option<String>) -> String {
    override_url
        .as_deref()
        .map(str::trim)
        .map(|url| url.trim_end_matches('/'))
        .filter(|url| !url.is_empty())
        .unwrap_or(GATEWAY_URL)
        .to_owned()
}

#[derive(Clone, PartialEq, Eq)]
pub struct ManagedToken(String);

impl ManagedToken {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ManagedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ManagedToken(***)")
    }
}

impl fmt::Display for ManagedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayErrorCode {
    InvalidRequest,
    InvalidEmail,
    CodeInvalid,
    CodeExpired,
    TooManyAttempts,
    EmailUnavailable,
    NoAccount,
    InvalidAudio,
    Unauthorized,
    IdempotencyConflict,
    RequestInProgress,
    FileTooLarge,
    RateLimited,
    ProviderUnavailable,
    CapacityExhausted,
    TranscriptionTimeout,
    InternalError,
    Unknown,
}

impl GatewayErrorCode {
    fn from_code(code: &str) -> Self {
        match code {
            "INVALID_REQUEST" => Self::InvalidRequest,
            "INVALID_EMAIL" => Self::InvalidEmail,
            "CODE_INVALID" => Self::CodeInvalid,
            "CODE_EXPIRED" => Self::CodeExpired,
            "TOO_MANY_ATTEMPTS" => Self::TooManyAttempts,
            "EMAIL_UNAVAILABLE" => Self::EmailUnavailable,
            "NO_ACCOUNT" => Self::NoAccount,
            "INVALID_AUDIO" => Self::InvalidAudio,
            "UNAUTHORIZED" => Self::Unauthorized,
            "IDEMPOTENCY_CONFLICT" => Self::IdempotencyConflict,
            "REQUEST_IN_PROGRESS" => Self::RequestInProgress,
            "FILE_TOO_LARGE" => Self::FileTooLarge,
            "RATE_LIMITED" => Self::RateLimited,
            "PROVIDER_UNAVAILABLE" => Self::ProviderUnavailable,
            "CAPACITY_EXHAUSTED" => Self::CapacityExhausted,
            "TRANSCRIPTION_TIMEOUT" => Self::TranscriptionTimeout,
            "INTERNAL_ERROR" => Self::InternalError,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayError {
    pub status: Option<u16>,
    pub code: GatewayErrorCode,
    pub retryable: bool,
    pub retry_after: Option<Duration>,
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Managed transcription service error: {:?}", self.code)
    }
}

impl std::error::Error for GatewayError {}

pub fn parse_gateway_error(status: u16, body: &str) -> GatewayError {
    let envelope: Option<Value> = serde_json::from_str(body).ok();
    let error = envelope.as_ref().and_then(|value| value.get("error"));
    let code = error
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str);
    let valid = code.is_some()
        && error
            .and_then(|value| value.get("retryable"))
            .and_then(Value::as_bool)
            .is_some();
    GatewayError {
        status: Some(status),
        code: code
            .map(GatewayErrorCode::from_code)
            .unwrap_or(if status == 401 {
                GatewayErrorCode::Unauthorized
            } else {
                GatewayErrorCode::Unknown
            }),
        retryable: if valid {
            error
                .and_then(|value| value.get("retryable"))
                .and_then(Value::as_bool)
                .unwrap_or(true)
        } else {
            true
        },
        retry_after: error
            .and_then(|value| value.get("retry_after_ms"))
            .and_then(Value::as_u64)
            .map(Duration::from_millis),
    }
}

fn transport_error() -> GatewayError {
    GatewayError {
        status: None,
        code: GatewayErrorCode::Unknown,
        retryable: true,
        retry_after: None,
    }
}

fn post(http: &dyn HttpClient, base: &str, path: &str, body: Value) -> Result<Value, GatewayError> {
    let response = http
        .send(HttpRequest {
            method: Method::Post,
            url: format!("{}{path}", base.trim_end_matches('/')),
            headers: vec![],
            body: Some(HttpBody::Json(body)),
        })
        .map_err(|_| transport_error())?;
    read_response(response)
}

fn read_response(mut response: HttpResponse) -> Result<Value, GatewayError> {
    let mut raw = Vec::new();
    if response
        .body
        .by_ref()
        .take(64 * 1024)
        .read_to_end(&mut raw)
        .is_err()
    {
        return Err(if response.status == 200 {
            transport_error()
        } else {
            parse_gateway_error(response.status, "")
        });
    }
    let raw = String::from_utf8_lossy(&raw);
    if response.status != 200 {
        return Err(parse_gateway_error(response.status, &raw));
    }
    serde_json::from_str(&raw).map_err(|_| transport_error())
}

#[derive(Deserialize)]
struct AuthReply {
    client_id: Option<String>,
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

fn auth_reply(value: Value) -> Result<AuthReply, GatewayError> {
    let reply: AuthReply = serde_json::from_value(value).map_err(|_| transport_error())?;
    if reply.token_type != "Bearer" || reply.access_token.is_empty() || reply.expires_in == 0 {
        return Err(transport_error());
    }
    Ok(reply)
}

pub struct Activation {
    pub client_id: String,
    pub access_token: ManagedToken,
    pub refresh_token: ManagedToken,
    pub expires_in: Duration,
}

impl fmt::Debug for Activation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Activation")
            .field("client_id", &"[redacted]")
            .field("access_token", &self.access_token)
            .field("refresh_token", &self.refresh_token)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

pub struct AccessGrant {
    pub access_token: ManagedToken,
    pub expires_in: Duration,
}

impl fmt::Debug for AccessGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessGrant")
            .field("access_token", &self.access_token)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

pub fn request_code(http: &dyn HttpClient, base: &str, email: &str) -> Result<(), GatewayError> {
    let response = http
        .send(HttpRequest {
            method: Method::Post,
            url: format!("{}/v1/auth/code", base.trim_end_matches('/')),
            headers: vec![],
            body: Some(HttpBody::Json(json!({ "email": email }))),
        })
        .map_err(|_| transport_error())?;
    if response.status == 202 {
        Ok(())
    } else if response.status == 200 {
        Err(transport_error())
    } else {
        read_response(response).map(|_| ())
    }
}

pub fn verify_code(
    http: &dyn HttpClient,
    base: &str,
    email: &str,
    code: &str,
) -> Result<Activation, GatewayError> {
    let reply = auth_reply(post(
        http,
        base,
        "/v1/auth/verify",
        json!({ "email": email, "code": code }),
    )?)?;
    let client_id = reply
        .client_id
        .filter(|value| !value.is_empty())
        .ok_or_else(transport_error)?;
    let refresh_token = reply
        .refresh_token
        .and_then(ManagedToken::new)
        .ok_or_else(transport_error)?;
    Ok(Activation {
        client_id,
        access_token: ManagedToken(reply.access_token),
        refresh_token,
        expires_in: Duration::from_secs(reply.expires_in),
    })
}

pub fn delete_account(
    http: &dyn HttpClient,
    base: &str,
    access_token: &ManagedToken,
) -> Result<(), GatewayError> {
    let response = http
        .send(HttpRequest {
            method: Method::Delete,
            url: format!("{}/v1/account", base.trim_end_matches('/')),
            headers: vec![(
                "Authorization".into(),
                format!("Bearer {}", access_token.expose()),
            )],
            body: None,
        })
        .map_err(|_| transport_error())?;
    if response.status == 204 {
        Ok(())
    } else if response.status == 200 {
        Err(transport_error())
    } else {
        read_response(response).map(|_| ())
    }
}

pub fn refresh(
    http: &dyn HttpClient,
    base: &str,
    refresh_token: &ManagedToken,
) -> Result<AccessGrant, GatewayError> {
    let reply = auth_reply(post(
        http,
        base,
        "/v1/auth/refresh",
        json!({ "refresh_token": refresh_token.expose() }),
    )?)?;
    Ok(AccessGrant {
        access_token: ManagedToken(reply.access_token),
        expires_in: Duration::from_secs(reply.expires_in),
    })
}

#[derive(Clone)]
pub struct ManagedCredential {
    pub client_id: String,
    pub refresh_token: ManagedToken,
    pub email: Option<String>,
    pub enabled: bool,
}

impl fmt::Debug for ManagedCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedCredential")
            .field("client_id", &"[redacted]")
            .field("refresh_token", &self.refresh_token)
            .finish()
    }
}

pub trait ManagedCredentialStore: Send + Sync {
    fn get(&self) -> Result<Option<ManagedCredential>, String>;
    fn set(&self, credential: &ManagedCredential) -> Result<(), String>;
    fn delete(&self) -> Result<(), String>;
}

#[derive(Default)]
pub struct MemoryManagedCredentialStore(Mutex<Option<ManagedCredential>>);

impl ManagedCredentialStore for MemoryManagedCredentialStore {
    fn get(&self) -> Result<Option<ManagedCredential>, String> {
        self.0
            .lock()
            .map(|credential| credential.clone())
            .map_err(|_| "Managed credential store unavailable".into())
    }

    fn set(&self, credential: &ManagedCredential) -> Result<(), String> {
        *self
            .0
            .lock()
            .map_err(|_| "Managed credential store unavailable")? = Some(credential.clone());
        Ok(())
    }

    fn delete(&self) -> Result<(), String> {
        *self
            .0
            .lock()
            .map_err(|_| "Managed credential store unavailable")? = None;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedAuthError {
    Disconnected,
    CredentialStoreUnavailable,
    Gateway(GatewayError),
}

impl fmt::Display for ManagedAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => f.write_str("Managed transcription is disconnected"),
            Self::CredentialStoreUnavailable => f.write_str("Managed credential store unavailable"),
            Self::Gateway(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ManagedAuthError {}

struct CachedToken {
    value: ManagedToken,
    expires_at: Instant,
}

#[derive(Default)]
struct TokenState {
    credential: Option<ManagedCredential>,
    access: Option<CachedToken>,
}

pub struct TokenManager {
    http: Arc<dyn HttpClient>,
    store: Arc<dyn ManagedCredentialStore>,
    base: String,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    state: Mutex<TokenState>,
    // Outside the state lock, so Settings never waits for a refresh in flight.
    revoked: AtomicBool,
}

impl TokenManager {
    pub fn new(
        http: Arc<dyn HttpClient>,
        store: Arc<dyn ManagedCredentialStore>,
        base: impl Into<String>,
    ) -> Self {
        Self::with_clock(http, store, base, Arc::new(Instant::now))
    }

    pub fn with_clock(
        http: Arc<dyn HttpClient>,
        store: Arc<dyn ManagedCredentialStore>,
        base: impl Into<String>,
        clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    ) -> Self {
        Self {
            http,
            store,
            base: base.into(),
            clock,
            state: Mutex::new(TokenState::default()),
            revoked: AtomicBool::new(false),
        }
    }

    pub fn is_disconnected(&self) -> bool {
        self.revoked.load(Ordering::SeqCst)
    }

    /// Forgets cached credentials and the revoked state, after Connect or Disconnect in Settings.
    pub fn reset(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = TokenState::default();
        }
        self.revoked.store(false, Ordering::SeqCst);
    }

    pub fn access_token(&self) -> Result<ManagedToken, ManagedAuthError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
        if self.is_disconnected() {
            return Err(ManagedAuthError::Disconnected);
        }
        if let Some(access) = &state.access {
            if access.expires_at.saturating_duration_since((self.clock)()) > REFRESH_MARGIN {
                return Ok(access.value.clone());
            }
        }
        self.refresh_locked(&mut state)
    }

    pub fn refresh_now(&self) -> Result<ManagedToken, ManagedAuthError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
        state.access = None;
        self.refresh_locked(&mut state)
    }

    fn refresh_locked(&self, state: &mut TokenState) -> Result<ManagedToken, ManagedAuthError> {
        if self.is_disconnected() {
            return Err(ManagedAuthError::Disconnected);
        }
        if state.credential.is_none() {
            state.credential = self
                .store
                .get()
                .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
        }
        let Some(credential) = &state.credential else {
            return Err(ManagedAuthError::Disconnected);
        };
        match refresh(self.http.as_ref(), &self.base, &credential.refresh_token) {
            Ok(grant) => {
                let value = grant.access_token;
                state.access = Some(CachedToken {
                    value: value.clone(),
                    expires_at: (self.clock)() + grant.expires_in,
                });
                Ok(value)
            }
            Err(error) if error.status == Some(401) => {
                state.access = None;
                state.credential = None;
                self.revoked.store(true, Ordering::SeqCst);
                self.store
                    .delete()
                    .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
                Err(ManagedAuthError::Disconnected)
            }
            Err(error) => Err(ManagedAuthError::Gateway(error)),
        }
    }
}

const PIECE_SAMPLES: usize = 170 * 16_000;
const CUT_SEARCH_SAMPLES: usize = 10 * 16_000;
const MAX_RETRIES: usize = 3;
const MAX_WAIT: Duration = Duration::from_secs(60);
type Sleeper = Box<dyn FnMut(Duration, &AtomicBool) -> bool + Send>;

struct DetectedOpening {
    samples: Vec<f32>,
    segments: Vec<RawSegment>,
    language: Language,
}

pub struct ManagedEngine {
    http: Arc<dyn HttpClient>,
    tokens: Arc<TokenManager>,
    base: String,
    source: Option<Source>,
    boundary: String,
    sleeper: Sleeper,
    detected_opening: Option<DetectedOpening>,
}

impl ManagedEngine {
    pub fn new(
        http: Arc<dyn HttpClient>,
        tokens: Arc<TokenManager>,
        source: Option<Source>,
    ) -> Self {
        Self::with_boundary(
            http,
            tokens,
            base_url(),
            source,
            format!("mm-{}", Uuid::new_v4()),
        )
    }

    pub fn with_boundary(
        http: Arc<dyn HttpClient>,
        tokens: Arc<TokenManager>,
        base: impl Into<String>,
        source: Option<Source>,
        boundary: impl Into<String>,
    ) -> Self {
        Self {
            http,
            tokens,
            base: base.into(),
            source,
            boundary: boundary.into(),
            sleeper: Box::new(default_sleep),
            detected_opening: None,
        }
    }

    pub fn set_sleeper(
        &mut self,
        sleeper: impl FnMut(Duration, &AtomicBool) -> bool + Send + 'static,
    ) {
        self.sleeper = Box::new(sleeper);
    }

    fn upload(
        &mut self,
        samples: &[f32],
        offset_ms: u64,
        language: Option<Language>,
        cancel: &AtomicBool,
    ) -> Result<(Vec<RawSegment>, Option<Language>), EngineError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let request_id = Uuid::new_v4();
        let audio = encode_flac(samples)?;
        let source = source_code(self.source);
        let duration_ms = samples.len() as u64 * 1000 / 16_000;
        let body = multipart(
            &self.boundary,
            &audio,
            request_id,
            source,
            offset_ms,
            duration_ms,
            language,
        );
        let mut refreshed = false;
        let mut retry = 0;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(EngineError::Cancelled);
            }
            // Asked again before every attempt, so a Disconnect stops the next one.
            let token = self.tokens.access_token().map_err(|error| {
                log_failure(auth_code(&error), request_id);
                auth_error(error)
            })?;
            let request = HttpRequest {
                method: Method::Post,
                url: format!("{}/v1/transcriptions", self.base.trim_end_matches('/')),
                headers: vec![("Authorization".into(), format!("Bearer {}", token.expose()))],
                body: Some(HttpBody::Multipart {
                    boundary: self.boundary.clone(),
                    bytes: body.clone(),
                }),
            };
            let result = send_cancellable(self.http.clone(), request, cancel);
            if cancel.load(Ordering::SeqCst) {
                return Err(EngineError::Cancelled);
            }
            let failure = match result {
                Ok((200, raw)) => match serde_json::from_slice::<UploadReply>(&raw) {
                    // duration_ms is the gateway's own measurement, so it is not compared.
                    Ok(reply) if reply.request_id == request_id.to_string() => {
                        return Ok((
                            reply
                                .segments
                                .into_iter()
                                .map(|segment| RawSegment {
                                    start_ms: segment.start_ms,
                                    end_ms: segment.end_ms,
                                    text: segment.text,
                                    words: Vec::new(),
                                })
                                .collect(),
                            reply.language.as_deref().and_then(Language::from_code),
                        ));
                    }
                    _ => UploadFailure::InvalidResponse,
                },
                Ok((401, _)) if !refreshed => {
                    log_failure("UNAUTHORIZED", request_id);
                    self.tokens.refresh_now().map_err(|error| {
                        log_failure(auth_code(&error), request_id);
                        auth_error(error)
                    })?;
                    refreshed = true;
                    // Refresh does not consume a retry or change the piece's request ID.
                    continue;
                }
                Ok((401, _)) => UploadFailure::Unauthorized,
                Ok((status, raw)) => {
                    let raw = String::from_utf8_lossy(&raw);
                    UploadFailure::Gateway(parse_gateway_error(status, &raw))
                }
                Err(AiError::Cancelled) => return Err(EngineError::Cancelled),
                Err(_) => UploadFailure::Transport,
            };
            log_failure(failure.code(), request_id);
            if !failure.retryable() {
                return Err(failure.engine_error());
            }
            if retry == MAX_RETRIES {
                return Err(failure.engine_error());
            }
            let pause = failure
                .retry_after()
                .map_or_else(|| backoff(retry, request_id), |pause| pause.min(MAX_WAIT));
            if !(self.sleeper)(pause, cancel) || cancel.load(Ordering::SeqCst) {
                return Err(EngineError::Cancelled);
            }
            retry += 1;
        }
    }
}

impl CloudEngine for ManagedEngine {
    fn name(&self) -> &str {
        "Managed transcription"
    }

    fn set_source(&mut self, source: Option<Source>) {
        self.source = source;
    }

    fn detect_language(
        &mut self,
        samples_16k: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        self.detected_opening = None;
        let opening = &samples_16k[..samples_16k.len().min(30 * 16_000)];
        let (segments, language) = self.upload(opening, 0, None, cancel)?;
        self.detected_opening = language.map(|language| DetectedOpening {
            samples: opening.to_vec(),
            segments,
            language,
        });
        Ok(language)
    }

    fn transcribe_span(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        _vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        let mut segments = Vec::new();
        if samples_16k.is_empty() {
            return Ok(segments);
        }
        let mut start = 0;
        if let Some(opening) = self.detected_opening.take() {
            let first_sound = samples_16k
                .iter()
                .position(|sample| sample.abs() > 0.01)
                .unwrap_or(0);
            let offset = if samples_16k.starts_with(&opening.samples) {
                Some(0)
            } else if samples_16k
                .get(first_sound..)
                .is_some_and(|samples| samples.starts_with(&opening.samples))
            {
                Some(first_sound)
            } else {
                None
            };
            let safe_end = opening
                .samples
                .iter()
                .rev()
                .take(1600)
                .all(|sample| sample.abs() <= 0.01);
            // A longer span can reuse the opening only when the split lands in silence.
            if opening.language == language
                && (safe_end
                    || offset
                        .is_some_and(|offset| offset + opening.samples.len() == samples_16k.len()))
            {
                if let Some(offset) = offset {
                    let offset_ms = offset as u64 * 1000 / 16_000;
                    segments.extend(opening.segments.into_iter().map(|mut segment| {
                        shift_segment_times(
                            &mut segment.start_ms,
                            &mut segment.end_ms,
                            &mut segment.words,
                            offset_ms,
                        );
                        segment
                    }));
                    start = offset + opening.samples.len();
                }
            }
        }
        for span in cloud_span_bounds(&samples_16k[start..], PIECE_SAMPLES, CUT_SEARCH_SAMPLES) {
            // Keep the gateway limit even if the splitter's selection changes later.
            for piece in (span.start..span.end).step_by(PIECE_SAMPLES) {
                let end = (piece + PIECE_SAMPLES).min(span.end);
                segments.extend(
                    self.upload(
                        &samples_16k[start + piece..start + end],
                        (start + piece) as u64 * 1000 / 16_000,
                        Some(language),
                        cancel,
                    )?
                    .0,
                );
            }
        }
        Ok(segments)
    }
}

#[derive(Deserialize)]
struct UploadReply {
    request_id: String,
    language: Option<String>,
    segments: Vec<UploadSegment>,
}

#[derive(Deserialize)]
struct UploadSegment {
    start_ms: u64,
    end_ms: u64,
    text: String,
}

fn source_code(source: Option<Source>) -> &'static str {
    match source {
        Some(Source::Microphone) => "microphone",
        Some(Source::SystemAudio) => "system",
        None => "unknown",
    }
}

fn multipart(
    boundary: &str,
    audio: &[u8],
    request_id: Uuid,
    source: &str,
    offset_ms: u64,
    duration_ms: u64,
    language: Option<Language>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(audio.len() + 1024);
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    field("request_id", &request_id.to_string());
    field("source", source);
    field("offset_ms", &offset_ms.to_string());
    field("duration_ms", &duration_ms.to_string());
    field("language", language.map_or("auto", Language::code));
    field("model", "whisper-large-v3");
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"span.flac\"\r\nContent-Type: audio/flac\r\n\r\n").as_bytes());
    body.extend_from_slice(audio);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

#[cfg(test)]
mod language_tests {
    use super::*;

    #[test]
    fn sends_supported_language_codes() {
        for language in Language::ALL {
            let body = multipart(
                "test",
                b"audio",
                Uuid::nil(),
                "microphone",
                0,
                1000,
                Some(language),
            );
            let text = String::from_utf8(body).unwrap();
            assert!(text.contains(&format!("name=\"language\"\r\n\r\n{}\r\n", language.code())));
        }
    }
}

enum UploadFailure {
    Gateway(GatewayError),
    Transport,
    InvalidResponse,
    Unauthorized,
}

impl UploadFailure {
    fn code(&self) -> &'static str {
        match self {
            Self::Gateway(error) => match error.code {
                GatewayErrorCode::InvalidRequest => "INVALID_REQUEST",
                GatewayErrorCode::InvalidEmail => "INVALID_EMAIL",
                GatewayErrorCode::CodeInvalid => "CODE_INVALID",
                GatewayErrorCode::CodeExpired => "CODE_EXPIRED",
                GatewayErrorCode::TooManyAttempts => "TOO_MANY_ATTEMPTS",
                GatewayErrorCode::EmailUnavailable => "EMAIL_UNAVAILABLE",
                GatewayErrorCode::NoAccount => "NO_ACCOUNT",
                GatewayErrorCode::InvalidAudio => "INVALID_AUDIO",
                GatewayErrorCode::Unauthorized => "UNAUTHORIZED",
                GatewayErrorCode::IdempotencyConflict => "IDEMPOTENCY_CONFLICT",
                GatewayErrorCode::RequestInProgress => "REQUEST_IN_PROGRESS",
                GatewayErrorCode::FileTooLarge => "FILE_TOO_LARGE",
                GatewayErrorCode::RateLimited => "RATE_LIMITED",
                GatewayErrorCode::ProviderUnavailable => "PROVIDER_UNAVAILABLE",
                GatewayErrorCode::CapacityExhausted => "CAPACITY_EXHAUSTED",
                GatewayErrorCode::TranscriptionTimeout => "TRANSCRIPTION_TIMEOUT",
                GatewayErrorCode::InternalError => "INTERNAL_ERROR",
                GatewayErrorCode::Unknown => "UNKNOWN",
            },
            Self::Transport => "TRANSPORT",
            Self::InvalidResponse => "INVALID_RESPONSE",
            Self::Unauthorized => "UNAUTHORIZED",
        }
    }

    fn retryable(&self) -> bool {
        match self {
            Self::Transport | Self::InvalidResponse => true,
            Self::Unauthorized => false,
            Self::Gateway(error) => {
                matches!(
                    error.code,
                    GatewayErrorCode::RateLimited
                        | GatewayErrorCode::RequestInProgress
                        | GatewayErrorCode::ProviderUnavailable
                        | GatewayErrorCode::CapacityExhausted
                        | GatewayErrorCode::TranscriptionTimeout
                ) || (error.code == GatewayErrorCode::Unknown
                    && (matches!(error.status, Some(502..=504)) || error.retryable))
            }
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Gateway(error) => error.retry_after,
            _ => None,
        }
    }

    fn engine_error(&self) -> EngineError {
        match self {
            Self::Unauthorized => EngineError::Unauthorized,
            // Only no reply at all is Offline: a reply means the network works, so the chain
            // moves on to the next engine instead of pausing.
            Self::Transport => EngineError::Offline,
            Self::InvalidResponse => EngineError::QuotaExhausted,
            Self::Gateway(error) => match error.code {
                GatewayErrorCode::RateLimited
                | GatewayErrorCode::RequestInProgress
                | GatewayErrorCode::ProviderUnavailable
                | GatewayErrorCode::CapacityExhausted
                | GatewayErrorCode::TranscriptionTimeout => EngineError::QuotaExhausted,
                GatewayErrorCode::Unauthorized => EngineError::Unauthorized,
                GatewayErrorCode::Unknown if self.retryable() => EngineError::QuotaExhausted,
                _ => EngineError::Other,
            },
        }
    }
}

fn auth_error(error: ManagedAuthError) -> EngineError {
    match error {
        ManagedAuthError::Disconnected => EngineError::Unauthorized,
        ManagedAuthError::Gateway(error) => UploadFailure::Gateway(error).engine_error(),
        ManagedAuthError::CredentialStoreUnavailable => EngineError::Other,
    }
}

fn auth_code(error: &ManagedAuthError) -> &'static str {
    match error {
        ManagedAuthError::Disconnected => "UNAUTHORIZED",
        ManagedAuthError::Gateway(error) => UploadFailure::Gateway(error.clone()).code(),
        ManagedAuthError::CredentialStoreUnavailable => "UNKNOWN",
    }
}

fn log_failure(code: &'static str, request_id: Uuid) {
    log_event(LogEvent::ManagedUploadFailure { code, request_id });
}

fn backoff(retry: usize, request_id: Uuid) -> Duration {
    let base_ms = 2_000_u64 << retry;
    let random = Uuid::new_v4().as_u128() as u64 ^ request_id.as_u128() as u64;
    Duration::from_millis(base_ms + random % (base_ms / 4 + 1)).min(MAX_WAIT)
}

fn default_sleep(duration: Duration, cancel: &AtomicBool) -> bool {
    let mut remaining = duration;
    while !remaining.is_zero() {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        let step = remaining.min(Duration::from_millis(100));
        thread::sleep(step);
        remaining -= step;
    }
    !cancel.load(Ordering::SeqCst)
}

fn send_cancellable(
    http: Arc<dyn HttpClient>,
    request: HttpRequest,
    cancel: &AtomicBool,
) -> Result<(u16, Vec<u8>), AiError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = http.send(request).and_then(|mut response| {
            let mut raw = Vec::new();
            response
                .body
                .by_ref()
                .take(16 * 1024 * 1024)
                .read_to_end(&mut raw)?;
            Ok((response.status, raw))
        });
        let _ = sender.send(result);
    });
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AiError::Unreachable("Managed request ended".into()))
            }
        }
    }
}

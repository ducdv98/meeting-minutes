use crate::ai::{HttpBody, HttpClient, HttpRequest, HttpResponse, Method};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fmt,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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
        code: if status == 401 {
            GatewayErrorCode::Unauthorized
        } else {
            code.map(GatewayErrorCode::from_code)
                .unwrap_or(GatewayErrorCode::Unknown)
        },
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

pub fn activate(
    http: &dyn HttpClient,
    base: &str,
    invite_code: &str,
) -> Result<Activation, GatewayError> {
    let reply = auth_reply(post(
        http,
        base,
        "/v1/auth/activate",
        json!({ "invite_code": invite_code }),
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
    disconnected: bool,
}

pub struct TokenManager {
    http: Arc<dyn HttpClient>,
    store: Arc<dyn ManagedCredentialStore>,
    base: String,
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    state: Mutex<TokenState>,
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
        }
    }

    pub fn is_disconnected(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.disconnected)
            .unwrap_or(true)
    }

    /// Forgets cached credentials and the revoked state, after Connect or Disconnect in Settings.
    pub fn reset(&self) {
        if let Ok(mut state) = self.state.lock() {
            *state = TokenState::default();
        }
    }

    pub fn access_token(&self) -> Result<ManagedToken, ManagedAuthError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
        if state.disconnected {
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
        if state.disconnected {
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
                state.disconnected = true;
                self.store
                    .delete()
                    .map_err(|_| ManagedAuthError::CredentialStoreUnavailable)?;
                Err(ManagedAuthError::Disconnected)
            }
            Err(error) => Err(ManagedAuthError::Gateway(error)),
        }
    }
}

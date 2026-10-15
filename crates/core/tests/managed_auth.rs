use meeting_minutes_core::{
    ai::{AiError, HttpClient, HttpRequest, HttpResponse, Method},
    cloud::managed::{
        base_url_from, delete_account, get_account, parse_gateway_error, refresh, request_code,
        verify_code, verify_code_with_token, GatewayErrorCode, ManagedAuthError, ManagedCredential,
        ManagedCredentialStore, ManagedToken, MemoryManagedCredentialStore, TokenManager,
        GATEWAY_URL,
    },
};
use std::{
    collections::VecDeque,
    io::Cursor,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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

    fn requests(&self) -> Vec<HttpRequest> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }
}

impl HttpClient for FakeHttp {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError> {
        self.requests.lock().unwrap().push(request);
        self.replies.lock().unwrap().pop_front().unwrap()
    }
}

fn reply(status: u16, body: &str) -> Result<HttpResponse, AiError> {
    Ok(HttpResponse {
        status,
        headers: vec![],
        body: Box::new(Cursor::new(body.as_bytes().to_vec())),
    })
}

fn activation_reply() -> Result<HttpResponse, AiError> {
    reply(
        200,
        r#"{"client_id":"private-client","access_token":"private-access","token_type":"Bearer","expires_in":900,"refresh_token":"private-refresh"}"#,
    )
}

#[test]
fn reads_account_allowance_and_waitlist_position() {
    let http = FakeHttp::new(vec![reply(
        200,
        r#"{"email":"person@example.com","state":"waitlisted","minutes_used":0.5,"minutes_remaining":119.5,"allowance_minutes":120,"resets_at":"2026-11-01T00:00:00.000Z","waitlist_position":3}"#,
    )]);
    let account = get_account(
        http.as_ref(),
        GATEWAY_URL,
        &ManagedToken::new("access").unwrap(),
    )
    .unwrap();
    assert_eq!(account.state, "waitlisted");
    assert_eq!(account.minutes_remaining, 119.5);
    assert_eq!(account.waitlist_position, Some(3));
    let request = http.requests().remove(0);
    assert!(matches!(request.method, Method::Get));
    assert_eq!(
        request.headers,
        vec![("Authorization".into(), "Bearer access".into())]
    );
}

#[test]
fn legacy_connection_without_an_account_keeps_its_distinct_error() {
    let http = FakeHttp::new(vec![reply(
        409,
        r#"{"error":{"code":"NO_ACCOUNT","retryable":false}}"#,
    )]);
    let error = get_account(
        http.as_ref(),
        GATEWAY_URL,
        &ManagedToken::new("legacy-access").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error.code, GatewayErrorCode::NoAccount);
}

#[test]
fn links_a_legacy_installation_when_verifying_an_email_code() {
    let http = FakeHttp::new(vec![activation_reply()]);
    verify_code_with_token(
        http.as_ref(),
        GATEWAY_URL,
        "person@example.com",
        "123456",
        Some(&ManagedToken::new("legacy-access").unwrap()),
    )
    .unwrap();
    assert_eq!(
        http.requests()[0].headers,
        vec![("Authorization".into(), "Bearer legacy-access".into())]
    );
}

fn refresh_reply(token: &str) -> Result<HttpResponse, AiError> {
    reply(
        200,
        &format!(r#"{{"access_token":"{token}","token_type":"Bearer","expires_in":900}}"#),
    )
}

fn credential() -> ManagedCredential {
    ManagedCredential {
        client_id: "private-client".into(),
        refresh_token: ManagedToken::new("private-refresh").unwrap(),
        email: None,
        enabled: true,
    }
}

fn manager(
    http: Arc<FakeHttp>,
    store: Arc<MemoryManagedCredentialStore>,
) -> (Arc<TokenManager>, Arc<Mutex<Instant>>) {
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock = {
        let now = now.clone();
        Arc::new(move || *now.lock().unwrap())
    };
    (
        Arc::new(TokenManager::with_clock(http, store, GATEWAY_URL, clock)),
        now,
    )
}

#[test]
fn requests_an_email_code_and_verifies_into_the_existing_token_shape() {
    let http = FakeHttp::new(vec![
        reply(202, r#"{"message":"sent"}"#),
        activation_reply(),
    ]);
    request_code(http.as_ref(), GATEWAY_URL, "person@example.com").unwrap();
    let activation =
        verify_code(http.as_ref(), GATEWAY_URL, "person@example.com", "123456").unwrap();
    assert_eq!(activation.client_id, "private-client");
    assert_eq!(activation.refresh_token.expose(), "private-refresh");
    let requests = http.requests();
    assert_eq!(requests[0].url, format!("{GATEWAY_URL}/v1/auth/code"));
    assert_eq!(
        requests[0].body.as_ref().unwrap().json().unwrap(),
        &serde_json::json!({"email":"person@example.com"})
    );
    assert_eq!(requests[1].url, format!("{GATEWAY_URL}/v1/auth/verify"));
    assert!(requests[1].headers.is_empty());
    assert_eq!(
        requests[1].body.as_ref().unwrap().json().unwrap(),
        &serde_json::json!({"email":"person@example.com","code":"123456"})
    );
}

#[test]
fn deletes_account_with_bearer_token_and_accepts_an_empty_success() {
    let http = FakeHttp::new(vec![reply(204, "")]);
    delete_account(
        http.as_ref(),
        GATEWAY_URL,
        &ManagedToken::new("private-access").unwrap(),
    )
    .unwrap();
    let requests = http.requests();
    assert!(matches!(requests[0].method, Method::Delete));
    assert_eq!(requests[0].url, format!("{GATEWAY_URL}/v1/account"));
    assert_eq!(
        requests[0].headers,
        vec![("Authorization".into(), "Bearer private-access".into())]
    );
    assert!(requests[0].body.is_none());
}

#[test]
fn distinguishes_account_errors_even_when_the_status_is_401() {
    for (status, code, expected) in [
        (401, "CODE_INVALID", GatewayErrorCode::CodeInvalid),
        (401, "CODE_EXPIRED", GatewayErrorCode::CodeExpired),
        (429, "TOO_MANY_ATTEMPTS", GatewayErrorCode::TooManyAttempts),
        (409, "NO_ACCOUNT", GatewayErrorCode::NoAccount),
    ] {
        let error = parse_gateway_error(
            status,
            &format!(r#"{{"error":{{"code":"{code}","retryable":false}}}}"#),
        );
        assert_eq!(error.code, expected);
    }
}

#[test]
fn verify_code_returns_credentials() {
    let http = FakeHttp::new(vec![activation_reply()]);
    let result = verify_code(
        http.as_ref(),
        "https://test.example/",
        "person@example.com",
        "123456",
    )
    .unwrap();
    assert_eq!(result.client_id, "private-client");
    assert_eq!(result.access_token.expose(), "private-access");
    assert_eq!(result.refresh_token.expose(), "private-refresh");
    assert_eq!(result.expires_in, Duration::from_secs(900));
    let requests = http.requests();
    assert_eq!(requests.len(), 1);
    assert!(matches!(requests[0].method, Method::Post));
    assert_eq!(requests[0].url, "https://test.example/v1/auth/verify");
    assert_eq!(
        requests[0].body.as_ref().unwrap().json().unwrap(),
        &serde_json::json!({ "email": "person@example.com", "code": "123456" })
    );
    let printed = format!("{result:?}");
    for secret in [
        "private-client",
        "private-access",
        "private-refresh",
        "123456",
    ] {
        assert!(!printed.contains(secret));
    }
}

#[test]
fn verify_code_401_is_unauthorized() {
    let http = FakeHttp::new(vec![reply(
        401,
        r#"{"error":{"code":"UNAUTHORIZED","message":"used","retryable":false}}"#,
    )]);
    let error =
        verify_code(http.as_ref(), GATEWAY_URL, "person@example.com", "123456").unwrap_err();
    assert_eq!(error.status, Some(401));
    assert_eq!(error.code, GatewayErrorCode::Unauthorized);
    assert!(!error.retryable);
}

#[test]
fn direct_refresh_uses_stored_token_and_does_not_rotate_it() {
    let http = FakeHttp::new(vec![refresh_reply("second-access")]);
    let token = ManagedToken::new("private-refresh").unwrap();
    let grant = refresh(http.as_ref(), GATEWAY_URL, &token).unwrap();
    assert_eq!(grant.access_token.expose(), "second-access");
    assert_eq!(grant.expires_in, Duration::from_secs(900));
    let requests = http.requests();
    assert_eq!(requests[0].url, format!("{GATEWAY_URL}/v1/auth/refresh"));
    assert_eq!(
        requests[0].body.as_ref().unwrap().json().unwrap(),
        &serde_json::json!({ "refresh_token": "private-refresh" })
    );
    assert!(!format!("{grant:?}").contains("second-access"));
}

#[test]
fn refreshes_at_60_seconds_before_expiry_and_not_earlier() {
    let http = FakeHttp::new(vec![
        refresh_reply("first-access"),
        refresh_reply("second-access"),
    ]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    store.set(&credential()).unwrap();
    let (manager, now) = manager(http.clone(), store);
    assert_eq!(manager.access_token().unwrap().expose(), "first-access");
    assert_eq!(http.requests().len(), 1);
    *now.lock().unwrap() += Duration::from_secs(839);
    assert_eq!(manager.access_token().unwrap().expose(), "first-access");
    assert!(http.requests().is_empty());
    *now.lock().unwrap() += Duration::from_secs(1);
    assert_eq!(manager.access_token().unwrap().expose(), "second-access");
    assert_eq!(http.requests().len(), 1);
}

#[test]
fn refresh_now_forces_a_new_token() {
    let http = FakeHttp::new(vec![
        refresh_reply("first-access"),
        refresh_reply("second-access"),
    ]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    store.set(&credential()).unwrap();
    let (manager, _) = manager(http.clone(), store);
    assert_eq!(manager.access_token().unwrap().expose(), "first-access");
    assert_eq!(manager.refresh_now().unwrap().expose(), "second-access");
    assert_eq!(manager.access_token().unwrap().expose(), "second-access");
    assert_eq!(http.requests().len(), 2);
}

#[test]
fn revoked_refresh_clears_credentials_and_disconnects() {
    let http = FakeHttp::new(vec![
        refresh_reply("first-access"),
        reply(
            401,
            r#"{"error":{"code":"UNAUTHORIZED","message":"revoked","retryable":false}}"#,
        ),
    ]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    store.set(&credential()).unwrap();
    let (manager, _) = manager(http.clone(), store.clone());
    manager.access_token().unwrap();
    assert_eq!(
        manager.refresh_now().unwrap_err(),
        ManagedAuthError::Disconnected
    );
    assert!(manager.is_disconnected());
    assert!(store.get().unwrap().is_none());
    assert_eq!(
        manager.access_token().unwrap_err(),
        ManagedAuthError::Disconnected
    );
    assert_eq!(http.requests().len(), 2);
}

#[test]
fn revoked_refresh_disconnects_even_when_the_body_is_not_utf8() {
    let http = FakeHttp::new(vec![Ok(HttpResponse {
        status: 401,
        headers: vec![],
        body: Box::new(Cursor::new(vec![0xff, 0xfe])),
    })]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    store.set(&credential()).unwrap();
    let (manager, _) = manager(http, store.clone());
    assert_eq!(
        manager.access_token().unwrap_err(),
        ManagedAuthError::Disconnected
    );
    assert!(store.get().unwrap().is_none());
}

#[test]
fn parses_gateway_error_envelope_and_retry_after() {
    let error = parse_gateway_error(
        503,
        r#"{"error":{"code":"CAPACITY_EXHAUSTED","message":"wait","retryable":true,"retry_after_ms":42000}}"#,
    );
    assert_eq!(error.code, GatewayErrorCode::CapacityExhausted);
    assert!(error.retryable);
    assert_eq!(error.retry_after, Some(Duration::from_millis(42_000)));
    assert_eq!(
        parse_gateway_error(400, r#"{"error":{"code":"FUTURE_CODE","retryable":false}}"#).code,
        GatewayErrorCode::Unknown
    );
}

#[test]
fn non_json_and_transport_failures_are_retryable() {
    let html = FakeHttp::new(vec![reply(502, "<html>proxy error</html>")]);
    let error =
        verify_code(html.as_ref(), GATEWAY_URL, "person@example.com", "123456").unwrap_err();
    assert!(error.retryable);
    assert_eq!(error.code, GatewayErrorCode::Unknown);
    assert!(!format!("{error:?} {error}").contains("proxy error"));

    let offline = FakeHttp::new(vec![Err(AiError::Unreachable("123456".into()))]);
    let error = verify_code(
        offline.as_ref(),
        GATEWAY_URL,
        "person@example.com",
        "123456",
    )
    .unwrap_err();
    assert!(error.retryable);
    assert_eq!(error.status, None);
    assert!(!format!("{error:?} {error}").contains("123456"));
}

#[test]
fn base_url_override_is_pure_and_trims_trailing_slashes() {
    assert_eq!(base_url_from(None), GATEWAY_URL);
    assert_eq!(
        base_url_from(Some("https://test.example///".into())),
        "https://test.example"
    );
    assert_eq!(base_url_from(Some("  ".into())), GATEWAY_URL);
}

#[test]
fn tokens_and_credentials_redact_debug_and_display() {
    let credential = credential();
    let token = ManagedToken::new("private-access").unwrap();
    for printed in [
        format!("{credential:?}"),
        format!("{token:?}"),
        format!("{token}"),
    ] {
        assert!(!printed.contains("private-client"));
        assert!(!printed.contains("private-refresh"));
        assert!(!printed.contains("private-access"));
    }
}

#[test]
fn shared_manager_runs_only_one_initial_refresh() {
    let http = FakeHttp::new(vec![refresh_reply("first-access")]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    store.set(&credential()).unwrap();
    let (manager, _) = manager(http.clone(), store);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let manager = manager.clone();
            scope.spawn(move || {
                assert_eq!(manager.access_token().unwrap().expose(), "first-access")
            });
        }
    });
    assert_eq!(http.requests().len(), 1);
}

#[test]
fn reset_reconnects_after_revocation_and_after_starting_unconnected() {
    let http = FakeHttp::new(vec![
        reply(
            401,
            r#"{"error":{"code":"UNAUTHORIZED","message":"revoked","retryable":false}}"#,
        ),
        refresh_reply("second-access"),
    ]);
    let store = Arc::new(MemoryManagedCredentialStore::default());
    let (manager, _) = manager(http.clone(), store.clone());
    assert_eq!(manager.access_token(), Err(ManagedAuthError::Disconnected));
    assert!(!manager.is_disconnected());
    store.set(&credential()).unwrap();
    assert_eq!(manager.access_token(), Err(ManagedAuthError::Disconnected));
    assert!(manager.is_disconnected());
    store.set(&credential()).unwrap();
    assert_eq!(manager.access_token(), Err(ManagedAuthError::Disconnected));
    manager.reset();
    assert_eq!(manager.access_token().unwrap().expose(), "second-access");
}

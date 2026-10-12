# Ticket 12 design: AI providers (Ollama and BYOK)

Ticket: .scratch/windows-mvp/issues/12-ai-providers.md. Spec: ../spec.md ("AI providers", ADR-0002 local-first, cloud only on an
explicit click). Vocabulary: CONTEXT.md. Earlier designs: 05–11-core-design.md.
Existing code: crates/core/src/{settings.rs, models.rs (ureq 2 usage), library.rs (`set_language` = Value-based meeting.json edit)},
src-tauri/src/main.rs, src/{SettingsView.tsx, MeetingView.tsx, App.tsx, processing.ts}.

Summary (13) and Ask (14) are NOT built here. This ticket delivers the provider-neutral text-generation layer they will call, the
settings/key storage, the cloud consent record + disclosure UI building block, and the "no provider" guidance.

## Dependencies

- core: none new (ureq 2 with rustls, serde_json already present).
- src-tauri: `[target.'cfg(windows)'.dependencies] keyring = { version = "=3.6.3", features = ["windows-native"] }` (Windows Credential
  Manager). Non-Windows builds use an in-memory store (dev only). No other new crates.

## Core: `ai` (pub mod, new file crates/core/src/ai.rs; split into ai/{mod,ollama,openai,anthropic}.rs if it grows past ~400 lines)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind { Ollama, Openai, Anthropic }     // json "ollama" | "openai" | "anthropic"
impl ProviderKind { pub fn is_cloud(self) -> bool; pub fn label(self) -> &'static str }  // "Ollama" | "OpenAI" | "Anthropic"

/// An API key. Never Serialize; Debug prints `ApiKey(***)`. Constructed only via `ApiKey::new(trimmed non-empty)`.
pub struct ApiKey(String);
impl ApiKey { pub fn new(raw: &str) -> Option<Self>; pub fn expose(&self) -> &str }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
pub enum Role { User, Assistant }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message { pub role: Role, pub content: String }
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OutputSchema { pub name: String, pub schema: serde_json::Value }   // JSON Schema; objects use additionalProperties:false
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GenerateRequest { pub system: String, pub messages: Vec<Message>, pub schema: Option<OutputSchema>, pub max_tokens: u32 }

#[derive(Debug)]
pub enum AiError {
    NotConfigured,                 // "No AI provider is set up"
    MissingKey(ProviderKind),      // "Add your <label> API key in Settings"
    Unreachable(String),           // connection refused/timeout; Ollama → "Ollama is not running at localhost:11434"
    Auth(ProviderKind),            // 401/403 → "<label> rejected the API key" — NEVER includes the response body
    Http { status: u16, message: String },   // message = provider error message, key redacted, ≤ 300 chars
    Refused,                       // "The model declined to answer"
    Truncated,                     // "The response was cut off (output limit reached)"
    InvalidOutput(String),         // schema output that is not valid JSON / does not deserialize
    Cancelled,
    Io(std::io::Error),
}
// Display = the quoted messages; Serialize as its Display string (like LibraryError / ProcessingError).

/// The provider-neutral interface Summary and Ask use. `on_delta` gets each text piece in order; the return value is the full
/// text (concatenation of all deltas). Checks `cancel` between stream events → Err(Cancelled) (caller keeps what it got via deltas).
pub trait TextProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;
    fn model(&self) -> &str;
    fn generate(&self, request: &GenerateRequest, on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<String, AiError>;
}

/// Structured output: requires `request.schema` (panics in debug / InvalidOutput otherwise). Streams like `generate`, then trims,
/// strips one surrounding ``` / ```json fence if present, parses JSON and deserializes into T; failure → InvalidOutput(serde msg).
pub fn generate_json<T: DeserializeOwned>(provider: &dyn TextProvider, request: &GenerateRequest,
    on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<T, AiError>;
```

### HTTP seam (so wire formats are unit-tested without a network)

```rust
pub enum Method { Get, Post }
pub struct HttpRequest { pub method: Method, pub url: String, pub headers: Vec<(String, String)>, pub body: Option<Value> }
pub struct HttpResponse { pub status: u16, pub body: Box<dyn BufRead + Send> }
pub trait HttpClient: Send + Sync {
    /// Any status is returned as a response (non-2xx is NOT an Err); Err only for transport failures → AiError::Unreachable(..).
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, AiError>;
}
/// ureq 2 implementation: connect timeout 5 s, read timeout 120 s; maps ureq::Error::Status(code, resp) to Ok(HttpResponse).
/// Transport error text must not contain headers (ureq's does not). Not unit tested.
pub struct UreqClient;
```

`HttpProvider { http: Arc<dyn HttpClient>, kind, model: String, key: Option<ApiKey>, base_url: String }` implements `TextProvider`.
Constructors: `HttpProvider::ollama(http, model)` (base `http://localhost:11434`), `::openai(http, model, key)`
(base `https://api.openai.com`), `::anthropic(http, model, key)` (base `https://api.anthropic.com`). `base_url` is a pub field only so
tests can assert URLs; no user setting.

Common status handling for every call: 401/403 → `Auth(kind)` (body discarded); other non-2xx → `Http { status, message }` where
message = parsed provider error message (`error.message` for OpenAI/Anthropic, `error` for Ollama) or the raw body, with every
occurrence of the key replaced by `***`, truncated to 300 chars.

### Wire formats

**Ollama** (local, no key)
- Models: `GET {base}/api/tags` → `{"models":[{"name":"llama3.2:3b",...}]}` → names, sorted.
- Generate: `POST {base}/api/chat` body `{"model", "messages":[{"role":"system","content":system}, ...messages], "stream":true,
  "format": <schema> (only when schema is Some), "options":{"num_predict": max_tokens}}`. Response is NDJSON, one object per line:
  `{"message":{"role":"assistant","content":"Hel"},"done":false}` … `{"done":true,"done_reason":"stop"}`. `done_reason == "length"`
  → Truncated. A line `{"error":"..."}` → Http { status: 200, message }.

**OpenAI** (Chat Completions)
- Models: `GET {base}/v1/models` with `Authorization: Bearer <key>` → `data[].id`, keep only ids starting with `gpt-` or `o`
  followed by a digit, drop ids containing any of `audio`, `realtime`, `transcribe`, `tts`, `image`, `search`, `embedding`; sorted.
- Generate: `POST {base}/v1/chat/completions`, Bearer header, body `{"model", "messages":[{"role":"system",...}, ...],
  "stream":true, "max_completion_tokens": max_tokens, "response_format": {"type":"json_schema","json_schema":{"name","schema",
  "strict":true}}` (response_format only with schema). SSE: lines `data: {json}`; blank lines and `:` comments ignored;
  `data: [DONE]` ends. Text = `choices[0].delta.content` (string, may be absent/null). `choices[0].delta.refusal` non-empty →
  Refused. `choices[0].finish_reason == "length"` → Truncated. A `data:` object with `error` → Http { status: 200, message }.

**Anthropic** (Messages API, raw HTTP — there is no official Rust SDK)
- Headers on every call: `x-api-key: <key>`, `anthropic-version: 2023-06-01`, `content-type: application/json`.
- Models: `GET {base}/v1/models?limit=100` → `data[].id` (keep API order = newest first). Default model when none chosen:
  `claude-opus-5-5`.
- Generate: `POST {base}/v1/messages` body `{"model", "max_tokens", "system", "messages":[...], "stream":true,
  "output_config":{"format":{"type":"json_schema","schema": <schema>}}}` (output_config only with schema; do NOT send `thinking`,
  `temperature` or prefill). SSE: `event: <name>` + `data: {json}` pairs. Text = `content_block_delta` whose `delta.type ==
  "text_delta"` → `delta.text`; ignore every other delta type (thinking_delta, signature_delta, …) and every other event
  (message_start, content_block_start/stop, ping). `message_delta.delta.stop_reason`: `"refusal"` → Refused, `"max_tokens"` →
  Truncated. `message_stop` ends. `event: error` / data `{"type":"error","error":{"message"}}` → Http { status: 200, message }.

Model listing: `pub fn list_models(http: &dyn HttpClient, kind: ProviderKind, key: Option<&ApiKey>) -> Result<Vec<String>, AiError>`
(cloud kind without key → MissingKey). Ollama unreachable → `Unreachable("Ollama is not running at localhost:11434")`.

### Secrets and status

```rust
pub trait SecretStore: Send + Sync {
    fn get(&self, kind: ProviderKind) -> Result<Option<ApiKey>, String>;   // Ollama → always Ok(None)
    fn set(&self, kind: ProviderKind, key: &ApiKey) -> Result<(), String>;
    fn delete(&self, kind: ProviderKind) -> Result<(), String>;            // missing entry is Ok
}
/// In-memory store (used by tests and by non-Windows dev builds).
pub struct MemorySecretStore;  // Mutex<HashMap<ProviderKind, String>>

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AiStatus {
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,            // the chosen model of the selected provider
    pub configured: bool,                 // provider set && model set && (Ollama || key stored)
    pub has_openai_key: bool,
    pub has_anthropic_key: bool,
}
pub fn ai_status(settings: &AiSettings, secrets: &dyn SecretStore) -> AiStatus;   // a store error counts as "no key"

/// Builds the configured provider, or NotConfigured / MissingKey.
pub fn configured_provider(settings: &AiSettings, secrets: &dyn SecretStore, http: Arc<dyn HttpClient>)
    -> Result<HttpProvider, AiError>;
```

### Cloud disclosure

```rust
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Disclosure {
    pub provider: ProviderKind, pub model: String, pub endpoint: String,   // e.g. "https://api.openai.com/v1/chat/completions"
    pub system: String, pub messages: Vec<Message>,                         // exactly the text that will be sent
    pub characters: usize,                                                  // system + all message contents (chars)
}
pub fn disclosure(provider: &HttpProvider, request: &GenerateRequest) -> Disclosure;
```

## Settings (settings.rs)

Add `pub ai: AiSettings` to `Settings` (struct-level `#[serde(default)]` already there):

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSettings {
    pub provider: Option<ProviderKind>,
    pub ollama_model: Option<String>,
    pub openai_model: Option<String>,
    pub anthropic_model: Option<String>,
}
impl AiSettings { pub fn model_for(&self, kind: ProviderKind) -> Option<&str>; }  // Anthropic falls back to "claude-opus-5-5"
```
API keys are never a field of any settings type. Old settings files load with `ai` = default (test).

## Library: cloud consent (library.rs)

meeting.json gains an optional array (serde default empty, skipped when empty) — edited Value-based like `set_language`:
```json
"cloud_consent": [ { "provider": "openai", "consented_at": "2026-10-12T10:00:00+07:00" } ]
```
```rust
pub fn cloud_consent(&self, id: &str, provider: ProviderKind) -> Result<bool, LibraryError>;
/// Idempotent: a second call for the same provider changes nothing (keeps the first timestamp). Refuses Ollama (not cloud) with
/// LibraryError::Encode("Ollama runs on this PC and needs no consent").
pub fn record_cloud_consent(&self, id: &str, provider: ProviderKind, at: DateTime<FixedOffset>) -> Result<(), LibraryError>;
```
Consent is per Meeting and per cloud provider (switching provider asks again). Every send is still an explicit click (ticket 13/14
enforce that; this ticket only stores consent). Keep `MeetingFile` round-tripping the field (add it to the struct).

## Tauri (main.rs)

- State: `Secrets(Box<dyn SecretStore>)` — on Windows `KeyringStore` (keyring::Entry::new("Meeting Minutes", "openai-api-key" |
  "anthropic-api-key"); get_password → `keyring::Error::NoEntry` = Ok(None); delete_credential with NoEntry = Ok), elsewhere
  `MemorySecretStore`. `Http(Arc<dyn HttpClient>)` = `Arc::new(UreqClient)`.
- Commands (all return `Result<_, String>`; error strings are AiError Display — never contain a key):
  - `ai_status() -> AiStatus`
  - `list_ai_models(provider: ProviderKind) -> Vec<String>` (async command / run on a thread, it does network I/O)
  - `save_api_key(provider, key: String) -> AiStatus` (Ollama → error; `ApiKey::new` None → "Enter an API key")
  - `delete_api_key(provider) -> AiStatus`
  - `update_ai_settings(provider: Option<ProviderKind>, model: Option<String>) -> AiStatus` — sets `settings.ai.provider` and,
    when provider is Some and model is Some, that provider's model field. Loads + saves settings.json like `update_settings`
    (which must keep `ai` untouched — it already loads first; verify).
  - `test_ai_provider() -> String` — `configured_provider` + `generate` with system "You are a connection test." and user
    message "Reply with the single word: ready", max_tokens 1024 (reasoning models spend output tokens on thinking first), no schema; returns the trimmed text.
    Runs on a thread (async). No Meeting content is sent, so no disclosure; the UI states what is sent.
- No logging of keys anywhere (there is no log crate; don't add `println!/eprintln!` of requests).
- Add all new commands to `generate_handler!`.

## Frontend

- `src/ai.ts`: types (`ProviderKind`, `AiStatus`, `Disclosure`, `Message`) and invoke wrappers for the commands above;
  `PROVIDERS = [{kind:"ollama",label:"Ollama (on this PC)"},{kind:"openai",label:"OpenAI (your API key)"},{kind:"anthropic",label:"Anthropic (your API key)"}]`.
- SettingsView gets an "AI provider" section (below the transcription settings, its own controls; not part of the existing Save):
  - Intro line: "Summary and Ask use a text model. Ollama runs on this PC; OpenAI and Anthropic send text to their servers only
    when you click."
  - Select "Provider": None / the three PROVIDERS → `updateAiSettings(provider, null)`.
  - Ollama: button "Find Ollama models" → `listAiModels("ollama")`; success → select "Model" (choosing calls
    `updateAiSettings("ollama", model)`); error → `role="alert"` with the message + "Install Ollama from ollama.com, start it, and
    pull a model (for example `ollama pull llama3.2`)".
  - OpenAI / Anthropic: when no key: password input "API key" + "Save key" (input cleared right after the call; key never kept in
    state after saving). When key stored: "API key saved in Windows Credential Manager" + "Remove key". Button "Load models" →
    select "Model" as for Ollama (Anthropic preselects `claude-opus-5-5` if present and none chosen).
  - When `status.configured`: button "Test" → shows "Test reply: <text>" (`role="status"`) or the error (`role="alert"`). For a cloud
    provider the button text is "Send test message to <label>" (explicit, nothing from Meetings is sent).
- `src/AiSetupGuidance.tsx`: `({ onOpenSettings })` → "Summary and Ask need an AI provider. Set up Ollama on this PC, or add your own
  OpenAI or Anthropic API key." + button "Open AI settings". MeetingView takes a new prop `onOpenSettings` and renders the guidance
  (a section headed "Summary and Ask") only when `aiStatus().configured === false`; nothing when configured (tickets 13/14 fill it).
  Recording, import and transcription UI are unaffected. App wires `onOpenSettings` to the existing Settings view.
- `src/CloudDisclosure.tsx`: `({ disclosure, onSend, onCancel })`, a dialog (`role="dialog"`, labelled "Send to <label>?"): provider,
  model, endpoint, "<n> characters", the system text and each message in a scrollable `<pre>`, note "Your consent is remembered for
  this Meeting; each send still needs your click.", buttons "Send" / "Cancel". Not wired into a flow yet (13/14 use it).

## Tests

Core `crates/core/tests/ai.rs` with a `FakeHttp` (records requests, returns queued canned responses) and a `FakeProvider`
(implements TextProvider: scripted deltas or error):
- Ollama: tags → sorted names; unreachable → Unreachable message; chat request body (system first, `format` only with schema,
  `num_predict`); NDJSON deltas arrive in order and concatenate; `done_reason:"length"` → Truncated; `{"error":..}` line → Http.
- OpenAI: models filter; request URL/Bearer/response_format strict; SSE parse with blank + comment lines and `[DONE]`; refusal →
  Refused; finish_reason length → Truncated; 401 whose body echoes the key → Auth and Display contains no key; 500 body containing
  the key → message has `***` not the key.
- Anthropic: headers (x-api-key, anthropic-version) and output_config only with schema; SSE stream with thinking_delta ignored and
  text_delta collected; stop_reason refusal → Refused; max_tokens → Truncated; error event → Http.
- Cancel: flag set after first delta → Err(Cancelled), on_delta saw only the first piece, no further reads needed.
- `generate_json`: valid JSON → struct; fenced ```json → parsed; invalid → InvalidOutput; schema-less request → InvalidOutput.
- `ApiKey` Debug does not contain the key; `ApiKey::new("  ")` is None.
- `ai_status`: none / Ollama without model / Ollama with model = configured / OpenAI without key / with key = configured.
- `disclosure`: endpoint per provider, characters count, messages exactly as given.
- Settings: old file loads with default `ai`; a saved settings.json after `save_api_key`-equivalent flow contains no key (save
  settings with ai set + key in MemorySecretStore, assert file bytes lack the key).
- Library consent (tests/library.rs): false initially; record → true; second record keeps first timestamp; per provider; Ollama
  refused; unknown id refused; rename keeps it; rescan keeps it.
Frontend (vitest, mocking `@tauri-apps/api/core` like the existing tests): SettingsView AI section (provider select calls
update; Ollama models listed + selectable; Ollama error shows guidance; save key clears input and shows saved state; test button
shows reply); AiSetupGuidance shown in MeetingView when not configured and hidden when configured; CloudDisclosure renders the
text and calls onSend/onCancel.

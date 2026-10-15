# Ticket 14 design: Ask a Meeting with cited Answers

Ticket: .scratch/windows-mvp/issues/14-ask.md. Spec: ../spec.md ("Ask (cited Answers)", "AI providers"). Vocabulary: CONTEXT.md
(Ask, Answer, Insufficient evidence, Incomplete, Unverified, Citation). Uses `citation` from 13-core-design.md and ticket 12's
`TextProvider` / `generate_json` / `Disclosure` / cloud consent.

## Core: `ask` (new pub mod, crates/core/src/ask.rs)

### History (`asks.json` in the Meeting folder)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerState {
    Pending,               // being answered (only while the app runs; see `interrupted`)
    Answered,              // supported by ≥1 valid Citation, no invalid ones
    InsufficientEvidence,  // explicit: the Transcript has no support
    OffTopic,              // a polite nudge back to the Meeting
    Incomplete,            // stopped, or failed after some text arrived
    Unverified,            // contains invalid Citations, or claims support with none
    Failed,                // failed before any text arrived
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskEntry {
    pub id: String,                    // "a0001", "a0002", … (next = max existing + 1)
    pub question: String,
    pub asked_at: String,              // RFC 3339
    pub revision: u32,                 // Transcript Revision the Ask was scoped to
    pub provider: ProviderKind,
    pub model: String,
    pub state: AnswerState,
    pub answer: String,
    pub citations: Vec<Citation>,      // valid ones only
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_citations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskHistory { pub schema_version: u32 /* 1 */, pub asks: Vec<AskEntry> }

pub fn read_asks(meeting_dir: &Path) -> io::Result<AskHistory>;          // missing → default (schema_version 1, empty)
pub fn write_asks(meeting_dir: &Path, history: &AskHistory) -> io::Result<()>;  // asks.json.tmp + sync_all + rename

#[derive(Debug, PartialEq, Eq)]
pub enum AskError { EmptyQuestion, QuestionTooLong /* > 2000 chars */, Busy, NothingToRetry }
// Display: "Type a question about this Meeting" / "Keep the question under 2000 characters" /
// "An Ask is already running for this Meeting" / "Only the latest failed question can be retried"; Serialize as Display.

impl AskHistory {
    /// Appends a Pending entry (question trimmed). Busy when any entry is Pending.
    pub fn begin(&mut self, question: &str, revision: u32, provider: ProviderKind, model: &str, at: &str) -> Result<String, AskError>;
    /// The latest entry, when it is Failed or Incomplete-with-error. Only the latest can be retried.
    pub fn retryable(&self) -> Option<&AskEntry>;
    /// Resets the retryable entry IN PLACE to Pending (same id and question; new asked_at/revision/provider/model; answer, citations,
    /// error cleared) — retry never duplicates the question. Busy when another entry is Pending; NothingToRetry otherwise.
    pub fn begin_retry(&mut self, revision: u32, provider: ProviderKind, model: &str, at: &str) -> Result<String, AskError>;
    /// Stores the outcome on the entry with this id (no-op if missing).
    pub fn finish(&mut self, id: &str, outcome: AskOutcome);
    /// Pending entries left by a crash/quit become Failed with error "The app closed before the Answer finished" (or Incomplete if
    /// they have answer text). Returns true if anything changed.
    pub fn interrupted(&mut self) -> bool;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskOutcome { pub state: AnswerState, pub answer: String, pub citations: Vec<Citation>, pub invalid_citations: Vec<String>, pub error: Option<String> }
```

### Context building

```rust
pub struct AskOptions { pub input_tokens: usize, pub max_tokens: u32, pub history_turns: usize }
impl AskOptions {
    /// max_tokens 2048; input_tokens = context_tokens(kind) − max_tokens − 512, at least 1024; history_turns 4.
    pub fn for_provider(kind: ProviderKind) -> Self;
}
pub struct AskRequest { pub request: GenerateRequest, pub allowed: HashSet<String> /* Segment IDs sent */ }

/// Builds the request for `question` against `transcript` (the CURRENT Revision; nothing from other Meetings).
pub fn build_ask(transcript: &Transcript, history: &AskHistory, question: &str, options: &AskOptions) -> AskRequest;
/// Lexical retrieval used when the whole Transcript does not fit. Returns Segments in Timeline order.
pub fn retrieve<'a>(transcript: &'a Transcript, question: &str, budget_tokens: usize) -> Vec<&'a Segment>;
```
- **History context**: entries with `revision == transcript.revision` and state Answered or InsufficientEvidence, excluding the entry
  being answered (a retry) and any Pending; the last `history_turns` of them, oldest first, as `user: question` / `assistant: answer`
  message pairs. Incomplete, Unverified, Failed, OffTopic and other-Revision entries are never context. If history exceeds a quarter
  of `input_tokens`, drop the oldest pairs until it fits.
- **Evidence**: budget = `input_tokens` − system − history − question tokens. If `evidence_text(all)` fits → all Segments. Otherwise
  `retrieve`:
  - terms: lowercase the question, split on non-alphanumeric chars, keep terms with ≥ 2 chars, minus a small English + Vietnamese
    stopword list (the, and, what, was, did, who, when, is, are, of, to, in, a/an, about, we, they, it, this, that; và, là, của, có,
    không, gì, ai, khi, nào, được, cho, với, này, đó, những, các, một). Question terms and Segment words are both folded to base
    letters before matching with a private `fold_vietnamese(char) -> char` table (à…ỹ, đ → a…y, d); no new crate.
  - score each Segment: Σ over distinct matched terms of `ln(1 + N / df(term))` (IDF-like; N = Segment count).
  - take Segments in descending score (ties → earlier first); for each, include it and its immediate neighbours (index ±1), skipping
    already included, until the next addition would exceed budget.
  - no Segment matches any term (e.g. "what was this meeting about?") → fall back to Segments spread evenly across the Timeline:
    take every k-th Segment (k chosen so the evidence fits), so the model sees the whole meeting at low density.
  - result sorted by `start_ms`.
- **Messages**: `[history pairs…, user: "Transcript segments:\n<evidence>\n\nQuestion: <question>"]`. `allowed` = IDs of the
  evidence Segments.
- **System prompt** must say: answer questions about one meeting only from the transcript segments given (`[segment-id] Speaker:
  text`); put the ids of the supporting segments in `citations`, exactly as written; never write times or ids in the answer text;
  if the segments do not contain the answer set `insufficient_evidence` true, `citations` [], and say briefly that the transcript
  does not cover it; if the input is not a question or request about this meeting (greeting, general knowledge, coding, …) set
  `off_topic` true, `citations` [], and politely suggest asking something about the meeting; answer in the language of the question;
  return only the JSON object.
- **Schema** (name `"meeting_answer"`), properties in this order (the answer streams first): `answer: string`, `citations: string[]`,
  `insufficient_evidence: boolean`, `off_topic: boolean`; all required, `additionalProperties: false`.

### Running an Ask

```rust
/// Streams `request` and returns the outcome. `on_text` receives the answer text so far (decoded from the partial JSON) each time it
/// grows. Never returns Err: every failure is an outcome.
pub fn run_ask(provider: &dyn TextProvider, ask: &AskRequest, transcript: &Transcript,
    on_text: &mut dyn FnMut(&str), cancel: &AtomicBool) -> AskOutcome;
/// Decodes the (possibly unfinished) JSON string value of the top-level "answer" key from a prefix of the model output: handles
/// whitespace, a leading ```json fence, escapes (\" \\ \/ \n \t \r \b \f \uXXXX incl. surrogate pairs); an unfinished escape at the
/// end is ignored. No "answer" key yet → "".
pub fn partial_answer(output_prefix: &str) -> String;
```
Outcome rules (validation in app code via `check_citations(transcript, Some(&ask.allowed), raw)`):
- `off_topic` → OffTopic, citations [] (any given are ignored).
- `insufficient_evidence` → InsufficientEvidence, citations [].
- otherwise invalid IDs present → Unverified (valid ones kept, invalid listed); no valid IDs at all → Unverified with
  `invalid_citations` possibly empty (unsupported claim); else Answered.
- `Err(Cancelled)` → Incomplete, answer = `partial_answer(received)`, no error (stopped by the user).
- any other `Err` → answer = partial; partial non-empty → Incomplete, else Failed; `error = Some(err.to_string())`.

## Tauri (main.rs)

`AiJobsInner.asks: HashMap<String /* meeting */, ActiveAsk { ask_id: String, partial: String, cancel: Arc<AtomicBool> }>`.
```rust
#[derive(Serialize)] struct AskView { asks: Vec<AskEntry>, active: Option<ActiveAskView { ask_id, partial }>, retryable_id: Option<String> }
```
- `meeting_asks(meeting_id) -> AskView`: `read_asks`; if no active Ask for this Meeting and `interrupted()` changed something →
  `write_asks`. (So a crash mid-Answer leaves a retryable Failed entry after restart.)
- `ask_send_plan(meeting_id, question) -> SendPlan` (type from ticket 13): cloud + not consented → `disclosure` =
  `ai::disclosure(&provider, &build_ask(...).request)` (exactly what will be sent).
- `ask_meeting(meeting_id, question, consent: bool) -> AskView`:
  - `configured_provider` (error returned, nothing stored); current Transcript required ("Transcribe this Meeting first");
    one active Ask per Meeting (`AskError::Busy`); refuse while the Meeting is busy (processing/recording) — reuse `meeting_busy`.
  - cloud: not consented → require `consent` and record it (as ticket 13), else "Review what will be sent first".
  - `begin` → `write_asks` → register ActiveAsk → thread: `build_ask` (history excludes the new entry since it is Pending) →
    `run_ask` with `on_text` storing `partial` and emitting `ask-delta` `{ meeting_id, ask_id, text }` → `finish` → `write_asks`
    (re-read the file first and apply `finish` to it, so nothing else is lost) → remove ActiveAsk → emit `asks-changed`
    `{ meeting_id }`.
- `retry_ask(meeting_id, consent: bool) -> AskView`: as above with `begin_retry` (NothingToRetry when the latest entry is not
  retryable).
- `stop_ask(meeting_id)`: sets the cancel flag.
- `meeting_busy` also refuses while an Ask is active ("An Ask is running for this Meeting"), so rename/delete/re-transcribe wait.
- Register commands; keep Windows-gnu clippy clean.

## Frontend

- `src/ask.ts`: `AnswerState`, `AskEntry`, `AskView` types; wrappers `meetingAsks`, `askSendPlan`, `askMeeting(meetingId, question,
  consent)`, `retryAsk(meetingId, consent)`, `stopAsk`, `onAskDelta`, `onAsksChanged`; `answerLabel(state)` → "Insufficient evidence",
  "Incomplete", "Unverified", "Failed", "" for answered/off_topic/pending.
- `src/AskPanel.tsx`: `({ meetingId, transcript, player, aiStatus })`, `<section aria-label="Ask">` with `<h2>Ask</h2>`:
  - History `<ol aria-label="Asks">` oldest first: question; label (if any) in a `<strong>`; answer text; CitationLinks; Unverified
    adds "Some Citations could not be checked against this Meeting."; an entry from an older Revision shows "Asked about transcript
    revision N"; Failed/Incomplete-with-error shows the error (`role="alert"` only for the latest). "Retry" button only on
    `retryable_id`.
  - Active: the streaming `partial` text (`aria-live="polite"`) + "Stop" → `stopAsk`.
  - Form: `<textarea aria-label="Question">` + submit "Ask" (cloud: "Ask <Label>"); disabled while active, when empty, or without a
    Transcript ("Transcribe the Meeting to ask about it."). Enter submits, Shift+Enter newline. Submit flow = ticket 13's
    send-plan/CloudDisclosure flow, then `askMeeting`; the input clears once the Ask has started.
  - Listens to `ask-delta` (for this meeting) and `asks-changed` (reload `meetingAsks`).
- MeetingView renders `AskPanel` after `SummaryPanel` when configured; history is shown read-only when not configured.

## Tests

Core `crates/core/tests/ask.rs` (FakeProvider with scripted delta pieces and optional final error / cancel-after-n):
- citation validation in Answers: unknown ID, an ID existing only in another Meeting's Transcript, an ID only in another Revision,
  and an existing ID that was not in the retrieved evidence → Unverified with `invalid_citations`; all valid → Answered with ms
  ranges from the Transcript; `insufficient_evidence` → InsufficientEvidence, no citations; off_topic → OffTopic; no citations and
  not insufficient → Unverified.
- context building: small Transcript → every Segment and no others; long Transcript with tiny budget → evidence includes the
  matching Segment and its neighbours, excludes far unmatched ones, stays within budget, sorted by time; Vietnamese without accents
  ("ngan sach") matches "ngân sách"; no-match fallback spreads across the Timeline; history includes only same-Revision
  Answered/InsufficientEvidence entries (Incomplete, Unverified, Failed, OffTopic, other-Revision excluded), at most
  `history_turns`.
- state transitions: begin → Pending; second begin → Busy; finish; Stop (cancel after first delta) → Incomplete with partial text
  and later excluded from context; error before text → Failed + retryable; error after text → Incomplete + error + retryable;
  `begin_retry` keeps the id and does not add an entry; retry of a non-latest failed → NothingToRetry; `interrupted` turns Pending
  into Failed; write/read round-trip (history survives restart).
- `partial_answer`: prefixes cut mid-key, mid-string, mid-escape, `é`, surrogate pair, fenced output.
Frontend (vitest): AskPanel renders history with labels and CitationLinks (click → `playSpan`); streaming delta text shows; Stop
calls `stop_ask`; Retry only on the retryable entry; cloud first Ask shows the disclosure and Send calls `ask_meeting` with
consent true; Ollama asks without disclosure.

Manual (laptop): ask something discussed → Answer with Citations that play; ask something never mentioned → Insufficient evidence;
Stop mid-answer → Incomplete; restart the app → history is there; say "hello" → nudge.

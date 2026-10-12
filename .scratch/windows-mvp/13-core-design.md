# Ticket 13 design: Summary and Action items with Citations

Ticket: .scratch/windows-mvp/issues/13-summary-and-action-items.md. Spec: ../spec.md ("Processing", "AI providers"), ADR-0002
(local-first; cloud only on an explicit click). Vocabulary: CONTEXT.md (Summary, Action item, Citation, Revision). Built on ticket 12
(12-core-design.md: `TextProvider`, `generate_json`, `Disclosure`, cloud consent). Ticket 14 (14-core-design.md) shares the
`citation` module defined here.

## Core: provider context sizes (ai)

- `ai/http.rs`: Ollama chat body `options` gains `"num_ctx": OLLAMA_CONTEXT_TOKENS` (`pub const OLLAMA_CONTEXT_TOKENS: u32 = 8192;`,
  re-exported from `ai`). Ollama otherwise uses a small default context and silently drops the start of long prompts. Update the
  existing Ollama body test.
- `ai/mod.rs`: `pub fn context_tokens(kind: ProviderKind) -> usize` → Ollama `OLLAMA_CONTEXT_TOKENS`, OpenAI 128_000,
  Anthropic 200_000.

## Core: `citation` (new pub mod, crates/core/src/citation.rs)

```rust
/// A Citation as stored and shown: the Segment ID plus its ms range copied by app code from the Transcript (never from the model).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation { pub segment_id: String, pub start_ms: u64, pub end_ms: u64 }

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CitationCheck { pub valid: Vec<Citation>, pub invalid: Vec<String> }

/// Validates model-written Segment IDs against `transcript` (the Meeting's Revision that was sent). An ID is valid only when it names a
/// Segment of this Transcript AND, when `allowed` is Some, is one of the IDs actually sent in this request. Normalises each raw ID by
/// trimming whitespace and one pair of surrounding `[` `]`; duplicates collapse (first occurrence order kept); empty strings are
/// ignored (neither valid nor invalid). Unknown IDs (including IDs that exist only in another Meeting or another Revision) → invalid.
pub fn check_citations(transcript: &Transcript, allowed: Option<&HashSet<String>>, raw: &[String]) -> CitationCheck;

/// One line per Segment as sent to a model: `[s00012] Me: text`. No times: model-written timestamps are never Citations.
pub fn evidence_line(segment: &Segment) -> String;
/// Joined with '\n'.
pub fn evidence_text<'a>(segments: impl IntoIterator<Item = &'a Segment>) -> String;

/// Conservative token estimate for budgeting (Vietnamese tokenises densely): chars / 3, rounded up.
pub fn estimate_tokens(text: &str) -> usize;
```

## Core: `summary` (new pub mod, crates/core/src/summary.rs)

### Stored format (`summary.json`, pretty JSON + trailing newline) and `summary.md`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryItem { pub text: String, pub citations: Vec<Citation> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionItem { pub text: String, pub owner: Option<String>, pub due: Option<String>, pub citations: Vec<Citation> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub schema_version: u32,          // 1
    pub revision: u32,                // the Transcript Revision it was made from
    pub created_at: String,           // RFC 3339 local
    pub provider: ProviderKind,
    pub model: String,
    pub overview: Vec<SummaryItem>,
    pub key_points: Vec<SummaryItem>,
    pub decisions: Vec<SummaryItem>,
    pub action_items: Vec<ActionItem>,
    pub dropped_items: u32,           // items removed because no Citation was valid
}
```
- `pub fn read_summary(meeting_dir: &Path) -> io::Result<Option<Summary>>` — missing file → Ok(None); a parse error → `io::Error`
  (InvalidData) with the serde message.
- `pub fn write_summary(meeting_dir: &Path, title: &str, summary: &Summary) -> io::Result<()>` — writes `summary.json` and then
  `summary.md`, each via `<name>.tmp` + `sync_all` + rename (like `write_transcript_markdown`). Replacing an older Summary is the
  intended regenerate behaviour (same as `transcript.md`).
- `pub fn render_summary_markdown(title: &str, summary: &Summary) -> String`:
  ```
  # <title> — Summary

  From transcript revision <r> · <Provider label> <model>

  ## Overview
  - <text> [00:01:05, 00:03:10]
  ## Key points
  ...
  ## Decisions
  ...
  ## Action items
  - <text> — owner: <owner> · due: <due> [00:12:00]
  ```
  Times `[HH:MM:SS]` from `start_ms` (same formatting as `transcript::render_markdown`). An empty section prints `_None_`. Owner/due
  parts only when present.

### Model output (wire schema, private)

```rust
#[derive(Deserialize)] struct RawItem { text: String, citations: Vec<String> }
#[derive(Deserialize)] struct RawAction { task: String, owner: String, due: String, citations: Vec<String> }  // "" = not stated
#[derive(Deserialize)] struct RawSummary { overview: Vec<RawItem>, key_points: Vec<RawItem>, decisions: Vec<RawItem>, action_items: Vec<RawAction> }
```
JSON schema (name `"meeting_summary"`): every object `additionalProperties: false` with all properties `required`, strings
`{"type":"string"}`, citations `{"type":"array","items":{"type":"string"}}`. No nullable types and no `minItems`/`maxItems` (keeps
OpenAI strict, Anthropic and Ollama grammar support identical). Owner/due `""` (after trim) → `None`.

### Prompts

System (exact text is an implementation detail but must say all of this):
- You summarise one meeting from its transcript; the transcript is given as lines `[segment-id] Speaker: text`, speaker is Me, Remote
  or Unknown.
- Write in <English|Vietnamese> (from `transcript.language`).
- overview: 1–3 short sentences on what the meeting was about. key_points: the main points discussed. decisions: only decisions
  actually made. action_items: tasks someone committed to or was asked to do; owner = who is responsible as stated, else ""; due = the
  deadline exactly as stated, else "".
- Every item lists in `citations` the segment ids that support it, exactly as written (e.g. `s00012`). Never invent ids. Never write
  times or ids inside the text. Leave a section empty when the transcript has nothing for it. Return only the JSON object.
User message: `"Transcript:\n" + evidence_text(segments)`.

Reduce system (for map-reduce, below): the same rules, plus: "You are given partial summaries of consecutive parts of one meeting, in
order, as JSON. Merge them into one summary of the whole meeting: combine duplicates, keep each merged item's citations (union of
the ones it came from), and use only citation ids that appear in the input." User message: the partial summaries serialised as JSON
(`{"parts":[RawSummary-shaped objects with string citations]}`).

### Algorithm

```rust
pub struct SummaryOptions { pub input_tokens: usize, pub max_tokens: u32 }
impl SummaryOptions {
    /// max_tokens 4096; input_tokens = context_tokens(kind) − max_tokens − 512 (prompt overhead margin), at least 1024.
    pub fn for_provider(kind: ProviderKind) -> Self;
}

/// The map-stage requests that would be sent (one when the whole Transcript fits). Used for the cloud disclosure and by `summarize`.
pub fn summary_requests(transcript: &Transcript, options: &SummaryOptions) -> Vec<GenerateRequest>;

/// on_step(done, total) after each model call; total = number of map requests + reduce calls planned so far (may grow).
pub fn summarize(provider: &dyn TextProvider, transcript: &Transcript, options: &SummaryOptions,
    on_step: &mut dyn FnMut(usize, usize), cancel: &AtomicBool) -> Result<Summary, AiError>;
```
1. No Segments → `Err(AiError::InvalidOutput("The Transcript has no Segments"))` (the UI never offers it; defensive).
2. **Chunking** (never silent truncation): if `estimate_tokens(system) + estimate_tokens(evidence_text(all)) <= input_tokens` → one
   request. Otherwise split the Segments in order into consecutive chunks, greedily, each chunk's system + evidence ≤ input_tokens.
   A single Segment longer than the budget gets its own chunk (it is sent whole; Segments are short in practice).
3. **Map**: for each chunk `generate_json::<RawSummary>`, validate every item with `check_citations(transcript, Some(chunk ids))`:
   drop invalid IDs; drop items whose text is empty or that have no valid ID (count them in `dropped_items` only for the final
   pass, see 5). Cancel between calls → `Err(Cancelled)`.
4. **Reduce** (only with >1 chunk): serialise the validated partials (with string IDs). Group consecutive partials while the group's
   reduce prompt fits `input_tokens`; reduce each group with one `generate_json::<RawSummary>` call (validated against the union of
   the group's input IDs); repeat with the results until one remains. If a level cannot shrink (each group has one partial), stop
   and concatenate the remaining partials section by section (documented fallback; no truncation).
5. **Finalise**: validate again against the whole Transcript (`allowed = None` after map/reduce validation already restricted it),
   build `Citation`s with ms ranges from the Transcript, set `dropped_items` = items dropped in the final step + in the map step for
   single-chunk Summaries (for multi-chunk, the final step's count plus the map-step count). Keep it simple: one counter incremented
   every time any item is dropped anywhere.
6. `revision = transcript.revision`, `provider = provider.kind()`, `model = provider.model()`, `created_at = chrono::Local::now()`.
Streaming deltas are ignored for the Summary (no partial UI); `generate_json` is called with a no-op `on_delta`.

## Tauri (main.rs)

State `AiJobs(Mutex<AiJobsInner>)`, `AiJobsInner { summaries: HashMap<String, SummaryJob>, asks: … (ticket 14) }`:
```rust
#[derive(Clone, Serialize)] struct SummaryJob { meeting_id: String, state: SummaryJobState /* "running" | "failed" */, done: usize, total: usize, error: Option<String>, #[serde(skip)] cancel: Arc<AtomicBool> }
#[derive(Clone, Serialize)] struct SummaryStatus { jobs: Vec<SummaryJob> }
#[derive(Clone, Serialize)] struct SendPlan { provider: ProviderKind, cloud: bool, consented: bool, disclosure: Option<Disclosure> }
```
- `meeting_summary(meeting_id) -> Option<Summary>` (`summary::read_summary(library.meeting_dir(id))` after `library.meeting(id)?`).
- `summary_status() -> SummaryStatus`; every change emits `summary-status` with the full status.
- `summary_send_plan(meeting_id) -> SendPlan` — `configured_provider` (error → its Display, e.g. "No AI provider is set up"); for a
  cloud provider: `consented = library.cloud_consent(id, kind)`, and when not consented `disclosure = Some(Disclosure)` built from the
  map requests: `system` = the map system prompt, `messages` = one user message per map request, `characters` = sum (use
  `ai::disclosure` on the first request for provider/model/endpoint and recompute the rest). Ollama → `cloud: false`, no disclosure.
- `generate_summary(meeting_id, consent: bool) -> SummaryStatus`:
  - refuse: no Transcript ("Transcribe this Meeting first"); a Summary job already running for it; the Meeting is busy
    (`meeting_busy`: recording/saving/processing).
  - cloud provider: if not consented → require `consent == true`, then `library.record_cloud_consent(id, kind, now)`; otherwise
    error "Review what will be sent first". This command is only ever called from a click (no automatic cloud path exists).
  - spawn `start_summary(app, id, provider)`: thread → read current Transcript → `summarize` with on_step updating the job → on
    success `write_summary(meeting_dir, title, &summary)`, remove the job, emit `summary-status` and `library-changed`; on
    `Cancelled` remove the job; on other error set state failed + error.
- `cancel_summary(meeting_id)`, `dismiss_summary(meeting_id)` (removes a failed job).
- **Automatic local run**: in the processing worker, after a successful `transcribe_meeting` (and the ticket-15 index refresh), if
  `Settings.ai.provider == Some(Ollama)` and `configured_provider` succeeds → `start_summary`. Never for cloud providers. A failure
  shows as a failed Summary job; it does not mark the transcription failed.
- `meeting_busy` also refuses while a Summary job is running for the Meeting ("A Summary is being made for this Meeting") — so rename,
  delete, language change and re-transcribe wait for it. `transcribe_meeting` enqueue: refuse likewise.
- Register the commands; keep `main.rs` passing the Windows-gnu clippy check.

## Frontend

- `src/summary.ts`: types (`Citation`, `SummaryItem`, `ActionItem`, `Summary`, `SummaryJob`, `SummaryStatus`, `SendPlan`) and
  wrappers `meetingSummary`, `summaryStatus`, `summarySendPlan`, `generateSummary(meetingId, consent)`, `cancelSummary`,
  `dismissSummary`, `onSummaryStatus(handler)`.
- `src/CitationLink.tsx`: `({ citation, player })` → `<button aria-label="Play from mm:ss">[mm:ss]</button>` (formatElapsed of
  `start_ms`), disabled without a player, click → `player.playSpan(start_ms, end_ms)`. Shared with Ask.
- `src/SummaryPanel.tsx`: `({ meetingId, transcript, player, aiStatus })`, a `<section aria-label="Summary">` with `<h2>Summary</h2>`:
  - No Transcript → "Transcribe the Meeting to get a Summary."
  - Running job → "Summarising… (done/total)" + "Stop" → cancelSummary. Failed job → `role="alert"` error + "Retry" + "Dismiss".
  - Summary present: headings Overview, Key points, Decisions, Action items (each `<h3>`), items as `<li>` with text and a
    CitationLink per Citation; action items show "Owner: X" / "Due: Y" when present; empty section → "None". Footer "From transcript
    revision N · <Provider> <model>". If `summary.revision !== transcript.revision` → note "This Summary is from an earlier Transcript
    revision." Button "Regenerate Summary".
  - No Summary and no job → button "Generate Summary" (cloud provider: "Generate Summary with <Label>").
  - Click flow (shared helper `useCloudSend` or inline): `summarySendPlan` → if `plan.disclosure` show `CloudDisclosure`; Send →
    `generateSummary(id, true)`; Cancel closes; no disclosure → `generateSummary(id, false)`.
  - Reloads the Summary on `library-changed` and when a job for this Meeting disappears.
- MeetingView: the existing "Summary and Ask" area renders `AiSetupGuidance` when not configured (unchanged); when configured it
  renders `SummaryPanel` (and ticket 14's `AskPanel`). An existing `summary.json` is still shown when the provider is not configured
  (read-only, citations playable), with the guidance below it.

## Tests

Core `crates/core/tests/summary.rs` (FakeProvider returning queued JSON strings, recording each request):
- `check_citations`: valid; unknown ID; ID only in another Meeting's Transcript (other transcript has s00099, this one doesn't) →
  invalid; ID of an older Revision not present in the current one → invalid; `allowed` excludes an existing ID → invalid; "[s00001]"
  and " s00001 " normalise; duplicates collapse.
- `summarize` single chunk: invented IDs dropped; an item whose only ID is invented is dropped and counted; ms ranges come from the
  Transcript; owner/due "" → None; language line in the system prompt for Vietnamese.
- long Transcript with a tiny `input_tokens`: more than one map request, every Segment's text appears in exactly one map request
  (nothing truncated), then reduce calls happen and the final Summary keeps citations from late chunks; a map-stage ID outside its
  chunk is dropped.
- cancel between map calls → Err(Cancelled) and no further requests.
- `write_summary` + `read_summary` round-trip; `summary.md` content (sections, `_None_`, times from ms, owner/due); missing file → None.
- Ollama body includes `num_ctx` (tests/ai.rs).
Frontend (vitest): SummaryPanel renders sections and CitationLinks that call `playSpan(start, end)`; stale note; Generate for Ollama
calls generate without disclosure; for a cloud provider shows CloudDisclosure, Send calls `generate_summary` with consent true,
Cancel calls nothing; running job shows progress + Stop; failed shows Retry.

Manual (laptop): with Ollama configured, Record or Import → after transcription the Summary appears by itself; each Citation plays its
span; with OpenAI/Anthropic nothing is sent until "Generate Summary with …" → disclosure → Send.

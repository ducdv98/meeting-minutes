use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use chrono::DateTime;
use meeting_minutes_core::{
    ai::{AiError, GenerateRequest, ProviderKind, TextProvider},
    ask::{AnswerState, AskOptions},
    library_ask::{
        plan_library_ask, read_library_asks, run_library_ask, write_library_asks, DateRange,
        LibraryAskError, LibraryAskHistory, LibraryAskPlan,
    },
    library_retrieval::{EvidenceSegment, MeetingEvidence},
    Library,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tempfile::TempDir;

struct FakeProvider {
    replies: Mutex<Vec<Result<String, AiError>>>,
    requests: Mutex<Vec<GenerateRequest>>,
    cancel_during_answer: bool,
}
impl FakeProvider {
    fn new(replies: Vec<Result<String, AiError>>) -> Self {
        Self {
            replies: Mutex::new(replies),
            requests: Mutex::new(Vec::new()),
            cancel_during_answer: false,
        }
    }
    fn answer(value: Value) -> Self {
        Self::new(vec![Ok(value.to_string())])
    }
}
impl TextProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }
    fn model(&self) -> &str {
        "fake-model"
    }
    fn generate(
        &self,
        request: &GenerateRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> Result<String, AiError> {
        self.requests.lock().unwrap().push(request.clone());
        let reply = self.replies.lock().unwrap().remove(0)?;
        if self.cancel_during_answer {
            let half = reply.len() / 2;
            on_delta(&reply[..half]);
            cancel.store(true, Ordering::SeqCst);
            return Err(AiError::Cancelled);
        }
        on_delta(&reply);
        Ok(reply)
    }
}

fn response(answer: &str, citations: &[&str], insufficient: bool) -> Value {
    json!({"answer": answer, "citations": citations, "insufficient_evidence": insufficient, "off_topic": false})
}
fn meeting(id: &str, text: &str) -> MeetingEvidence {
    MeetingEvidence {
        meeting_id: id.into(),
        title: format!("Title {id}"),
        started_at: "2026-10-20T09:00:00Z".into(),
        language: None,
        segments: vec![EvidenceSegment {
            segment_id: "s00001".into(),
            revision: 2,
            start_ms: 1000,
            end_ms: 1900,
            participant: "Anna".into(),
            text: text.into(),
        }],
    }
}
fn plan(range: DateRange) -> LibraryAskPlan {
    LibraryAskPlan {
        question: "What happened?".into(),
        range,
        evidence: vec![
            meeting("one", "Alpha approved"),
            meeting("two", "Beta approved"),
        ],
    }
}

#[test]
fn plan_validates_question_before_using_the_provider() {
    let temp = TempDir::new().unwrap();
    let library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let provider = FakeProvider::new(vec![]);
    let cancel = AtomicBool::new(false);
    assert!(matches!(
        plan_library_ask(&provider, &library, "  ", DateRange::All, 0, &cancel),
        Err(LibraryAskError::EmptyQuestion)
    ));
    assert!(matches!(
        plan_library_ask(
            &provider,
            &library,
            &"a".repeat(2001),
            DateRange::All,
            0,
            &cancel
        ),
        Err(LibraryAskError::QuestionTooLong)
    ));
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[test]
fn date_ranges_use_the_meeting_start_boundary() {
    let now = 1_800_000_000_000;
    assert_eq!(
        DateRange::Last7Days.since_ms(now),
        Some(now - 7 * 86_400_000)
    );
    assert_eq!(
        DateRange::Last30Days.since_ms(now),
        Some(now - 30 * 86_400_000)
    );
    assert_eq!(
        DateRange::Last90Days.since_ms(now),
        Some(now - 90 * 86_400_000)
    );
    assert_eq!(DateRange::All.since_ms(now), None);
    assert_eq!(
        serde_json::to_string(&DateRange::Last7Days).unwrap(),
        "\"last7_days\""
    );
}

#[test]
fn run_maps_cross_meeting_references_and_rejects_unknown_ones() {
    let provider = FakeProvider::answer(response(
        "Both approved",
        &["M1-s00001", "M2-s00001"],
        false,
    ));
    let mut shown = Vec::new();
    let result = run_library_ask(
        &provider,
        &plan(DateRange::All),
        &HashSet::new(),
        &mut |text| shown.push(text.to_owned()),
        &AtomicBool::new(false),
    );
    assert_eq!(result.state, AnswerState::Answered);
    assert_eq!(result.meetings_read, ["one", "two"]);
    assert_eq!(
        result
            .citations
            .iter()
            .map(|c| c.meeting_id.as_str())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert!(result
        .citations
        .iter()
        .all(|c| c.segment_id == "s00001" && c.revision == 2 && c.start_ms == 1000));
    assert_eq!(shown.last().unwrap(), "Both approved");

    for reference in ["M3-s00001", "M1-s99999"] {
        let invalid = FakeProvider::answer(response("Unsupported", &[reference], false));
        let result = run_library_ask(
            &invalid,
            &plan(DateRange::All),
            &HashSet::new(),
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(result.state, AnswerState::Unverified);
        assert_eq!(result.invalid_citations, [reference]);
    }
}

#[test]
fn excluded_meetings_are_not_sent_or_replaced_and_prior_answers_are_absent() {
    let provider = FakeProvider::answer(response("Approved", &["M1-s00001"], false));
    let result = run_library_ask(
        &provider,
        &plan(DateRange::All),
        &HashSet::from(["two".into()]),
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(result.meetings_read, ["one"]);
    let request = &provider.requests.lock().unwrap()[0];
    assert_eq!(request.messages.len(), 1);
    assert!(request.messages[0].content.contains("Title one"));
    assert!(!request.messages[0].content.contains("Title two"));
    assert!(!request.messages[0].content.contains("earlier Answer"));
    assert_eq!(result.state, AnswerState::Answered);
}

#[test]
fn insufficient_evidence_uses_the_range_text_and_empty_evidence_skips_the_provider() {
    for (range, expected) in [
        (DateRange::All, "Your Library doesn't cover this."),
        (
            DateRange::Last30Days,
            "Your Library doesn't cover this in the selected range.",
        ),
    ] {
        let provider = FakeProvider::answer(response("No", &[], true));
        let result = run_library_ask(
            &provider,
            &plan(range),
            &HashSet::new(),
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(result.state, AnswerState::InsufficientEvidence);
        assert_eq!(result.answer, expected);
        let empty_provider = FakeProvider::new(vec![]);
        let result = run_library_ask(
            &empty_provider,
            &LibraryAskPlan {
                evidence: vec![],
                ..plan(range)
            },
            &HashSet::new(),
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(result.state, AnswerState::InsufficientEvidence);
        assert_eq!(result.answer, expected);
        assert!(empty_provider.requests.lock().unwrap().is_empty());
    }
}

#[test]
fn cancelled_stream_is_incomplete() {
    let mut provider = FakeProvider::answer(response("Partial answer", &["M1-s00001"], false));
    provider.cancel_during_answer = true;
    let result = run_library_ask(
        &provider,
        &plan(DateRange::All),
        &HashSet::new(),
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(result.state, AnswerState::Incomplete);
    assert!(result.citations.is_empty());
}

#[test]
fn history_round_trips_and_survives_index_rebuild() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let mut history = LibraryAskHistory::default();
    let id = history
        .begin(
            "What happened?",
            DateRange::All,
            ProviderKind::Ollama,
            "fake-model",
            "now",
        )
        .unwrap();
    let provider = FakeProvider::answer(response("Approved", &["M1-s00001"], false));
    let outcome = run_library_ask(
        &provider,
        &plan(DateRange::All),
        &HashSet::new(),
        &mut |_| {},
        &AtomicBool::new(false),
    );
    history.finish(&id, outcome);
    write_library_asks(&root, &history).unwrap();
    let before = fs::read(root.join("library-asks.json")).unwrap();
    assert_eq!(read_library_asks(&root).unwrap(), history);
    let report = library.rescan().unwrap();
    assert!(report.malformed.is_empty());
    assert_eq!(fs::read(root.join("library-asks.json")).unwrap(), before);
    drop(library);
    let _rebuilt = Library::open(&root, temp.path().join("rebuilt.sqlite3")).unwrap();
    assert_eq!(fs::read(root.join("library-asks.json")).unwrap(), before);
}

#[test]
fn interrupted_pending_history_entry_becomes_incomplete() {
    let mut history = LibraryAskHistory::default();
    history
        .begin(
            "What happened?",
            DateRange::Last7Days,
            ProviderKind::Ollama,
            "fake-model",
            "now",
        )
        .unwrap();
    assert!(history.interrupted());
    assert_eq!(history.asks[0].state, AnswerState::Incomplete);
    assert!(!history.interrupted());
}

#[derive(Deserialize)]
struct Evaluation {
    questions: Vec<EvaluationQuestion>,
}
#[derive(Deserialize)]
struct EvaluationQuestion {
    id: String,
    tags: Vec<String>,
    keywords: Vec<String>,
    supporting: Vec<Support>,
}
#[derive(Deserialize)]
struct Support {
    meeting_id: String,
    segment_id: String,
}
fn copy_dir(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}
#[test]
fn plan_uses_expanded_terms_and_falls_back_to_question_terms() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library-ask");
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    copy_dir(&fixture.join("Library"), &root);
    let library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let now = DateTime::parse_from_rfc3339("2026-10-27T12:00:00Z")
        .unwrap()
        .timestamp_millis();
    let expanded = FakeProvider::new(vec![Ok(json!({"terms": ["SmartScreen"]}).to_string())]);
    let plan = plan_library_ask(
        &expanded,
        &library,
        "What does the support page explain?",
        DateRange::All,
        now,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(plan.evidence.iter().any(|meeting| meeting
        .segments
        .iter()
        .any(|segment| segment.text.contains("SmartScreen"))));
    assert!(expanded.requests.lock().unwrap()[0].messages[0]
        .content
        .contains("Meeting languages"));

    let fallback = FakeProvider::new(vec![Err(AiError::InvalidOutput("rewrite failed".into()))]);
    let plan = plan_library_ask(
        &fallback,
        &library,
        "SmartScreen",
        DateRange::All,
        now,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!plan.evidence.is_empty());
    let recent = FakeProvider::new(vec![Ok(json!({"terms": ["Northwind"]}).to_string())]);
    let plan = plan_library_ask(
        &recent,
        &library,
        "Northwind?",
        DateRange::Last7Days,
        now,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(plan.evidence.is_empty());
}

#[test]
fn retrieval_stops_before_loading_later_meetings_when_budget_is_full() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    for (date, text) in [
        ("2026-10-20T10:00:00Z", "alpha beta gamma"),
        ("2026-10-21T10:00:00Z", "alpha"),
    ] {
        let meeting = library
            .create_meeting("Planning", DateTime::parse_from_rfc3339(date).unwrap())
            .unwrap();
        let transcript = json!({
            "schema_version": 1, "revision": 1, "created_at": date, "model": "small",
            "language": "en", "language_detected": false, "duration_ms": 1000,
            "segments": [{"id": "s00001", "start_ms": 0, "end_ms": 900, "text": text, "speaker": "remote"}]
        });
        fs::write(
            library.meeting_dir(&meeting.id).join("transcript.r1.json"),
            serde_json::to_vec(&transcript).unwrap(),
        )
        .unwrap();
        library.refresh_transcript(&meeting.id).unwrap();
    }
    let terms = vec!["alpha".into(), "beta".into(), "gamma".into()];
    let all = library.library_evidence(&terms, None, 100).unwrap();
    assert_eq!(all.len(), 2);
    let first = &all[0];
    assert_eq!(first.segments[0].text, "alpha beta gamma");
    let line = format!(
        "[{}] {}: {}",
        first.segments[0].segment_id, first.segments[0].participant, first.segments[0].text
    );
    let budget = meeting_minutes_core::citation::estimate_tokens(&line);
    fs::write(
        library
            .meeting_dir(&all[1].meeting_id)
            .join("transcript.r1.json"),
        b"broken",
    )
    .unwrap();
    let limited = library.library_evidence(&terms, None, budget).unwrap();
    assert_eq!(limited.len(), 1);
    assert_eq!(limited[0].meeting_id, first.meeting_id);
}

#[test]
fn ollama_recall_baseline_covers_all_supporting_segments_for_at_least_80_percent() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library-ask");
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    copy_dir(&fixture.join("Library"), &root);
    let library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let evaluation: Evaluation =
        serde_json::from_slice(&fs::read(fixture.join("questions.json")).unwrap()).unwrap();
    let budget = AskOptions::for_provider(ProviderKind::Ollama).input_tokens;
    let mut passed = 0;
    let mut total = 0;
    for question in evaluation
        .questions
        .iter()
        .filter(|q| q.tags.iter().any(|tag| tag == "answerable"))
    {
        total += 1;
        let evidence = library
            .library_evidence(&question.keywords, None, budget)
            .unwrap();
        let found = question.supporting.iter().all(|support| {
            evidence.iter().any(|meeting| {
                meeting.meeting_id == support.meeting_id
                    && meeting
                        .segments
                        .iter()
                        .any(|segment| segment.segment_id == support.segment_id)
            })
        });
        println!("{}: {}", question.id, if found { "pass" } else { "miss" });
        passed += usize::from(found);
    }
    assert!(passed * 100 >= total * 80, "recall: {passed}/{total}");
}

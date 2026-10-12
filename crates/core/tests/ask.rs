use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use meeting_minutes_core::{
    ai::{AiError, GenerateRequest, ProviderKind, TextProvider},
    ask::{
        build_ask, partial_answer, read_asks, retrieve, run_ask, write_asks, AnswerState, AskError,
        AskHistory, AskOptions, AskOutcome,
    },
    transcript::{Language, Segment, Speaker, Transcript},
};
use serde_json::json;

fn transcript(count: usize) -> Transcript {
    Transcript {
        schema_version: 1,
        revision: 2,
        created_at: "2026-01-01T00:00:00Z".into(),
        model: "whisper".into(),
        language: Language::En,
        language_detected: false,
        duration_ms: 100_000,
        segments: (1..=count)
            .map(|i| Segment {
                id: format!("s{i:05}"),
                start_ms: i as u64 * 1000,
                end_ms: i as u64 * 1000 + 500,
                text: format!("Routine discussion section {i} about schedules."),
                speaker: Speaker::Remote,
            })
            .collect(),
    }
}

#[test]
fn retrieve_budgets_3000_segments_quickly() {
    let transcript = transcript(3000);
    let start = Instant::now();
    let selected = retrieve(&transcript, "schedules", 100_000);
    assert_eq!(selected.len(), 3000);
    assert!(start.elapsed() < Duration::from_secs(1));
}
fn opts(input_tokens: usize) -> AskOptions {
    AskOptions {
        input_tokens,
        max_tokens: 100,
        history_turns: 4,
    }
}
struct FakeProvider {
    parts: Vec<String>,
    error: Option<fn() -> AiError>,
    cancel_after_first: bool,
}
impl FakeProvider {
    fn answer(value: serde_json::Value) -> Self {
        Self {
            parts: vec![value.to_string()],
            error: None,
            cancel_after_first: false,
        }
    }
}
impl TextProvider for FakeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }
    fn model(&self) -> &str {
        "fake"
    }
    fn generate(
        &self,
        _: &GenerateRequest,
        on_delta: &mut dyn FnMut(&str),
        cancel: &AtomicBool,
    ) -> Result<String, AiError> {
        let mut output = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            output.push_str(part);
            on_delta(part);
            if self.cancel_after_first && i == 0 {
                cancel.store(true, Ordering::SeqCst);
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(AiError::Cancelled);
            }
        }
        if let Some(error) = self.error {
            Err(error())
        } else {
            Ok(output)
        }
    }
}
fn response(answer: &str, ids: &[&str], insufficient: bool, off_topic: bool) -> serde_json::Value {
    json!({"answer":answer,"citations":ids,"insufficient_evidence":insufficient,"off_topic":off_topic})
}
fn ask_with_all(transcript: &Transcript) -> meeting_minutes_core::ask::AskRequest {
    build_ask(
        transcript,
        &AskHistory::default(),
        "What was discussed?",
        &opts(5000),
    )
}
#[test]
fn answers_only_accept_citations_from_the_current_evidence() {
    let mut other_meeting = transcript(1);
    other_meeting.segments[0].id = "s00099".into();
    let transcript = transcript(2);
    let mut older_revision = transcript.clone();
    older_revision.revision = 1;
    older_revision.segments.push(Segment {
        id: "s00003".into(),
        ..older_revision.segments[0].clone()
    });
    assert!(other_meeting.segment("s00099").is_some());
    assert!(older_revision.segment("s00003").is_some());
    let ask = ask_with_all(&transcript);
    let mut seen = Vec::new();
    let good = run_ask(
        &FakeProvider::answer(response("Schedule", &["s00001"], false, false)),
        &ask,
        &transcript,
        &mut |s| seen.push(s.to_owned()),
        &AtomicBool::new(false),
    );
    assert_eq!(good.state, AnswerState::Answered);
    assert_eq!(good.citations[0].start_ms, 1000);
    assert_eq!(seen.last().unwrap(), "Schedule");
    for id in ["s00099", "s00003"] {
        // another Meeting or older Revision
        let result = run_ask(
            &FakeProvider::answer(response("Unsupported", &[id], false, false)),
            &ask,
            &transcript,
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(result.state, AnswerState::Unverified);
        assert_eq!(result.invalid_citations, [id]);
    }
    let mut limited = ask_with_all(&transcript);
    limited.allowed.remove("s00002");
    let result = run_ask(
        &FakeProvider::answer(response("Only outside evidence", &["s00002"], false, false)),
        &limited,
        &transcript,
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(result.invalid_citations, ["s00002"]);
    assert_eq!(result.state, AnswerState::Unverified);
    for (value, expected) in [
        (
            response("No support", &[], true, false),
            AnswerState::InsufficientEvidence,
        ),
        (
            response("Ask about this meeting", &["s00001"], false, true),
            AnswerState::OffTopic,
        ),
        (
            response("Claim", &[], false, false),
            AnswerState::Unverified,
        ),
    ] {
        let result = run_ask(
            &FakeProvider::answer(value),
            &ask,
            &transcript,
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(result.state, expected);
        assert!(result.citations.is_empty());
        if expected == AnswerState::InsufficientEvidence {
            assert_eq!(result.answer, "The transcript of this Meeting does not cover this.");
        } else if expected == AnswerState::OffTopic {
            assert_eq!(result.answer, "Ask about this meeting");
        }
    }
}
#[test]
fn retrieval_keeps_relevant_segments_neighbours_and_timeline_order() {
    let mut transcript = transcript(20);
    transcript.segments[10].text = "Ngân sách quý ba được duyệt".into();
    let selected = retrieve(&transcript, "ngan sach", 80);
    let ids: Vec<_> = selected.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"s00011"));
    assert!(ids.contains(&"s00010"));
    assert!(ids.contains(&"s00012"));
    assert!(!ids.contains(&"s00001"));
    assert!(selected
        .windows(2)
        .all(|pair| pair[0].start_ms < pair[1].start_ms));
    let evidence = meeting_minutes_core::citation::evidence_text(selected);
    assert!(meeting_minutes_core::citation::estimate_tokens(&evidence) <= 80);
    let request = build_ask(&transcript, &AskHistory::default(), "ngan sach", &opts(340));
    assert!(request.allowed.contains("s00011"));
    assert!(!request.allowed.contains("s00001"));
    assert!(request
        .request
        .messages
        .last()
        .unwrap()
        .content
        .contains("Ngân sách"));
    let fallback = retrieve(&transcript, "what was this meeting about?", 100);
    assert!(fallback.len() > 1);
    assert!(fallback.last().unwrap().start_ms > fallback.first().unwrap().start_ms + 5000);
}
#[test]
fn small_transcript_and_history_use_only_supported_current_answers() {
    let transcript = transcript(2);
    let mut history = AskHistory::default();
    let states = [
        AnswerState::Answered,
        AnswerState::InsufficientEvidence,
        AnswerState::Incomplete,
        AnswerState::Unverified,
        AnswerState::Failed,
        AnswerState::OffTopic,
    ];
    for (i, state) in states.into_iter().enumerate() {
        let id = history
            .begin(
                &format!("question {i}"),
                2,
                ProviderKind::Ollama,
                "m",
                "now",
            )
            .unwrap();
        history.finish(
            &id,
            AskOutcome {
                state,
                answer: format!("answer {i}"),
                citations: vec![],
                invalid_citations: vec![],
                error: None,
            },
        );
    }
    let old = history
        .begin("old question", 1, ProviderKind::Ollama, "m", "now")
        .unwrap();
    history.finish(
        &old,
        AskOutcome {
            state: AnswerState::Answered,
            answer: "old answer".into(),
            citations: vec![],
            invalid_citations: vec![],
            error: None,
        },
    );
    let request = build_ask(&transcript, &history, "next?", &opts(5000));
    assert_eq!(request.allowed.len(), 2);
    let text = request
        .request
        .messages
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("question 0") && text.contains("answer 1"));
    for excluded in [
        "question 2",
        "question 3",
        "question 4",
        "question 5",
        "old question",
    ] {
        assert!(!text.contains(excluded));
    }
    assert!(text.contains("[s00001]") && text.contains("[s00002]"));
    let limited = build_ask(
        &transcript,
        &history,
        "next?",
        &AskOptions {
            history_turns: 1,
            ..opts(5000)
        },
    );
    assert_eq!(limited.request.messages.len(), 3);
    assert_eq!(limited.request.messages[0].content, "question 1");
}
#[test]
fn history_lifecycle_retry_crash_and_storage() {
    let dir = tempfile::tempdir().unwrap();
    let mut history = read_asks(dir.path()).unwrap();
    assert_eq!(history.schema_version, 1);
    assert_eq!(
        history.begin(" ", 2, ProviderKind::Ollama, "m", "now"),
        Err(AskError::EmptyQuestion)
    );
    assert_eq!(
        history.begin(&"x".repeat(2001), 2, ProviderKind::Ollama, "m", "now"),
        Err(AskError::QuestionTooLong)
    );
    let id = history
        .begin("  What?  ", 2, ProviderKind::Ollama, "m", "now")
        .unwrap();
    assert_eq!(id, "a0001");
    assert_eq!(history.asks[0].question, "What?");
    assert_eq!(
        history.begin("again", 2, ProviderKind::Ollama, "m", "now"),
        Err(AskError::Busy)
    );
    assert!(history.interrupted());
    assert_eq!(history.asks[0].state, AnswerState::Failed);
    assert!(history.retryable().is_some());
    assert_eq!(
        history
            .begin_retry(3, ProviderKind::Openai, "new", "later")
            .unwrap(),
        id
    );
    assert_eq!(history.asks.len(), 1);
    assert_eq!(history.asks[0].revision, 3);
    history.finish(
        &id,
        AskOutcome {
            state: AnswerState::Answered,
            answer: "Yes".into(),
            citations: vec![],
            invalid_citations: vec![],
            error: None,
        },
    );
    assert!(history.retryable().is_none());
    assert_eq!(
        history.begin_retry(3, ProviderKind::Ollama, "m", "later"),
        Err(AskError::NothingToRetry)
    );
    write_asks(dir.path(), &history).unwrap();
    assert_eq!(read_asks(dir.path()).unwrap(), history);
    let next = history
        .begin("Second", 3, ProviderKind::Ollama, "m", "now")
        .unwrap();
    assert_eq!(next, "a0002");
    history.finish(
        &next,
        AskOutcome {
            state: AnswerState::Failed,
            answer: String::new(),
            citations: vec![],
            invalid_citations: vec![],
            error: Some("network".into()),
        },
    );
    assert_eq!(history.retryable().unwrap().id, "a0002");
    let third = history
        .begin("Third", 3, ProviderKind::Ollama, "m", "now")
        .unwrap();
    history.finish(
        &third,
        AskOutcome {
            state: AnswerState::Answered,
            answer: "Done".into(),
            citations: vec![],
            invalid_citations: vec![],
            error: None,
        },
    );
    assert_eq!(
        history.begin_retry(3, ProviderKind::Ollama, "m", "later"),
        Err(AskError::NothingToRetry)
    );
    let fourth = history
        .begin("Fourth", 3, ProviderKind::Ollama, "m", "now")
        .unwrap();
    history.asks.last_mut().unwrap().answer = "partial".into();
    assert!(history.interrupted());
    assert_eq!(history.retryable().unwrap().id, fourth);
    assert_eq!(history.retryable().unwrap().state, AnswerState::Incomplete);
}
#[test]
fn stop_and_errors_preserve_partial_answer_and_retry_rules() {
    let transcript = transcript(1);
    let ask = ask_with_all(&transcript);
    let stopped = FakeProvider {
        parts: vec!["{\"answer\":\"Hello".into(), " world\"}".into()],
        error: None,
        cancel_after_first: true,
    };
    let outcome = run_ask(
        &stopped,
        &ask,
        &transcript,
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(outcome.state, AnswerState::Incomplete);
    assert_eq!(outcome.answer, "Hello");
    assert_eq!(outcome.error, None);
    for (parts, state, answer) in [
        (
            vec!["{\"answer\":\"partial".into()],
            AnswerState::Incomplete,
            "partial",
        ),
        (vec![], AnswerState::Failed, ""),
    ] {
        let provider = FakeProvider {
            parts,
            error: Some(|| AiError::Truncated),
            cancel_after_first: false,
        };
        let outcome = run_ask(
            &provider,
            &ask,
            &transcript,
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(outcome.state, state);
        assert_eq!(outcome.answer, answer);
        assert!(outcome.error.is_some());
    }
}
#[test]
fn partial_json_answer_decodes_prefixes_and_escapes() {
    assert_eq!(partial_answer("{\"ans"), "");
    assert_eq!(partial_answer(" { \"answer\" : \"Hi"), "Hi");
    assert_eq!(
        partial_answer(
            "```json\n{\"answer\":\"hé\\n\\\"ok\\\"\\/\\t\\r\\b\\f\\u00E9\\uD83D\\uDE00"
        ),
        "hé\n\"ok\"/\t\r\u{0008}\u{000c}é😀"
    );
    assert_eq!(partial_answer("{\"answer\":\"done\\u"), "done");
    assert_eq!(
        partial_answer("{\"other\":\"answer\",\"answer\":\"Yes\"}"),
        "Yes"
    );
}

use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

use meeting_minutes_core::{
    ai::{AiError, GenerateRequest, ProviderKind, TextProvider},
    citation::{check_citations, estimate_tokens, evidence_line, evidence_text},
    participant::Participants,
    summary::{
        read_summary, render_summary_markdown, summarize, summary_requests, write_summary,
        SummaryOptions,
    },
    transcript::{Language, Segment, Speaker, Transcript},
};
use serde_json::json;

fn transcript(count: usize, language: Language) -> Transcript {
    Transcript {
        schema_version: 1,
        revision: 3,
        created_at: "2026-01-01T00:00:00Z".into(),
        model: "whisper".into(),
        language,
        language_detected: false,
        duration_ms: 600_000,
        segments: (1..=count)
            .map(|i| Segment {
                id: format!("s{i:05}"),
                start_ms: i as u64 * 65_000,
                end_ms: i as u64 * 65_000 + 900,
                text: format!("Discussion point {i} about launch schedule and budget."),
                speaker: Speaker::Me,
            })
            .collect(),
    }
}

#[test]
fn summary_evidence_names_participants_and_identifies_the_user() {
    let mut transcript = transcript(2, Language::En);
    transcript.segments[1].speaker = Speaker::Remote;
    let participants = Participants::new([("remote".into(), "Anna".into())].into(), None, Some("Duc"), false);
    let request = &summary_requests(&transcript, &participants, &SummaryOptions { input_tokens: 4000, max_tokens: 512 })[0];
    assert!(request.messages[0].content.contains("[s00001] Duc: Discussion"));
    assert!(request.messages[0].content.contains("[s00002] Anna: Discussion"));
    assert!(request.system.contains("Duc is the user"));
}

#[test]
fn summary_tracks_resolved_participants_and_old_files_default_safely() {
    let transcript = transcript(1, Language::En);
    let defaults = Participants::new(Default::default(), None, None, false);
    let named = Participants::new([("me".into(), "Duc".into())].into(), None, None, false);
    let provider = FakeProvider::with(&[&raw(json!([]), json!([]))]);
    let summary = summarize(&provider, &transcript, &defaults, &SummaryOptions { input_tokens: 4000, max_tokens: 512 }, &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    assert!(!summary.participants_changed(&transcript, &defaults));
    assert!(summary.participants_changed(&transcript, &named));
    let mut old = serde_json::to_value(&summary).unwrap();
    old.as_object_mut().unwrap().remove("participants_fingerprint");
    let old: meeting_minutes_core::summary::Summary = serde_json::from_value(old).unwrap();
    assert!(!old.participants_changed(&transcript, &defaults));
    assert!(old.participants_changed(&transcript, &named));
}

#[test]
fn summary_requests_budget_3000_segments_quickly() {
    let transcript = transcript(3000, Language::En);
    let start = Instant::now();
    let requests = summary_requests(&transcript, &Participants::new(Default::default(), None, None, false), &SummaryOptions { input_tokens: 100_000, max_tokens: 512 });
    assert_eq!(requests.len(), 1);
    assert!(start.elapsed() < Duration::from_secs(1));
}
#[derive(Default)]
struct FakeProvider {
    outputs: Mutex<VecDeque<String>>,
    requests: Mutex<Vec<GenerateRequest>>,
}
impl FakeProvider {
    fn with(outputs: &[&str]) -> Self {
        Self {
            outputs: Mutex::new(outputs.iter().map(|s| s.to_string()).collect()),
            ..Self::default()
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
        request: &GenerateRequest,
        _: &mut dyn FnMut(&str),
        _: &AtomicBool,
    ) -> Result<String, AiError> {
        self.requests.lock().unwrap().push(request.clone());
        Ok(self
            .outputs
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted response"))
    }
}
fn raw(overview: serde_json::Value, action_items: serde_json::Value) -> String {
    json!({"overview":overview,"key_points":[],"decisions":[],"action_items":action_items})
        .to_string()
}
#[test]
fn citation_ids_are_checked_against_this_revision_and_sent_evidence() {
    let mut other_meeting = transcript(1, Language::En);
    let transcript = transcript(2, Language::En);
    other_meeting.segments[0].id = "s00099".into();
    let mut older_revision = transcript.clone();
    older_revision.revision = 2;
    older_revision.segments.push(Segment {
        id: "s00098".into(),
        ..older_revision.segments[0].clone()
    });
    let allowed = HashSet::from(["s00001".to_owned()]);
    let checked = check_citations(
        &transcript,
        Some(&allowed),
        &[
            " [s00001] ".into(),
            " s00001 ".into(),
            "s00002".into(),
            "s00099".into(),
            "".into(),
        ],
    );
    assert_eq!(checked.valid.len(), 1);
    assert_eq!(checked.valid[0].start_ms, 65_000);
    assert_eq!(checked.invalid, ["s00002", "s00099"]);
    assert_eq!(
        check_citations(&transcript, None, &["s00099".into()]).invalid,
        ["s00099"]
    ); // another Meeting or older Revision
    assert_eq!(
        check_citations(&other_meeting, None, &["s00099".into()])
            .valid
            .len(),
        1
    );
    assert_eq!(
        check_citations(&older_revision, None, &["s00098".into()])
            .valid
            .len(),
        1
    );
    assert_eq!(
        check_citations(&transcript, None, &["s00098".into()]).invalid,
        ["s00098"]
    );
    assert_eq!(
        evidence_line(&transcript.segments[0], &Participants::new(Default::default(), None, None, false), transcript.revision),
        "[s00001] Me: Discussion point 1 about launch schedule and budget."
    );
    assert_eq!(evidence_text(&transcript.segments, &Participants::new(Default::default(), None, None, false), transcript.revision).lines().count(), 2);
    assert_eq!(estimate_tokens("abcd"), 2);
}
#[test]
fn single_chunk_drops_unsupported_items_and_keeps_transcript_times() {
    let output = raw(
        json!([{"text":"Budget approved","citations":["[s00001]","invented"]},{"text":"Unsupported","citations":["s99999"]}]),
        json!([{"task":"Send notes","owner":"  ","due":"","citations":["s00002"]}]),
    );
    let provider = FakeProvider::with(&[&output]);
    let mut steps = Vec::new();
    let result = summarize(
        &provider,
        &transcript(2, Language::Vi),
        &Participants::new(Default::default(), None, None, false),
        &SummaryOptions {
            input_tokens: 4000,
            max_tokens: 512,
        },
        &mut |d, t| steps.push((d, t)),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(steps, [(1, 1)]);
    assert_eq!(result.overview.len(), 1);
    assert_eq!(result.overview[0].citations[0].start_ms, 65_000);
    assert_eq!(result.dropped_items, 1);
    assert_eq!(result.action_items[0].owner, None);
    assert_eq!(result.action_items[0].due, None);
    assert!(provider.requests.lock().unwrap()[0]
        .system
        .contains("Vietnamese"));
    assert_eq!(result.revision, 3);
}
#[test]
fn gemini_summary_keeps_only_citations_to_this_meeting() {
    let output = raw(json!([{"text":"Decision","citations":["s00001"]},{"text":"Invented","citations":["s99999"]}]), json!([]));
    let provider = FakeProvider::with(&[&output]);
    let provider = GeminiProvider(provider);
    let result = summarize(&provider, &transcript(2, Language::En), &Participants::new(Default::default(), None, None, false), &SummaryOptions::for_provider(ProviderKind::Gemini), &mut |_, _| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.provider, ProviderKind::Gemini);
    assert_eq!(result.overview.len(), 1);
    assert_eq!(result.overview[0].citations[0].start_ms, 65_000);
    assert_eq!(result.dropped_items, 1);
}

struct GeminiProvider(FakeProvider);
impl TextProvider for GeminiProvider {
    fn kind(&self) -> ProviderKind { ProviderKind::Gemini }
    fn model(&self) -> &str { "gemini-2.5-flash" }
    fn generate(&self, request: &GenerateRequest, on_delta: &mut dyn FnMut(&str), cancel: &AtomicBool) -> Result<String, AiError> { self.0.generate(request, on_delta, cancel) }
}
#[test]
fn long_transcript_is_chunked_without_losing_late_segments_and_reduced() {
    let transcript = transcript(30, Language::En);
    let options = SummaryOptions {
        input_tokens: 750,
        max_tokens: 200,
    };
    let maps = summary_requests(&transcript, &Participants::new(Default::default(), None, None, false), &options);
    assert!(maps.len() > 1);
    for segment in &transcript.segments {
        assert_eq!(
            maps.iter()
                .filter(|request| request.messages[0]
                    .content
                    .contains(&format!("[{}]", segment.id)))
                .count(),
            1
        );
    }
    let map_outputs: Vec<_> = maps
        .iter()
        .enumerate()
        .map(|(index, request)| {
            let last = request.messages[0]
                .content
                .lines()
                .filter_map(|line| {
                    line.strip_prefix('[')?
                        .split_once(']')
                        .map(|(id, _)| id.to_owned())
                })
                .next_back()
                .unwrap();
            let citations = if index == 0 {
                vec![last.clone()]
            } else {
                vec![last.clone(), "s00001".into()]
            };
            raw(
                json!([{"text":format!("Point {last}"),"citations":citations}]),
                json!([]),
            )
        })
        .collect();
    let late = transcript.segments.last().unwrap().id.clone();
    let reduce = raw(
        json!([{"text":"Whole meeting","citations":[late]}]),
        json!([]),
    );
    let mut all = map_outputs;
    all.extend(std::iter::repeat_n(reduce, 30));
    let provider = FakeProvider::with(&all.iter().map(String::as_str).collect::<Vec<_>>());
    let result = summarize(
        &provider,
        &transcript,
        &Participants::new(Default::default(), None, None, false),
        &options,
        &mut |_, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(provider.requests.lock().unwrap().len() > maps.len());
    let requests = provider.requests.lock().unwrap();
    assert!(!requests[maps.len()].messages[0].content.contains("s00001"));
    assert!(result
        .overview
        .iter()
        .any(|item| item.citations.iter().any(|c| c.segment_id == "s00030")));
}
#[test]
fn cancel_between_map_calls_stops_before_another_request() {
    let transcript = transcript(30, Language::En);
    let options = SummaryOptions {
        input_tokens: 500,
        max_tokens: 100,
    };
    assert!(summary_requests(&transcript, &Participants::new(Default::default(), None, None, false), &options).len() > 1);
    let output = raw(json!([]), json!([]));
    let provider = FakeProvider::with(&[&output]);
    let cancel = AtomicBool::new(false);
    let result = summarize(
        &provider,
        &transcript,
        &Participants::new(Default::default(), None, None, false),
        &options,
        &mut |_, _| cancel.store(true, Ordering::SeqCst),
        &cancel,
    );
    assert!(matches!(result, Err(AiError::Cancelled)));
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}
#[test]
fn summary_round_trip_and_markdown_sections() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_summary(dir.path()).unwrap(), None);
    let provider = FakeProvider::with(&[&raw(
        json!([{"text":"Launch plan","citations":["s00001"]}]),
        json!([{"task":"Ship","owner":"Pat","due":"Friday","citations":["s00002"]}]),
    )]);
    let summary = summarize(
        &provider,
        &transcript(2, Language::En),
        &Participants::new(Default::default(), None, None, false),
        &SummaryOptions {
            input_tokens: 4000,
            max_tokens: 200,
        },
        &mut |_, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    write_summary(dir.path(), "Planning", &summary).unwrap();
    assert_eq!(read_summary(dir.path()).unwrap(), Some(summary.clone()));
    let markdown = render_summary_markdown("Planning", &summary);
    assert!(markdown.contains("## Key points\n_None_"));
    assert!(markdown.contains("Launch plan [00:01:05]"));
    assert!(markdown.contains("Ship — owner: Pat · due: Friday [00:02:10]"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("summary.md")).unwrap(),
        markdown
    );
    assert!(std::fs::read_to_string(dir.path().join("summary.json"))
        .unwrap()
        .ends_with('\n'));
}

#[test]
fn renaming_the_user_changes_the_fingerprint_even_without_their_segments() {
    let mut remote_only = transcript(2, Language::En);
    for segment in &mut remote_only.segments { segment.speaker = Speaker::Remote; }
    let before = Participants::new(Default::default(), None, Some("Duc"), false);
    let after = Participants::new([("me".into(), "Alex".into())].into(), None, Some("Duc"), false);
    assert_ne!(
        meeting_minutes_core::summary::participants_fingerprint(&remote_only, &before),
        meeting_minutes_core::summary::participants_fingerprint(&remote_only, &after)
    );
}

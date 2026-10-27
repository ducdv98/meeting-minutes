use std::{collections::HashSet, fs, path::Path};

use chrono::DateTime;
use meeting_minutes_core::Library;
use serde::Deserialize;
use tempfile::TempDir;

#[derive(Deserialize)]
struct EvaluationSet {
    now: String,
    questions: Vec<Question>,
}

#[derive(Deserialize)]
struct Question {
    id: String,
    question: String,
    language: String,
    tags: Vec<String>,
    expected_answer: String,
    supporting: Vec<Support>,
    keywords: Vec<String>,
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
fn evaluation_library_rebuilds_with_published_transcripts_and_searchable_languages() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library-ask");
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    copy_dir(&fixture.join("Library"), &root);

    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let report = library.rescan().unwrap();
    assert!(report.malformed.is_empty(), "{:?}", report.malformed);
    assert_eq!(report.meetings.len(), 10);

    let draft_id = "2026-10-23T09-00-00";
    for meeting in &report.meetings {
        let detail = library.meeting(&meeting.id).unwrap();
        assert_eq!(detail.title, meeting.title);
        assert!(!detail.participants.is_empty());
        let transcript = library.transcript(&meeting.id).unwrap();
        if meeting.id == draft_id {
            assert!(transcript.is_none());
        } else {
            let transcript = transcript.unwrap();
            assert!((30..=80).contains(&transcript.segments.len()));
            assert_eq!(detail.language.as_deref(), Some(transcript.language.code()));
            assert!(
                library
                    .participants(&meeting.id)
                    .unwrap()
                    .list(&transcript)
                    .len()
                    >= 2
            );
        }
    }
    assert!(library
        .search("procurement")
        .unwrap()
        .iter()
        .any(|hit| hit.meeting_id == "2026-07-09T10-00-00"));
    assert!(library
        .search("ngân sách")
        .unwrap()
        .iter()
        .any(|hit| hit.meeting_id == "2026-10-08T09-00-00"));
    assert!(library.search("unpublishedneedle").unwrap().is_empty());

    let evaluation: EvaluationSet =
        serde_json::from_slice(&fs::read(fixture.join("questions.json")).unwrap()).unwrap();
    assert_eq!(evaluation.now, "2026-10-27T12:00:00Z");
    let now = DateTime::parse_from_rfc3339(&evaluation.now).unwrap();
    assert!((18..=22).contains(&evaluation.questions.len()));
    let mut ids = HashSet::new();
    let mut tags = HashSet::new();
    let mut languages = HashSet::new();
    for question in evaluation.questions {
        assert!(ids.insert(question.id.clone()));
        assert!(!question.question.trim().is_empty());
        assert!(!question.expected_answer.trim().is_empty());
        assert!(!question.keywords.is_empty());
        assert!(matches!(question.language.as_str(), "en" | "vi"));
        languages.insert(question.language.clone());
        tags.extend(question.tags.iter().cloned());
        if question.tags.iter().any(|tag| tag == "answerable") {
            assert!(!question.supporting.is_empty(), "{}", question.id);
        } else {
            assert!(question.supporting.is_empty(), "{}", question.id);
            assert!(question.tags.iter().any(|tag| tag == "out-of-library"));
        }
        for support in question.supporting {
            let meeting = report
                .meetings
                .iter()
                .find(|meeting| meeting.id == support.meeting_id)
                .unwrap();
            let started_at = DateTime::parse_from_rfc3339(&meeting.started_at).unwrap();
            for days in [7, 30, 90] {
                if question
                    .tags
                    .iter()
                    .any(|tag| tag == &format!("out-of-range-for:{days}"))
                {
                    assert!(
                        now.signed_duration_since(started_at).num_days() >= days,
                        "{}: {}",
                        question.id,
                        support.meeting_id
                    );
                }
            }
            let transcript = library.transcript(&support.meeting_id).unwrap().unwrap();
            if question.tags.iter().any(|tag| tag == "cross-language") {
                assert_ne!(transcript.language.code(), question.language);
            }
            assert!(
                transcript
                    .segments
                    .iter()
                    .any(|segment| segment.id == support.segment_id),
                "{}: {} / {}",
                question.id,
                support.meeting_id,
                support.segment_id
            );
        }
    }
    assert_eq!(languages, HashSet::from(["en".to_owned(), "vi".to_owned()]));
    for tag in [
        "answerable",
        "paraphrased",
        "cross-language",
        "out-of-range-for:7",
        "out-of-range-for:30",
        "out-of-range-for:90",
        "out-of-library",
    ] {
        assert!(tags.contains(tag), "missing tag: {tag}");
    }
}

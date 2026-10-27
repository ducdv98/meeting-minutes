use chrono::{DateTime, FixedOffset};
use meeting_minutes_core::{transcript::Language, Library};
use tempfile::TempDir;

fn library(temp: &TempDir) -> Library {
    Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap()
}

fn add_meeting(
    library: &mut Library,
    title: &str,
    started_at: &str,
    language: Option<Language>,
    segments: &[(&str, &str)],
) -> String {
    let started_at: DateTime<FixedOffset> = DateTime::parse_from_rfc3339(started_at).unwrap();
    let meeting = library
        .create_meeting_with_language(title, started_at, language)
        .unwrap();
    if !segments.is_empty() {
        let segments: Vec<_> = segments
            .iter()
            .enumerate()
            .map(|(index, (id, text))| {
                serde_json::json!({
                    "id": id, "start_ms": index as u64 * 1000,
                    "end_ms": index as u64 * 1000 + 900,
                    "text": text, "speaker": "remote"
                })
            })
            .collect();
        let transcript = serde_json::json!({
            "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
            "model": "small", "language": "en", "language_detected": false,
            "duration_ms": segments.len() as u64 * 1000, "segments": segments
        });
        std::fs::write(
            library.meeting_dir(&meeting.id).join("transcript.r1.json"),
            serde_json::to_vec(&transcript).unwrap(),
        )
        .unwrap();
        library.refresh_transcript(&meeting.id).unwrap();
    }
    meeting.id
}

#[test]
fn no_matching_segments_returns_no_evidence() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    add_meeting(
        &mut library,
        "Planning",
        "2026-10-12T09:30:00Z",
        Some(Language::En),
        &[("s1", "The budget is approved")],
    );

    assert!(library
        .library_evidence(&["migration".into()], None, 100)
        .unwrap()
        .is_empty());
}

#[test]
fn matching_segment_brings_its_neighbours_and_meeting_details() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    let id = add_meeting(
        &mut library,
        "Migration planning",
        "2026-10-12T09:30:00Z",
        Some(Language::En),
        &[
            ("s1", "We can start next week"),
            ("s2", "The migration is approved"),
            ("s3", "Anna will prepare the checklist"),
            ("s4", "Another topic begins"),
        ],
    );
    library
        .set_participant_name(
            &id,
            meeting_minutes_core::transcript::Speaker::Remote,
            Some("Anna"),
        )
        .unwrap();

    let evidence = library
        .library_evidence(&["migration".into()], None, 100)
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].meeting_id, id);
    assert_eq!(evidence[0].title, "Migration planning");
    assert_eq!(evidence[0].started_at, "2026-10-12T09:30:00+00:00");
    assert_eq!(evidence[0].language, Some(Language::En));
    assert_eq!(
        evidence[0]
            .segments
            .iter()
            .map(|segment| segment.segment_id.as_str())
            .collect::<Vec<_>>(),
        ["s1", "s2", "s3"]
    );
    assert_eq!(evidence[0].segments[1].revision, 1);
    assert_eq!(evidence[0].segments[1].start_ms, 1000);
    assert_eq!(evidence[0].segments[1].end_ms, 1900);
    assert_eq!(evidence[0].segments[1].participant, "Anna");
    assert_eq!(evidence[0].segments[1].text, "The migration is approved");
}

#[test]
fn meetings_follow_their_best_bm25_hit_and_segments_follow_timeline_order() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    let common = add_meeting(
        &mut library,
        "Budget review",
        "2026-10-13T09:30:00Z",
        None,
        &[
            ("c1", "budget planning"),
            ("c2", "a separate point"),
            ("c3", "another point"),
            ("c4", "budget again"),
        ],
    );
    let strong = add_meeting(
        &mut library,
        "Migration decision",
        "2026-10-12T09:30:00Z",
        None,
        &[("r1", "budget migration approved")],
    );

    let evidence = library
        .library_evidence(&["budget".into(), "migration".into()], None, 200)
        .unwrap();
    assert_eq!(evidence.len(), 2);
    assert_eq!(evidence[0].meeting_id, strong);
    assert_eq!(evidence[1].meeting_id, common);
    assert_eq!(
        evidence[1]
            .segments
            .iter()
            .map(|segment| segment.segment_id.as_str())
            .collect::<Vec<_>>(),
        ["c1", "c2", "c3", "c4"]
    );
}

#[test]
fn meeting_languages_are_distinct_across_the_library() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    for (date, language) in [
        ("2026-10-12T09:30:00Z", Some(Language::Vi)),
        ("2026-10-13T09:30:00Z", Some(Language::En)),
        ("2026-10-14T09:30:00Z", Some(Language::Vi)),
        ("2026-10-15T09:30:00Z", None),
    ] {
        add_meeting(&mut library, "Meeting", date, language, &[]);
    }

    assert_eq!(
        library.meeting_languages().unwrap(),
        vec![Language::En, Language::Vi]
    );
}

#[test]
fn evidence_never_exceeds_the_token_budget() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    add_meeting(
        &mut library,
        "Budget",
        "2026-10-12T09:30:00Z",
        None,
        &[
            ("s1", "A long explanation of our previous plan"),
            ("s2", "budget approved"),
            ("s3", "A long explanation of our next plan"),
        ],
    );

    let evidence = library
        .library_evidence(&["budget".into()], None, 10)
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].segments.len(), 1);
    assert_eq!(evidence[0].segments[0].segment_id, "s2");
    let lines = evidence[0]
        .segments
        .iter()
        .map(|segment| {
            format!(
                "[{}] {}: {}",
                segment.segment_id, segment.participant, segment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(meeting_minutes_core::citation::estimate_tokens(&lines) <= 10);
    assert!(library
        .library_evidence(&["budget".into()], None, 0)
        .unwrap()
        .is_empty());
}

#[test]
fn since_ms_filters_by_meeting_start_including_the_boundary() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    add_meeting(
        &mut library,
        "Earlier",
        "2026-10-12T09:30:00Z",
        None,
        &[("s1", "migration")],
    );
    let recent = add_meeting(
        &mut library,
        "Recent",
        "2026-10-13T09:30:00+07:00",
        None,
        &[("s2", "migration")],
    );
    let boundary = DateTime::parse_from_rfc3339("2026-10-13T02:30:00Z")
        .unwrap()
        .timestamp_millis();

    let evidence = library
        .library_evidence(&["migration".into()], Some(boundary), 100)
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].meeting_id, recent);
}

#[test]
fn vietnamese_diacritics_and_fts_syntax_are_handled_as_search_terms() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    let id = add_meeting(
        &mut library,
        "Pricing",
        "2026-10-12T09:30:00Z",
        Some(Language::Vi),
        &[("s1", "Giá đã được chốt")],
    );

    assert_eq!(
        library
            .library_evidence(&["gia".into()], None, 100)
            .unwrap()[0]
            .meeting_id,
        id
    );
    assert_eq!(
        library
            .library_evidence(&["gia\") OR NOT (".into()], None, 100)
            .unwrap()[0]
            .meeting_id,
        id
    );
    assert!(library
        .library_evidence(&["!\"()".into()], None, 100)
        .unwrap()
        .is_empty());
}

#[test]
fn unpublished_draft_and_replaced_revision_do_not_supply_evidence() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    let draft = add_meeting(
        &mut library,
        "Processing",
        "2026-10-12T09:30:00Z",
        None,
        &[],
    );
    std::fs::write(
        library.meeting_dir(&draft).join("transcript.draft.json"),
        br#"{"text":"secret migration"}"#,
    )
    .unwrap();
    let published = add_meeting(
        &mut library,
        "Published",
        "2026-10-13T09:30:00Z",
        None,
        &[("s1", "old migration")],
    );
    let replacement = serde_json::json!({
        "schema_version": 1, "revision": 2, "created_at": "2026-10-13T10:00:00Z",
        "model": "small", "language": "en", "language_detected": false,
        "duration_ms": 1000, "segments": [{
            "id": "s2", "start_ms": 0, "end_ms": 900,
            "text": "new decision", "speaker": "remote"
        }]
    });
    std::fs::write(
        library.meeting_dir(&published).join("transcript.r2.json"),
        serde_json::to_vec(&replacement).unwrap(),
    )
    .unwrap();
    library.refresh_transcript(&published).unwrap();

    assert!(library
        .library_evidence(&["migration".into()], None, 100)
        .unwrap()
        .is_empty());
    assert_eq!(
        library
            .library_evidence(&["decision".into()], None, 100)
            .unwrap()[0]
            .segments[0]
            .revision,
        2
    );
}

#[test]
fn retrieval_has_no_meeting_count_cap() {
    let temp = TempDir::new().unwrap();
    let mut library = library(&temp);
    for day in 1..=12 {
        add_meeting(
            &mut library,
            "Planning",
            &format!("2026-10-{day:02}T09:30:00Z"),
            None,
            &[("s1", "budget")],
        );
    }

    assert_eq!(
        library
            .library_evidence(&["budget".into()], None, 200)
            .unwrap()
            .len(),
        12
    );
}

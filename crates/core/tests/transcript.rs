use meeting_minutes_core::participant::{MeetingParticipants, Participants};
use meeting_minutes_core::transcript::{
    assign_ids, place_segments, render_markdown, Language, PlacedSegment, RawSegment, Speaker,
    Transcript,
};
use meeting_minutes_core::Library;
use tempfile::TempDir;

#[test]
fn participants_follow_settings_merge_and_voice_note_rules() {
    let segment = |speaker| meeting_minutes_core::transcript::Segment {
        id: "s00001".into(),
        start_ms: 0,
        end_ms: 100,
        text: "Hi".into(),
        speaker,
    };
    let mut names = MeetingParticipants::new();
    assert_eq!(
        Participants::new(names.clone(), None, None, false).name(&segment(Speaker::Me), 1),
        "Me"
    );
    assert_eq!(
        Participants::new(names.clone(), None, Some("Duc"), false).name(&segment(Speaker::Me), 1),
        "Duc"
    );
    assert_eq!(
        Participants::new(names.clone(), None, None, false).name(&segment(Speaker::Remote), 1),
        "Remote"
    );
    assert_eq!(
        Participants::new(names.clone(), None, None, false).name(&segment(Speaker::Unknown), 1),
        "Unknown"
    );
    names.insert("remote".into(), "Anna".into());
    names.insert("unknown".into(), "Anna".into());
    assert_eq!(
        Participants::new(names.clone(), None, None, false).name(&segment(Speaker::Remote), 1),
        "Anna"
    );
    assert_eq!(
        Participants::new(names.clone(), None, None, false).name(&segment(Speaker::Unknown), 1),
        "Anna"
    );
    assert_eq!(
        Participants::new(names, None, Some("Duc"), true).name(&segment(Speaker::Unknown), 1),
        "Duc"
    );
}

#[test]
fn normalizes_and_places_engine_output() {
    let placed = place_segments(
        &[
            RawSegment {
                start_ms: 600,
                end_ms: 2500,
                text: "  two \n words  ".into(),
            },
            RawSegment {
                start_ms: 0,
                end_ms: 300,
                text: " [BLANK_AUDIO] ".into(),
            },
            RawSegment {
                start_ms: 0,
                end_ms: 200,
                text: "  ".into(),
            },
            RawSegment {
                start_ms: 1000,
                end_ms: 1000,
                text: "zero".into(),
            },
            RawSegment {
                start_ms: 100,
                end_ms: 500,
                text: " first ".into(),
            },
            RawSegment {
                start_ms: 300,
                end_ms: 350,
                text: "[hello] text [more]".into(),
            },
        ],
        2000,
        5000,
        Speaker::Me,
    );
    assert_eq!(placed.len(), 3);
    assert_eq!(
        (
            placed[0].start_ms,
            placed[0].end_ms,
            placed[0].text.as_str()
        ),
        (5100, 5500, "first")
    );
    assert_eq!(placed[1].text, "[hello] text [more]");
    assert_eq!(
        (
            placed[2].start_ms,
            placed[2].end_ms,
            placed[2].text.as_str()
        ),
        (5600, 7000, "two words")
    );
}

#[test]
fn assigns_ordered_ids_clamps_duration_and_reload_keeps_ids() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting(
            "Planning",
            chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap(),
        )
        .unwrap();
    let segments = assign_ids(
        vec![
            PlacedSegment {
                start_ms: 1900,
                end_ms: 2500,
                text: "second".into(),
                speaker: Speaker::Remote,
            },
            PlacedSegment {
                start_ms: 0,
                end_ms: 500,
                text: "first".into(),
                speaker: Speaker::Me,
            },
            PlacedSegment {
                start_ms: 2100,
                end_ms: 2200,
                text: "outside".into(),
                speaker: Speaker::Me,
            },
        ],
        2000,
    );
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.id.as_str(), s.start_ms, s.end_ms))
            .collect::<Vec<_>>(),
        vec![("s00001", 0, 500), ("s00002", 1900, 2000)]
    );
    let transcript = Transcript {
        schema_version: 1,
        revision: 1,
        created_at: "2026-10-12T10:00:00Z".into(),
        model: "small".into(),
        language: Language::En,
        language_detected: false,
        duration_ms: 2000,
        segments,
    };
    std::fs::write(
        library.meeting_dir(&meeting.id).join("transcript.r1.json"),
        serde_json::to_vec(&transcript).unwrap(),
    )
    .unwrap();
    assert_eq!(
        library.transcript(&meeting.id).unwrap().unwrap().segments,
        transcript.segments
    );
    assert!(render_markdown("Planning", &transcript).contains("**[00:00:00] Me:** first"));
}

#[test]
fn meeting_language_updates_without_discarding_other_metadata() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting(
            "Planning",
            chrono::DateTime::parse_from_rfc3339("2026-10-12T09:30:00Z").unwrap(),
        )
        .unwrap();
    let path = library.meeting_dir(&meeting.id).join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    metadata["future"] = serde_json::json!({ "kept": true });
    std::fs::write(&path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    library
        .set_language(&meeting.id, Some(Language::Vi))
        .unwrap();
    assert_eq!(
        library.meeting(&meeting.id).unwrap().language.as_deref(),
        Some("vi")
    );
    let updated: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(updated["future"], metadata["future"]);
    library.set_language(&meeting.id, None).unwrap();
    assert_eq!(library.meeting(&meeting.id).unwrap().language, None);
}

use meeting_minutes_core::transcript::{
    assign_ids, place_segments, render_markdown, speaker_label, Activity, Language, PlacedSegment,
    RawSegment, Speaker, Transcript,
};
use meeting_minutes_core::Library;
use tempfile::TempDir;

#[test]
fn normalizes_and_places_engine_output() {
    let activity = Activity {
        microphone: Some(vec![0.1; 100]),
        system_audio: Some(vec![0.0; 100]),
    };
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
        &activity,
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
fn labels_source_activity_and_threshold() {
    let activity = |mic: Option<Vec<f32>>, sys: Option<Vec<f32>>| Activity {
        microphone: mic,
        system_audio: sys,
    };
    assert_eq!(
        speaker_label(&activity(Some(vec![0.02; 20]), Some(vec![0.0; 20])), 0, 400),
        Speaker::Me
    );
    assert_eq!(
        speaker_label(&activity(Some(vec![0.0; 20]), Some(vec![0.02; 20])), 0, 400),
        Speaker::Remote
    );
    assert_eq!(
        speaker_label(
            &activity(Some(vec![0.02; 20]), Some(vec![0.02; 20])),
            0,
            400
        ),
        Speaker::Unknown
    );
    assert_eq!(
        speaker_label(&activity(Some(vec![0.0; 20]), Some(vec![0.0; 20])), 0, 400),
        Speaker::Unknown
    );
    assert_eq!(
        speaker_label(&activity(None, None), 0, 400),
        Speaker::Unknown
    );
    assert_eq!(
        speaker_label(
            &activity(
                Some([vec![0.02; 3], vec![0.0; 17]].concat()),
                Some(vec![0.0; 20])
            ),
            0,
            400
        ),
        Speaker::Me
    );
    assert_eq!(
        speaker_label(
            &activity(
                Some([vec![0.02; 2], vec![0.0; 18]].concat()),
                Some(vec![0.0; 20])
            ),
            0,
            400
        ),
        Speaker::Unknown
    );
}

#[test]
fn long_segment_counts_a_half_second_remote_interjection() {
    let activity = Activity {
        microphone: Some(vec![0.02; 500]),
        system_audio: Some([vec![0.0; 200], vec![0.02; 25], vec![0.0; 275]].concat()),
    };
    assert_eq!(speaker_label(&activity, 0, 10_000), Speaker::Unknown);
    let activity = Activity {
        microphone: Some(vec![0.02; 500]),
        system_audio: Some([vec![0.0; 200], vec![0.02; 24], vec![0.0; 276]].concat()),
    };
    assert_eq!(speaker_label(&activity, 0, 10_000), Speaker::Me);
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

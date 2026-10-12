use chrono::{DateTime, FixedOffset};
use meeting_minutes_core::ai::ProviderKind;
use meeting_minutes_core::{Library, LibraryError, OrphanRecording};
use tempfile::TempDir;

fn at(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).unwrap()
}

#[test]
fn cloud_consent_is_per_meeting_and_provider_and_keeps_first_timestamp() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    assert!(!library
        .cloud_consent(&meeting.id, ProviderKind::Openai)
        .unwrap());
    library
        .record_cloud_consent(
            &meeting.id,
            ProviderKind::Openai,
            at("2026-10-12T10:00:00Z"),
        )
        .unwrap();
    library
        .record_cloud_consent(
            &meeting.id,
            ProviderKind::Openai,
            at("2026-10-13T10:00:00Z"),
        )
        .unwrap();
    assert!(library
        .cloud_consent(&meeting.id, ProviderKind::Openai)
        .unwrap());
    assert!(!library
        .cloud_consent(&meeting.id, ProviderKind::Anthropic)
        .unwrap());
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(library.meeting_dir(&meeting.id).join("meeting.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        metadata["cloud_consent"][0]["consented_at"],
        "2026-10-12T10:00:00+00:00"
    );
    assert!(library
        .record_cloud_consent(
            &meeting.id,
            ProviderKind::Ollama,
            at("2026-10-12T10:00:00Z")
        )
        .is_err());
    assert!(matches!(
        library.cloud_consent("missing", ProviderKind::Openai),
        Err(LibraryError::MeetingNotFound)
    ));
    assert!(matches!(
        library.record_cloud_consent("missing", ProviderKind::Openai, at("2026-10-12T10:00:00Z")),
        Err(LibraryError::MeetingNotFound)
    ));
    let renamed = library.rename_meeting(&meeting.id, "Review").unwrap();
    assert!(library
        .cloud_consent(&renamed.id, ProviderKind::Openai)
        .unwrap());
    library.rescan().unwrap();
    assert!(library
        .cloud_consent(&renamed.id, ProviderKind::Openai)
        .unwrap());
}

#[test]
fn editing_current_segment_persists_text_without_changing_its_citation() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let folder = library.meeting_dir(&meeting.id);
    let revision = serde_json::json!({
        "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
        "model": "small", "language": "en", "language_detected": false,
        "duration_ms": 2000, "segments": [{"id": "s00001", "start_ms": 100, "end_ms": 900, "text": "Hello", "speaker": "me"}]
    });
    std::fs::write(
        folder.join("transcript.r1.json"),
        serde_json::to_vec(&revision).unwrap(),
    )
    .unwrap();

    let edited = library
        .edit_segment(&meeting.id, 1, "s00001", "  New\n   words  ")
        .unwrap();
    assert_eq!(edited.revision, 1);
    assert_eq!(edited.segment("s00001").unwrap().text, "New words");
    assert_eq!(edited.segment("s00001").unwrap().start_ms, 100);
    assert_eq!(edited.segment("s00001").unwrap().end_ms, 900);
    assert_eq!(
        edited.segment("s00001").unwrap().speaker,
        meeting_minutes_core::transcript::Speaker::Me
    );
    assert_eq!(library.transcript(&meeting.id).unwrap(), Some(edited));
    assert!(std::fs::read_to_string(folder.join("transcript.md"))
        .unwrap()
        .contains("**[00:00:00] Me:** New words"));
    assert!(!folder.join("transcript.r1.json.edit.tmp").exists());
    assert!(matches!(
        library.edit_segment(&meeting.id, 1, "s00001", " \n "),
        Err(LibraryError::EmptySegmentText)
    ));
    assert!(matches!(
        library.edit_segment(&meeting.id, 1, "missing", "Text"),
        Err(LibraryError::SegmentNotFound)
    ));
}

#[test]
fn older_revision_remains_readable_and_cannot_be_edited() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let folder = library.meeting_dir(&meeting.id);
    for (revision, text) in [(1, "Earlier"), (2, "Later")] {
        let value = serde_json::json!({
            "schema_version": 1, "revision": revision, "created_at": "2026-10-12T10:00:00Z",
            "model": "small", "language": "en", "language_detected": false,
            "duration_ms": 1000, "segments": [{"id": "s00001", "start_ms": 0, "end_ms": 1000, "text": text, "speaker": "me"}]
        });
        std::fs::write(
            folder.join(format!("transcript.r{revision}.json")),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    let earlier = std::fs::read(folder.join("transcript.r1.json")).unwrap();
    assert_eq!(
        library
            .transcript_revision(&meeting.id, 1)
            .unwrap()
            .unwrap()
            .segment("s00001")
            .unwrap()
            .text,
        "Earlier"
    );
    assert_eq!(
        library
            .transcript(&meeting.id)
            .unwrap()
            .unwrap()
            .segment("s00001")
            .unwrap()
            .text,
        "Later"
    );
    assert!(library
        .transcript_revision(&meeting.id, 3)
        .unwrap()
        .is_none());
    assert!(matches!(
        library.edit_segment(&meeting.id, 1, "s00001", "Changed"),
        Err(LibraryError::StaleRevision)
    ));
    assert_eq!(
        std::fs::read(folder.join("transcript.r1.json")).unwrap(),
        earlier
    );
}

#[test]
fn creating_a_meeting_makes_a_readable_folder_and_index_entry() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("data/index.sqlite3")).unwrap();

    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00+07:00"))
        .unwrap();

    assert_eq!(meeting.id, "2026-10-12 0930 Planning");
    assert_eq!(meeting.title, "Planning");
    assert_eq!(meeting.started_at, "2026-10-12T09:30:00+07:00");
    assert_eq!(library.list_meetings().unwrap()[0].id, meeting.id);
    let metadata = std::fs::read_to_string(root.join(&meeting.id).join("meeting.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&metadata).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "schema_version": 1, "title": "Planning", "started_at": "2026-10-12T09:30:00+07:00",
            "language": null, "duration_ms": 0, "audio": null, "recordings": [], "markers": []
        })
    );
}

#[test]
fn collisions_use_suffixes_without_changing_existing_folders_or_metadata() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let occupied = root.join("2026-10-12 0930 Planning");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("meeting.json"), b"keep me").unwrap();
    std::fs::write(occupied.join("other.txt"), b"also keep me").unwrap();
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();

    let first = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let second = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();

    assert_eq!(first.id, "2026-10-12 0930 Planning (2)");
    assert_eq!(second.id, "2026-10-12 0930 Planning (3)");
    assert_eq!(
        std::fs::read(occupied.join("meeting.json")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        std::fs::read(occupied.join("other.txt")).unwrap(),
        b"also keep me"
    );
}

#[test]
fn two_meetings_in_the_same_minute_get_distinct_names() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let first = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let second = library
        .create_meeting("Planning", at("2026-10-12T09:30:59Z"))
        .unwrap();
    assert_eq!(first.id, "2026-10-12 0930 Planning");
    assert_eq!(second.id, "2026-10-12 0930 Planning (2)");
}

#[test]
fn an_occupied_folder_without_metadata_is_preserved() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let occupied = root.join("2026-10-12 0930 Planning");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("notes.txt"), b"private notes").unwrap();
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    assert_eq!(meeting.id, "2026-10-12 0930 Planning (2)");
    assert!(!occupied.join("meeting.json").exists());
    assert_eq!(
        std::fs::read(occupied.join("notes.txt")).unwrap(),
        b"private notes"
    );
}

#[test]
fn deleting_the_index_and_rescanning_rebuilds_the_same_meetings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let index = temp.path().join("data/index.sqlite3");
    let before = {
        let mut library = Library::open(&root, &index).unwrap();
        library
            .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
            .unwrap();
        library
            .create_meeting("Review", at("2026-10-13T11:00:00Z"))
            .unwrap();
        library.list_meetings().unwrap()
    };
    std::fs::remove_file(&index).unwrap();

    let mut reopened = Library::open(&root, &index).unwrap();
    assert_eq!(reopened.list_meetings().unwrap(), before);
    let report = reopened.rescan().unwrap();
    assert_eq!(report.meetings, before);
    assert!(report.malformed.is_empty());
    assert_eq!(reopened.list_meetings().unwrap(), before);
}

#[test]
fn rescan_picks_up_hand_added_folders_and_edited_titles() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let existing = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let existing_path = root.join(&existing.id).join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&existing_path).unwrap()).unwrap();
    metadata["title"] = serde_json::json!("Updated planning");
    metadata["extra"] = serde_json::json!({"hand_edited": true});
    metadata["recordings"] = serde_json::json!([{"future": "data"}]);
    std::fs::write(existing_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let added = root.join("Custom folder");
    std::fs::create_dir(&added).unwrap();
    std::fs::write(
        added.join("meeting.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1, "title": "Added", "started_at": "2026-10-14T12:00:00+07:00",
            "language": "vi", "recordings": [], "markers": []
        }))
        .unwrap(),
    )
    .unwrap();

    let report = library.rescan().unwrap();
    assert_eq!(
        report
            .meetings
            .iter()
            .map(|m| (m.id.as_str(), m.title.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Custom folder", "Added"),
            ("2026-10-12 0930 Planning", "Updated planning")
        ]
    );
    assert!(report.malformed.is_empty());
}

#[test]
fn rescan_reports_bad_folders_and_keeps_valid_meetings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    library
        .create_meeting("Good", at("2026-10-12T09:30:00Z"))
        .unwrap();
    for folder in ["a missing", "b invalid", "c version", "d date"] {
        std::fs::create_dir(root.join(folder)).unwrap();
    }
    std::fs::write(root.join("b invalid/meeting.json"), b"{").unwrap();
    std::fs::write(root.join("c version/meeting.json"), br#"{"schema_version":2,"title":"Bad","started_at":"2026-10-12T09:30:00Z","language":null,"recordings":[],"markers":[]}"#).unwrap();
    std::fs::write(root.join("d date/meeting.json"), br#"{"schema_version":1,"title":"Bad","started_at":"yesterday","language":null,"recordings":[],"markers":[]}"#).unwrap();
    std::fs::create_dir(root.join(".hidden")).unwrap();
    std::fs::write(root.join("ordinary.txt"), b"ignored").unwrap();

    let report = library.rescan().unwrap();
    assert_eq!(report.meetings.len(), 1);
    assert_eq!(report.meetings[0].title, "Good");
    assert_eq!(
        report
            .malformed
            .iter()
            .map(|m| m.folder.as_str())
            .collect::<Vec<_>>(),
        vec!["a missing", "b invalid", "c version", "d date"]
    );
    assert_eq!(report.malformed[0].reason, "missing meeting.json");
    assert!(report.malformed.iter().all(|m| !m.reason.is_empty()));
}

#[test]
fn list_orders_by_instant_then_title_then_folder_id() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    library
        .create_meeting("Later", at("2026-10-12T10:00:00Z"))
        .unwrap();
    library
        .create_meeting("Zulu", at("2026-10-12T11:00:00+01:00"))
        .unwrap();
    library
        .create_meeting("Alpha", at("2026-10-12T10:00:00Z"))
        .unwrap();
    library
        .create_meeting("Alpha", at("2026-10-12T10:00:00Z"))
        .unwrap();
    library
        .create_meeting("Newest", at("2026-10-12T10:01:00Z"))
        .unwrap();
    assert_eq!(
        library
            .list_meetings()
            .unwrap()
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "2026-10-12 1001 Newest",
            "2026-10-12 1000 Alpha",
            "2026-10-12 1000 Alpha (2)",
            "2026-10-12 1000 Later",
            "2026-10-12 1100 Zulu"
        ]
    );
}

#[test]
fn corrupt_index_is_recreated_and_can_be_rebuilt() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let index = temp.path().join("index.sqlite3");
    {
        let mut library = Library::open(&root, &index).unwrap();
        library
            .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
            .unwrap();
    }
    std::fs::write(&index, b"not a database").unwrap();
    let library = Library::open(&root, &index).unwrap();
    assert_eq!(library.list_meetings().unwrap()[0].title, "Planning");
}

#[test]
fn orphan_recordings_are_sorted_across_meetings() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let b = library
        .create_meeting("B", at("2026-10-12T09:31:00Z"))
        .unwrap();
    let a = library
        .create_meeting("A", at("2026-10-12T09:30:00Z"))
        .unwrap();
    for (id, number) in [(&b.id, 2), (&a.id, 10), (&a.id, 1)] {
        std::fs::create_dir_all(root.join(id).join(format!(".chunks/recording-{number}"))).unwrap();
    }
    std::fs::create_dir_all(root.join(&a.id).join(".chunks/recording-nope")).unwrap();
    std::fs::create_dir_all(root.join(".hidden/.chunks/recording-3")).unwrap();
    assert_eq!(
        library.orphan_recordings().unwrap(),
        vec![
            OrphanRecording {
                meeting_id: a.id.clone(),
                number: 1
            },
            OrphanRecording {
                meeting_id: a.id,
                number: 10
            },
            OrphanRecording {
                meeting_id: b.id,
                number: 2
            },
        ]
    );
}

#[test]
fn begin_next_recording_refuses_an_unsaved_recording() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let (meeting, _, _) = library.begin_recording(at("2026-10-12T09:30:00Z")).unwrap();
    assert!(matches!(
        library.begin_next_recording(&meeting.id, at("2026-10-12T10:00:00Z")),
        Err(LibraryError::UnsavedRecording)
    ));
}

#[test]
fn rename_moves_the_folder_preserves_files_and_metadata_and_updates_the_index() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let folder = root.join(&original.id);
    std::fs::write(folder.join("notes.txt"), b"keep").unwrap();
    let metadata_path = folder.join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    metadata["future"] = serde_json::json!({"value": 42});
    std::fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();

    let renamed = library
        .rename_meeting(&original.id, "  Project review  ")
        .unwrap();

    assert_eq!(renamed.id, "2026-10-12 0930 Project review");
    assert_eq!(renamed.title, "Project review");
    assert!(!folder.exists());
    let new_folder = root.join(&renamed.id);
    assert_eq!(
        std::fs::read(new_folder.join("notes.txt")).unwrap(),
        b"keep"
    );
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(new_folder.join("meeting.json")).unwrap()).unwrap();
    assert_eq!(saved["future"], metadata["future"]);
    assert_eq!(saved["title"], "Project review");
    assert_eq!(library.list_meetings().unwrap(), vec![renamed]);
}

#[test]
fn rename_refreshes_derived_markdown_for_current_revision() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let folder = library.meeting_dir(&original.id);
    let revision = serde_json::json!({
        "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
        "model": "small", "language": "en", "language_detected": false,
        "duration_ms": 1000, "segments": [{"id": "s00001", "start_ms": 0, "end_ms": 1000, "text": "Hello", "speaker": "me"}]
    });
    std::fs::write(
        folder.join("transcript.r1.json"),
        serde_json::to_vec(&revision).unwrap(),
    )
    .unwrap();
    std::fs::write(folder.join("transcript.md"), "# Planning\n\nOld content").unwrap();
    let renamed = library.rename_meeting(&original.id, "Review").unwrap();
    let markdown =
        std::fs::read_to_string(library.meeting_dir(&renamed.id).join("transcript.md")).unwrap();
    assert!(markdown.starts_with("# Review\n"));
    assert!(markdown.contains("Hello"));
    assert_eq!(
        std::fs::read(library.meeting_dir(&renamed.id).join("transcript.r1.json")).unwrap(),
        serde_json::to_vec(&revision).unwrap()
    );
}

#[test]
fn rename_to_taken_name_uses_suffix_without_touching_the_other_meeting() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let taken = library
        .create_meeting("Review", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let renamed = library.rename_meeting(&original.id, "Review").unwrap();
    assert_eq!(renamed.id, "2026-10-12 0930 Review (2)");
    assert_eq!(library.meeting(&taken.id).unwrap().title, "Review");
    assert_eq!(library.list_meetings().unwrap().len(), 2);
}

#[test]
fn rename_to_same_title_keeps_folder_and_empty_title_is_rejected() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    assert_eq!(
        library.rename_meeting(&original.id, " Planning ").unwrap(),
        original
    );
    assert!(matches!(
        library.rename_meeting(&original.id, "  "),
        Err(LibraryError::EmptyTitle)
    ));
    assert!(root.join(&original.id).exists());
    assert_eq!(library.list_meetings().unwrap(), vec![original]);
}

#[test]
fn case_only_rename_changes_the_folder_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();

    let renamed = library.rename_meeting(&original.id, "PLANNING").unwrap();

    assert_eq!(renamed.id, "2026-10-12 0930 PLANNING");
    assert_eq!(renamed.title, "PLANNING");
    // List real names: on Windows `root.join(&original.id)` still resolves to the renamed folder.
    let folders: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(folders, vec![renamed.id.clone()]);
    assert!(root.join(&renamed.id).join("meeting.json").exists());
    assert_eq!(library.list_meetings().unwrap(), vec![renamed]);
}

#[test]
fn rename_refuses_unsaved_recordings_and_missing_or_invalid_ids() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    std::fs::create_dir_all(root.join(&original.id).join(".chunks/recording-1")).unwrap();
    assert!(matches!(
        library.rename_meeting(&original.id, "New title"),
        Err(LibraryError::UnsavedRecording)
    ));
    assert!(matches!(
        library.rename_meeting("missing", "New title"),
        Err(LibraryError::MeetingNotFound)
    ));
    assert!(matches!(
        library.rename_meeting("..", "New title"),
        Err(LibraryError::MeetingNotFound)
    ));
    assert!(matches!(
        library.meeting(".."),
        Err(LibraryError::MeetingNotFound)
    ));
    assert!(root.join(&original.id).exists());
    assert_eq!(library.list_meetings().unwrap(), vec![original]);
}

#[test]
fn meeting_detail_reports_markers_language_and_only_existing_audio() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let summary = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let folder = root.join(&summary.id);
    let metadata_path = folder.join("meeting.json");
    let mut metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata_path).unwrap()).unwrap();
    metadata["language"] = serde_json::json!("vi");
    metadata["duration_ms"] = serde_json::json!(125_000);
    metadata["audio"] = serde_json::json!("meeting.mp3");
    metadata["markers"] = serde_json::json!([
        {"kind": "pause", "at_ms": 1_000, "paused_ms": 5_000},
        {"kind": "gap", "at_ms": 20_000},
        {"kind": "device_switch", "at_ms": 40_000, "source": "microphone", "device": "Headset"}
    ]);
    std::fs::write(&metadata_path, serde_json::to_vec(&metadata).unwrap()).unwrap();
    let without_audio = library.meeting(&summary.id).unwrap();
    assert_eq!(without_audio.language.as_deref(), Some("vi"));
    assert_eq!(without_audio.duration_ms, 125_000);
    assert_eq!(without_audio.markers.len(), 3);
    assert_eq!(without_audio.audio_path, None);
    let audio = folder.join("meeting.mp3");
    std::fs::write(&audio, b"mp3").unwrap();
    assert_eq!(
        library.meeting(&summary.id).unwrap().audio_path,
        Some(audio)
    );
}

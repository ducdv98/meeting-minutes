use chrono::{DateTime, FixedOffset};
use meeting_minutes_core::ai::ProviderKind;
use meeting_minutes_core::{Library, LibraryError, OrphanRecording};
use meeting_minutes_core::transcript::Speaker;
use tempfile::TempDir;

fn write_transcript(library: &Library, id: &str, revision: u32, text: &str) {
    let value = serde_json::json!({
        "schema_version": 1, "revision": revision, "created_at": "2026-10-12T10:00:00Z",
        "model": "small", "language": "en", "language_detected": false,
        "duration_ms": 2000, "segments": [{"id": "s00001", "start_ms": 100, "end_ms": 900, "text": text, "speaker": "me"}]
    });
    std::fs::write(
        library
            .meeting_dir(id)
            .join(format!("transcript.r{revision}.json")),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn search_tracks_participant_names_after_rename() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    write_transcript(&library, &meeting.id, 1, "Budget");
    library.refresh_transcript(&meeting.id).unwrap();
    library.set_participant_name(&meeting.id, Speaker::Me, Some("Anna")).unwrap();
    assert_eq!(library.search("Anna").unwrap()[0].meeting_id, meeting.id);
    library.rename_participant(&meeting.id, "Anna", "Bao").unwrap();
    assert!(library.search("Anna").unwrap().is_empty());
    assert_eq!(library.search("Bao").unwrap()[0].snippet, "Budget");
}

#[test]
fn settings_name_reindexes_every_meeting() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let first = library.create_meeting("First", at("2026-10-12T09:30:00Z")).unwrap();
    let second = library.create_meeting("Second", at("2026-10-13T09:30:00Z")).unwrap();
    for meeting in [&first, &second] { write_transcript(&library, &meeting.id, 1, "Budget"); library.refresh_transcript(&meeting.id).unwrap(); }
    library.set_your_name(Some("Duc")).unwrap();
    assert_eq!(library.search("Duc").unwrap().len(), 2);
    library.set_your_name(Some("Bao")).unwrap();
    assert!(library.search("Duc").unwrap().is_empty());
    assert_eq!(library.search("Bao").unwrap().len(), 2);
}

#[test]
fn a_fresh_index_opened_with_your_name_indexes_it() {
    let temp = TempDir::new().unwrap();
    let library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let meeting = library_meeting(library);
    std::fs::remove_file(temp.path().join("index.sqlite3")).unwrap();
    let library = Library::open_with_name(temp.path().join("Library"), temp.path().join("index.sqlite3"), Some("Duc")).unwrap();
    assert_eq!(library.search("Duc").unwrap()[0].meeting_id, meeting);
}

fn library_meeting(mut library: Library) -> String {
    let meeting = library.create_meeting("First", at("2026-10-12T09:30:00Z")).unwrap();
    write_transcript(&library, &meeting.id, 1, "Budget");
    meeting.id
}

#[test]
fn summary_participants_unchanged_without_its_revision() {
    let temp = TempDir::new().unwrap();
    let library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let id = library_meeting(library);
    let library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    std::fs::write(library.meeting_dir(&id).join("summary.json"), r#"{"schema_version":1,"revision":7,"created_at":"now","provider":"ollama","model":"m","overview":[],"key_points":[],"decisions":[],"action_items":[],"dropped_items":0}"#).unwrap();
    assert!(!library.summary_participants_changed(&id).unwrap());
}

#[test]
fn library_checks_summary_participants_for_saved_and_legacy_files() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    write_transcript(&library, &meeting.id, 1, "Budget");
    let folder = library.meeting_dir(&meeting.id);
    let transcript = library.transcript(&meeting.id).unwrap().unwrap();
    let participants = library.participants(&meeting.id).unwrap();
    let mut summary = serde_json::json!({"schema_version":1,"revision":1,"created_at":"now","provider":"ollama","model":"local","overview":[],"key_points":[],"decisions":[],"action_items":[],"dropped_items":0,
        "participants_fingerprint":meeting_minutes_core::summary::participants_fingerprint(&transcript, &participants)});
    std::fs::write(folder.join("summary.json"), serde_json::to_vec(&summary).unwrap()).unwrap();
    assert!(!library.summary_participants_changed(&meeting.id).unwrap());
    library.set_participant_name(&meeting.id, Speaker::Me, Some("Anna")).unwrap();
    assert!(library.summary_participants_changed(&meeting.id).unwrap());
    summary.as_object_mut().unwrap().remove("participants_fingerprint");
    std::fs::write(folder.join("summary.json"), serde_json::to_vec(&summary).unwrap()).unwrap();
    assert!(library.summary_participants_changed(&meeting.id).unwrap());
    library.set_participant_name(&meeting.id, Speaker::Me, None).unwrap();
    assert!(!library.summary_participants_changed(&meeting.id).unwrap());
}

#[test]
fn import_segments_can_be_moved_in_groups_and_return_to_unknown() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Import", at("2026-10-12T09:30:00Z")).unwrap();
    let folder = library.meeting_dir(&meeting.id);
    let transcript = serde_json::json!({"schema_version":1,"revision":1,"created_at":"now","model":"small","language":"en","language_detected":false,"duration_ms":3000,"segments":[
        {"id":"s00001","start_ms":0,"end_ms":800,"text":"First","speaker":"unknown"},
        {"id":"s00002","start_ms":900,"end_ms":1700,"text":"Second","speaker":"unknown"},
        {"id":"s00003","start_ms":1800,"end_ms":2600,"text":"Third","speaker":"unknown"}
    ]});
    std::fs::write(folder.join("transcript.r1.json"), serde_json::to_vec(&transcript).unwrap()).unwrap();
    library.assign_segments(&meeting.id, 1, &["s00001".into(), "s00003".into()], Some("Anna")).unwrap();
    library.assign_segments(&meeting.id, 1, &["s00002".into()], Some("Bao")).unwrap();
    assert_eq!(library.participant_names().unwrap(), vec!["Anna", "Bao"]);
    assert!(matches!(library.assign_segments(&meeting.id, 0, &["s00001".into()], Some("Bao")), Err(LibraryError::StaleRevision)));
    assert_eq!(library.meeting(&meeting.id).unwrap().segment_participants.unwrap().segments.len(), 3);
    let markdown = std::fs::read_to_string(folder.join("transcript.md")).unwrap();
    assert!(markdown.contains("Anna:** First"));
    assert!(markdown.contains("Bao:** Second"));
    assert!(markdown.contains("Anna:** Third"));
    library.remove_participant(&meeting.id, "Bao").unwrap();
    assert!(std::fs::read_to_string(folder.join("transcript.md")).unwrap().contains("Unknown:** Second"));
    library.assign_segments(&meeting.id, 1, &["s00003".into()], None).unwrap();
    assert!(std::fs::read_to_string(folder.join("transcript.md")).unwrap().contains("Unknown:** Third"));
    assert_eq!(library.meeting(&meeting.id).unwrap().segment_participants.unwrap().segments.len(), 1);
}

#[test]
fn rename_participant_merges_defaults_labels_and_segment_assignments() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    library.set_your_name(Some("Duc")).unwrap();
    let meeting = library.create_meeting("Call", at("2026-10-12T09:30:00Z")).unwrap();
    write_transcript(&library, &meeting.id, 1, "Hello");
    library.set_participant_name(&meeting.id, Speaker::Remote, Some("Anna")).unwrap();
    library.assign_segments(&meeting.id, 1, &["s00001".into()], Some("Bao")).unwrap();
    library.rename_participant(&meeting.id, "Bao", "Anna").unwrap();
    assert!(std::fs::read_to_string(library.meeting_dir(&meeting.id).join("transcript.md")).unwrap().contains("Anna:** Hello"));
    library.rename_participant(&meeting.id, "Duc", "Anna").unwrap();
    assert_eq!(library.meeting(&meeting.id).unwrap().participants.get("me").map(String::as_str), Some("Anna"));
    library.remove_participant(&meeting.id, "Anna").unwrap();
    assert!(std::fs::read_to_string(library.meeting_dir(&meeting.id).join("transcript.md")).unwrap().contains("Duc:** Hello"));
}

#[test]
fn participant_names_survive_rescan_and_update_transcript_markdown() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let index = temp.path().join("index.sqlite3");
    let mut library = Library::open(&root, &index).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    write_transcript(&library, &meeting.id, 1, "Hello");
    library.set_participant_name(&meeting.id, Speaker::Me, Some("Duc")).unwrap();
    assert_eq!(library.meeting(&meeting.id).unwrap().participants.get("me").map(String::as_str), Some("Duc"));
    assert!(std::fs::read_to_string(library.meeting_dir(&meeting.id).join("transcript.md")).unwrap().contains("Duc:** Hello"));
    drop(library);
    let mut reopened = Library::open(&root, &index).unwrap();
    reopened.rescan().unwrap();
    assert_eq!(reopened.meeting(&meeting.id).unwrap().participants.get("me").map(String::as_str), Some("Duc"));
    assert_eq!(reopened.participant_names().unwrap(), vec!["Duc"]);
}

#[test]
fn participant_defaults_merging_and_voice_notes_use_meeting_sources() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    library.set_your_name(Some("Duc")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    let path = library.meeting_dir(&meeting.id);
    let transcript = serde_json::json!({"schema_version":1,"revision":1,"created_at":"2026-10-12T10:00:00Z","model":"small","language":"en","language_detected":false,"duration_ms":3000,"segments":[
        {"id":"s00001","start_ms":0,"end_ms":500,"text":"Hello","speaker":"me"},
        {"id":"s00002","start_ms":600,"end_ms":1000,"text":"Hi","speaker":"remote"},
        {"id":"s00003","start_ms":1100,"end_ms":1500,"text":"Maybe","speaker":"unknown"}
    ]});
    std::fs::write(path.join("transcript.r1.json"), serde_json::to_vec(&transcript).unwrap()).unwrap();
    library.set_participant_name(&meeting.id, Speaker::Remote, Some("Anna")).unwrap();
    library.set_participant_name(&meeting.id, Speaker::Unknown, Some("Anna")).unwrap();
    let markdown = std::fs::read_to_string(path.join("transcript.md")).unwrap();
    assert!(markdown.contains("Duc:** Hello"));
    assert!(markdown.contains("Anna:** Hi"));
    assert!(markdown.contains("Anna:** Maybe"));
    library.set_participant_name(&meeting.id, Speaker::Remote, Some("Remote")).unwrap();
    assert!(!library.meeting(&meeting.id).unwrap().participants.contains_key("remote"));
    library.set_participant_name(&meeting.id, Speaker::Me, Some("Duc")).unwrap();
    assert!(!library.meeting(&meeting.id).unwrap().participants.contains_key("me"));
    let mut metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(path.join("meeting.json")).unwrap()).unwrap();
    metadata["recordings"] = serde_json::json!([{"number":1,"started_at":"2026-10-12T09:30:00Z","timeline_start_ms":0,"duration_ms":3000,"files":[{"source":"microphone","file":"mic.flac","channels":1}]}]);
    assert!(!library.meeting(&meeting.id).unwrap().voice_note);
    std::fs::write(path.join("meeting.json"), serde_json::to_vec(&metadata).unwrap()).unwrap();
    assert!(library.meeting(&meeting.id).unwrap().voice_note);
    library.set_participant_name(&meeting.id, Speaker::Me, Some("Duc Voice")).unwrap();
    let markdown = std::fs::read_to_string(path.join("transcript.md")).unwrap();
    assert!(markdown.contains("Duc Voice:** Maybe"));
    assert!(!markdown.contains("Anna:** Maybe"));
}

#[test]
fn search_finds_current_segments_across_meetings_and_survives_rebuild() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let index = temp.path().join("index.sqlite3");
    let mut library = Library::open(&root, &index).unwrap();
    let first = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    let second = library
        .create_meeting("Review", at("2026-10-13T09:30:00Z"))
        .unwrap();
    write_transcript(&library, &first.id, 1, "Budget for người");
    write_transcript(&library, &second.id, 1, "Budget approved");
    library.rescan().unwrap();
    let hits = library.search("budg").unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|hit| hit.revision == 1));
    assert!(hits
        .iter()
        .any(|hit| hit.meeting_id == first.id && hit.snippet.contains("\u{2}Budget\u{3}")));
    assert!(hits.iter().any(|hit| hit.meeting_id == second.id));
    assert_eq!(library.search("nguoi").unwrap()[0].meeting_id, first.id);
    assert!(library.search("\"*-:").unwrap().is_empty());
    assert!(library.search("").unwrap().is_empty());
    assert!(library.search("NEAR(").is_ok());
    drop(library);
    std::fs::remove_file(&index).unwrap();
    let reopened = Library::open(&root, &index).unwrap();
    assert_eq!(reopened.search("budget").unwrap().len(), 2);
}

#[test]
fn search_keeps_a_second_meeting_visible_after_many_matches_in_one() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(temp.path().join("Library"), temp.path().join("index.sqlite3")).unwrap();
    let first = library.create_meeting("First", at("2026-10-12T09:30:00Z")).unwrap();
    let second = library.create_meeting("Second", at("2026-10-13T09:30:00Z")).unwrap();
    let transcript = serde_json::json!({
        "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
        "model": "small", "language": "en", "language_detected": false,
        "duration_ms": 60000,
        "segments": (1..=60).map(|i| serde_json::json!({"id": format!("s{i:05}"), "start_ms": i * 1000, "end_ms": i * 1000 + 500, "text": "budget", "speaker": "me"})).collect::<Vec<_>>()
    });
    std::fs::write(library.meeting_dir(&first.id).join("transcript.r1.json"), serde_json::to_vec(&transcript).unwrap()).unwrap();
    write_transcript(&library, &second.id, 1, "budget");
    library.rescan().unwrap();
    let hits = library.search("budget").unwrap();
    assert!(hits.iter().any(|hit| hit.meeting_id == first.id));
    assert!(hits.iter().any(|hit| hit.meeting_id == second.id));
    assert!(hits.iter().filter(|hit| hit.meeting_id == first.id).count() <= 5);
}

#[test]
fn search_tracks_revisions_edits_and_renames() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    write_transcript(&library, &meeting.id, 1, "alpha");
    library.refresh_transcript(&meeting.id).unwrap();
    assert_eq!(library.search("alpha").unwrap().len(), 1);
    write_transcript(&library, &meeting.id, 2, "beta");
    library.refresh_transcript(&meeting.id).unwrap();
    assert!(library.search("alpha").unwrap().is_empty());
    library
        .edit_segment(&meeting.id, 2, "s00001", "gamma")
        .unwrap();
    assert!(library.search("beta").unwrap().is_empty());
    let renamed = library.rename_meeting(&meeting.id, "Review").unwrap();
    assert_eq!(library.search("gamma").unwrap()[0].meeting_id, renamed.id);
    assert_eq!(library.search("gamma").unwrap()[0].revision, 2);
}

#[test]
fn delete_moves_meeting_then_restore_and_rescan_rebuilds_search() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    write_transcript(&library, &meeting.id, 1, "budget");
    library.refresh_transcript(&meeting.id).unwrap();
    assert!(library.delete_meeting("missing", &|_| Ok(())).is_err());
    let original = library.meeting_dir(&meeting.id);
    std::fs::create_dir(original.join(".chunks")).unwrap();
    std::fs::create_dir(original.join(".chunks/recording-1")).unwrap();
    assert!(matches!(
        library.delete_meeting(&meeting.id, &|_| Ok(())),
        Err(LibraryError::UnsavedRecording)
    ));
    std::fs::remove_dir(original.join(".chunks/recording-1")).unwrap();
    assert!(library
        .delete_meeting(&meeting.id, &|_| Err(std::io::Error::other("failed")))
        .is_err());
    assert_eq!(library.search("budget").unwrap().len(), 1);
    library
        .delete_meeting(&meeting.id, &|path| {
            std::fs::rename(path, bin.join(&meeting.id))
        })
        .unwrap();
    assert!(!original.exists());
    assert!(library.list_meetings().unwrap().is_empty());
    assert!(library.search("budget").unwrap().is_empty());
    std::fs::rename(bin.join(&meeting.id), &original).unwrap();
    library.rescan().unwrap();
    assert_eq!(library.search("budget").unwrap().len(), 1);
}

#[test]
fn version_two_index_upgrades_and_indexes_current_transcripts() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let index = temp.path().join("index.sqlite3");
    let mut library = Library::open(&root, &index).unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    write_transcript(&library, &meeting.id, 1, "budget");
    drop(library);
    let connection = rusqlite::Connection::open(&index).unwrap();
    connection
        .execute_batch("DROP TABLE segments; PRAGMA user_version = 2;")
        .unwrap();
    drop(connection);
    let reopened = Library::open(&root, &index).unwrap();
    assert_eq!(reopened.search("budget").unwrap().len(), 1);
}

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

    assert_eq!(meeting.id, "2026-10-12 0930");
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
    let occupied = root.join("2026-10-12 0930");
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

    assert_eq!(first.id, "2026-10-12 0930 (2)");
    assert_eq!(second.id, "2026-10-12 0930 (3)");
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
        .create_meeting("Review", at("2026-10-12T09:30:59Z"))
        .unwrap();
    assert_eq!(first.id, "2026-10-12 0930");
    assert_eq!(second.id, "2026-10-12 0930 (2)");
}

#[test]
fn an_occupied_folder_without_metadata_is_preserved() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let occupied = root.join("2026-10-12 0930");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("notes.txt"), b"private notes").unwrap();
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    assert_eq!(meeting.id, "2026-10-12 0930 (2)");
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
            ("2026-10-12 0930", "Updated planning")
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
            "2026-10-12 1001",
            "2026-10-12 1000 (2)",
            "2026-10-12 1000 (3)",
            "2026-10-12 1000",
            "2026-10-12 1100"
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
    assert_eq!(meeting.id, "2026-10-12 0930");
    assert!(matches!(
        library.begin_next_recording(&meeting.id, at("2026-10-12T10:00:00Z")),
        Err(LibraryError::UnsavedRecording)
    ));
}

#[test]
fn rename_keeps_the_folder_preserves_files_and_metadata_and_updates_the_index() {
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

    assert_eq!(renamed.id, original.id);
    assert_eq!(renamed.title, "Project review");
    assert!(folder.exists());
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
fn legacy_title_folder_loads_and_renames_in_place() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    let legacy_id = "2026-10-12 0930 Planning";
    std::fs::rename(root.join(&meeting.id), root.join(legacy_id)).unwrap();
    library.rescan().unwrap();
    let renamed = library.rename_meeting(legacy_id, "Review").unwrap();
    assert_eq!(renamed.id, legacy_id);
    assert_eq!(library.meeting(legacy_id).unwrap().title, "Review");
    assert!(root.join(legacy_id).join("meeting.json").is_file());
}

#[test]
fn summary_finished_after_rename_uses_the_current_title() {
    use meeting_minutes_core::summary::{write_summary_for_meeting, Summary};

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    let summary: Summary = serde_json::from_value(serde_json::json!({
        "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
        "provider": "ollama", "model": "fake", "overview": [], "key_points": [],
        "decisions": [], "action_items": [], "dropped_items": 0
    })).unwrap();
    library.rename_meeting(&meeting.id, "Review").unwrap();
    write_summary_for_meeting(&root.join(&meeting.id), &summary).unwrap();
    assert!(std::fs::read_to_string(root.join(&meeting.id).join("summary.md")).unwrap().starts_with("# Review — Summary\n"));
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
    let summary = serde_json::json!({
        "schema_version": 1, "revision": 1, "created_at": "2026-10-12T10:00:00Z",
        "provider": "ollama", "model": "fake", "overview": [], "key_points": [],
        "decisions": [], "action_items": [], "dropped_items": 0
    });
    std::fs::write(folder.join("summary.json"), serde_json::to_vec(&summary).unwrap()).unwrap();
    std::fs::write(folder.join("summary.md"), "# Planning — Summary\n\nOld content").unwrap();
    let renamed = library.rename_meeting(&original.id, "Review").unwrap();
    let markdown =
        std::fs::read_to_string(library.meeting_dir(&renamed.id).join("transcript.md")).unwrap();
    assert!(markdown.starts_with("# Review\n"));
    assert!(markdown.contains("Hello"));
    let summary_markdown = std::fs::read_to_string(library.meeting_dir(&renamed.id).join("summary.md")).unwrap();
    assert!(summary_markdown.starts_with("# Review — Summary\n"));
    assert_eq!(
        std::fs::read(library.meeting_dir(&renamed.id).join("transcript.r1.json")).unwrap(),
        serde_json::to_vec(&revision).unwrap()
    );
}

#[test]
fn rename_does_not_change_colliding_ids_or_the_other_meeting() {
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
    assert_eq!(renamed.id, original.id);
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
fn case_only_rename_keeps_the_folder_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();

    let renamed = library.rename_meeting(&original.id, "PLANNING").unwrap();

    assert_eq!(renamed.id, original.id);
    assert_eq!(renamed.title, "PLANNING");
    let folders: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(folders, vec![renamed.id.clone()]);
    assert!(root.join(&renamed.id).join("meeting.json").exists());
    assert_eq!(library.list_meetings().unwrap(), vec![renamed]);
}

#[test]
fn rename_accepts_unsaved_recordings_and_refuses_missing_or_invalid_ids() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let original = library
        .create_meeting("Planning", at("2026-10-12T09:30:00Z"))
        .unwrap();
    std::fs::create_dir_all(root.join(&original.id).join(".chunks/recording-1")).unwrap();
    assert_eq!(library.rename_meeting(&original.id, "New title").unwrap().id, original.id);
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
    assert_eq!(library.list_meetings().unwrap()[0].title, "New title");
}

#[test]
fn concurrent_title_and_language_writes_both_survive() {
    use std::sync::{Arc, Barrier};

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("Library");
    let mut library = Library::open(&root, temp.path().join("index.sqlite3")).unwrap();
    let meeting = library.create_meeting("Planning", at("2026-10-12T09:30:00Z")).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let rename_index = temp.path().join("rename-index.sqlite3");
    std::thread::scope(|scope| {
        let root = root.clone();
        let id = meeting.id.clone();
        let thread_barrier = barrier.clone();
        scope.spawn(move || {
            let mut writer = Library::open(root, rename_index).unwrap();
            for number in 0..32 {
                thread_barrier.wait();
                writer.rename_meeting(&id, &format!("Review {number}")).unwrap();
                thread_barrier.wait();
            }
        });
        for number in 0..32 {
            barrier.wait();
            let language = if number % 2 == 0 { meeting_minutes_core::transcript::Language::En } else { meeting_minutes_core::transcript::Language::Vi };
            library.set_language(&meeting.id, Some(language)).unwrap();
            barrier.wait();
        }
    });
    assert_eq!(library.meeting(&meeting.id).unwrap().title, "Review 31");
    let metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join(&meeting.id).join("meeting.json")).unwrap()
    ).unwrap();
    assert_eq!(metadata["language"], "vi");
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

#[test]
fn search_folds_accents_and_non_decomposing_letters() {
    let temp = TempDir::new().unwrap();
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .unwrap();
    let meeting = library
        .create_meeting("Languages", at("2026-10-12T09:30:00Z"))
        .unwrap();
    write_transcript(
        &library,
        &meeting.id,
        1,
        "Español français português Straße Łódź Đà Nẵng",
    );
    library.rescan().unwrap();
    for query in [
        "Espanol",
        "francais",
        "portugues",
        "Strasse",
        "Lodz",
        "Da",
        "Đà",
    ] {
        assert_eq!(
            library.search(query).unwrap()[0].meeting_id,
            meeting.id,
            "{query}"
        );
    }
    assert_eq!(
        library.search("franc\u{327}ais").unwrap()[0].meeting_id,
        meeting.id
    );
}

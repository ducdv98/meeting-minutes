use chrono::{DateTime, FixedOffset};
use meeting_minutes_core::Library;
use tempfile::TempDir;

fn at(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).unwrap()
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
            "language": null, "recordings": [], "markers": []
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

use meeting_minutes_core::participant::{MeetingParticipants, Participants};
use meeting_minutes_core::transcript::{
    assign_ids, place_segments, render_markdown, segment_sentences, Language, PlacedSegment,
    RawSegment, RawWord, Speaker, Transcript, MAX_SEGMENT_MS, MAX_WORD_MS, MIN_SEGMENT_WORDS,
};
use meeting_minutes_core::Library;
use tempfile::TempDir;

#[test]
fn old_meeting_transcript_model_loads_as_one_engine() {
    let old = r#"{"schema_version":1,"revision":1,"created_at":"2025-01-01","model":"small","language":"en","language_detected":false,"duration_ms":0,"segments":[]}"#;
    let meeting_dir = TempDir::new().unwrap();
    let path = meeting_dir.path().join("transcript.r1.json");
    std::fs::write(&path, old).unwrap();
    let transcript: Transcript = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(transcript.model, vec!["small".into()]);
    assert!(serde_json::to_string(&transcript)
        .unwrap()
        .contains("\"model\":[\"small\"]"));
}

#[test]
fn sentence_punctuation_splits_one_source_at_word_timestamps() {
    let raw = RawSegment {
        start_ms: 0,
        end_ms: 4000,
        text: "Hello my friend. How are you?".into(),
        words: vec![
            RawWord {
                start_ms: 0,
                end_ms: 300,
                text: "Hello".into(),
            },
            RawWord {
                start_ms: 350,
                end_ms: 650,
                text: "my".into(),
            },
            RawWord {
                start_ms: 700,
                end_ms: 1300,
                text: "friend.".into(),
            },
            RawWord {
                start_ms: 1700,
                end_ms: 2300,
                text: "How".into(),
            },
            RawWord {
                start_ms: 2400,
                end_ms: 3000,
                text: "are".into(),
            },
            RawWord {
                start_ms: 3100,
                end_ms: 4000,
                text: "you?".into(),
            },
        ],
    };
    let segments = segment_sentences(place_segments(&[raw], 4000, 0, Speaker::Me));
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 1300, "Hello my friend."), (1700, 4000, "How are you?")]
    );
}

#[test]
fn long_pause_separates_sentences_even_without_punctuation() {
    let raw = RawSegment {
        start_ms: 0,
        end_ms: 6400,
        text: "First thought then another thought".into(),
        words: vec![
            RawWord {
                start_ms: 0,
                end_ms: 500,
                text: "First".into(),
            },
            RawWord {
                start_ms: 550,
                end_ms: 1400,
                text: "thought".into(),
            },
            RawWord {
                start_ms: 4000,
                end_ms: 4600,
                text: "then".into(),
            },
            RawWord {
                start_ms: 4700,
                end_ms: 5300,
                text: "another".into(),
            },
            RawWord {
                start_ms: 5400,
                end_ms: 6400,
                text: "thought".into(),
            },
        ],
    };
    let segments = segment_sentences(place_segments(&[raw], 6400, 0, Speaker::Me));
    assert_eq!(
        segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
        vec!["First thought", "then another thought"]
    );
}

#[test]
fn short_fragment_joins_the_closest_neighbour_of_its_source() {
    let raw = RawSegment {
        start_ms: 0,
        end_ms: 4000,
        text: "We need a plan. Yes. We can start tomorrow.".into(),
        words: vec![
            RawWord {
                start_ms: 0,
                end_ms: 400,
                text: "We".into(),
            },
            RawWord {
                start_ms: 450,
                end_ms: 800,
                text: "need".into(),
            },
            RawWord {
                start_ms: 850,
                end_ms: 1100,
                text: "a".into(),
            },
            RawWord {
                start_ms: 1150,
                end_ms: 1600,
                text: "plan.".into(),
            },
            RawWord {
                start_ms: 1800,
                end_ms: 2050,
                text: "Yes.".into(),
            },
            RawWord {
                start_ms: 2200,
                end_ms: 2600,
                text: "We".into(),
            },
            RawWord {
                start_ms: 2700,
                end_ms: 3000,
                text: "can".into(),
            },
            RawWord {
                start_ms: 3100,
                end_ms: 3500,
                text: "start".into(),
            },
            RawWord {
                start_ms: 3600,
                end_ms: 4000,
                text: "tomorrow.".into(),
            },
        ],
    };
    let segments = segment_sentences(place_segments(&[raw], 4000, 0, Speaker::Me));
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (0, "We need a plan."),
            (1800, "Yes. We can start tomorrow.")
        ]
    );
}

#[test]
fn overlong_sentence_splits_at_its_longest_internal_pause() {
    let raw = RawSegment {
        start_ms: 0,
        end_ms: 24_000,
        text: "One two three four five".into(),
        words: vec![
            RawWord {
                start_ms: 0,
                end_ms: 4000,
                text: "One".into(),
            },
            RawWord {
                start_ms: 5000,
                end_ms: 9000,
                text: "two".into(),
            },
            RawWord {
                start_ms: 10_500,
                end_ms: 14_000,
                text: "three".into(),
            },
            RawWord {
                start_ms: 15_000,
                end_ms: 19_000,
                text: "four".into(),
            },
            RawWord {
                start_ms: 20_000,
                end_ms: 24_000,
                text: "five".into(),
            },
        ],
    };
    let segments = segment_sentences(place_segments(&[raw], 24_000, 0, Speaker::Me));
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 9000, "One two"), (10_500, 24_000, "three four five")]
    );
}

#[test]
fn long_splits_repeat_without_losing_words() {
    let text = (0..12)
        .map(|index| format!("word{index}"))
        .collect::<Vec<_>>();
    let raw = RawSegment {
        start_ms: 0,
        end_ms: 60_000,
        text: text.join(" "),
        words: text
            .iter()
            .enumerate()
            .map(|(index, text)| RawWord {
                start_ms: index as u64 * 5_000,
                end_ms: (index + 1) as u64 * 5_000,
                text: text.clone(),
            })
            .collect(),
    };
    let segments = segment_sentences(place_segments(&[raw], 60_000, 0, Speaker::Me));
    assert!(segments.len() > 2);
    assert!(segments
        .iter()
        .all(|segment| segment.end_ms - segment.start_ms <= 20_000));
    assert_eq!(
        segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        text.join(" ")
    );
}

#[test]
fn sentence_can_cross_engine_output_boundaries_but_not_sources() {
    let first = RawSegment {
        start_ms: 0,
        end_ms: 500,
        text: "The".into(),
        words: vec![RawWord {
            start_ms: 0,
            end_ms: 500,
            text: "The".into(),
        }],
    };
    let second = RawSegment {
        start_ms: 500,
        end_ms: 1200,
        text: "plan.".into(),
        words: vec![RawWord {
            start_ms: 500,
            end_ms: 1200,
            text: "plan.".into(),
        }],
    };
    let remote = RawSegment {
        start_ms: 300,
        end_ms: 800,
        text: "Okay.".into(),
        words: vec![RawWord {
            start_ms: 300,
            end_ms: 800,
            text: "Okay.".into(),
        }],
    };
    let mut placed = place_segments(&[first, second], 1200, 0, Speaker::Me);
    placed.extend(place_segments(&[remote], 1200, 0, Speaker::Remote));
    let segments = segment_sentences(placed);
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.speaker, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(Speaker::Me, "The plan."), (Speaker::Remote, "Okay.")]
    );
}

#[test]
fn invalid_word_timing_keeps_the_engine_text() {
    let raw = RawSegment {
        start_ms: 100,
        end_ms: 1100,
        text: "Keep this text.".into(),
        words: vec![RawWord {
            start_ms: 100,
            end_ms: 100,
            text: "Keep".into(),
        }],
    };
    let segments = segment_sentences(place_segments(&[raw], 1100, 0, Speaker::Unknown));
    assert_eq!(segments[0].text, "Keep this text.");
}

#[test]
fn sentence_segmenter_keeps_segments_without_word_timings() {
    let placed = vec![PlacedSegment {
        start_ms: 100,
        end_ms: 3100,
        text: "First complete sentence.".into(),
        speaker: Speaker::Remote,
        words: vec![],
    }];
    let segments = segment_sentences(placed);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].start_ms, 100);
    assert_eq!(segments[0].end_ms, 3100);
    assert_eq!(segments[0].text, "First complete sentence.");
}

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
                words: vec![],
            },
            RawSegment {
                start_ms: 0,
                end_ms: 300,
                text: " [BLANK_AUDIO] ".into(),
                words: vec![],
            },
            RawSegment {
                start_ms: 0,
                end_ms: 200,
                text: "  ".into(),
                words: vec![],
            },
            RawSegment {
                start_ms: 1000,
                end_ms: 1000,
                text: "zero".into(),
                words: vec![],
            },
            RawSegment {
                start_ms: 100,
                end_ms: 500,
                text: " first ".into(),
                words: vec![],
            },
            RawSegment {
                start_ms: 300,
                end_ms: 350,
                text: "[hello] text [more]".into(),
                words: vec![],
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
                words: vec![],
            },
            PlacedSegment {
                start_ms: 0,
                end_ms: 500,
                text: "first".into(),
                speaker: Speaker::Me,
                words: vec![],
            },
            PlacedSegment {
                start_ms: 2100,
                end_ms: 2200,
                text: "outside".into(),
                speaker: Speaker::Me,
                words: vec![],
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
        model: vec!["small".into()],
        fallback_reason: None,
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

fn timed_words(words: &[(u64, u64, &str)]) -> RawSegment {
    RawSegment {
        start_ms: words[0].0,
        end_ms: words[words.len() - 1].1,
        text: words
            .iter()
            .map(|word| word.2)
            .collect::<Vec<_>>()
            .join(" "),
        words: words
            .iter()
            .map(|&(start_ms, end_ms, text)| RawWord {
                start_ms,
                end_ms,
                text: text.into(),
            })
            .collect(),
    }
}

#[test]
fn a_word_timed_across_a_long_silence_does_not_stretch_its_segment() {
    let raw = timed_words(&[
        (0, 400, "Police"),
        (500, 900, "said"),
        (1_000, 1_400, "he"),
        (1_500, 1_900, "appeared"),
        (2_000, 28_000, "today."),
    ]);
    let segments = segment_sentences(place_segments(&[raw], 30_000, 0, Speaker::Remote));
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        vec![(0, 2_000 + MAX_WORD_MS, "Police said he appeared today.")]
    );
}

#[test]
fn a_long_split_does_not_leave_a_fragment_too_short_to_stand_alone() {
    // Sixteen 1 s words with 0.5 s pauses, and the longest pause before the last word.
    let mut words = Vec::new();
    let mut at = 0;
    for index in 0..16u64 {
        if index == 15 {
            at += 1_000;
        }
        words.push((at, at + 1_000, "word"));
        at += 1_500;
    }
    let segments = segment_sentences(place_segments(&[timed_words(&words)], at, 0, Speaker::Me));
    assert!(segments.len() >= 2);
    for segment in &segments {
        assert!(segment.words.len() >= MIN_SEGMENT_WORDS, "{segments:?}");
        assert!(segment.end_ms - segment.start_ms <= MAX_SEGMENT_MS);
    }
}

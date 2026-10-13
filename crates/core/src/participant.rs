use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::transcript::{Segment, Speaker, Transcript};

pub type MeetingParticipants = BTreeMap<String, String>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentParticipants {
    pub revision: u32,
    pub segments: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct Participants {
    pub labels: MeetingParticipants,
    pub segments: Option<SegmentParticipants>,
    pub settings_name: Option<String>,
    pub voice_note: bool,
}

impl Participants {
    pub fn new(labels: MeetingParticipants, segments: Option<SegmentParticipants>, settings_name: Option<&str>, voice_note: bool) -> Self {
        Self { labels, segments, settings_name: settings_name.map(str::to_owned), voice_note }
    }

    pub fn label_name(&self, label: Speaker) -> String {
        let label = if self.voice_note { Speaker::Me } else { label };
        self.labels.get(label_key(label)).cloned().unwrap_or_else(|| default_name(label, self.settings_name.as_deref()).to_owned())
    }

    pub fn name(&self, segment: &Segment, revision: u32) -> String {
        // Every Segment of a Voice note belongs to the user.
        self.segments.as_ref().filter(|map| !self.voice_note && map.revision == revision)
            .and_then(|map| map.segments.get(&segment.id)).cloned()
            .unwrap_or_else(|| self.label_name(segment.speaker))
    }

    /// Every Participant in use, plus each present Speaker label's Participant even when all its Segments moved away.
    pub fn list(&self, transcript: &Transcript) -> Vec<String> {
        let mut names = Vec::new();
        for segment in &transcript.segments {
            for name in [self.name(segment, transcript.revision), self.label_name(segment.speaker)] {
                if !names.contains(&name) { names.push(name); }
            }
        }
        names
    }
}

pub fn label_key(label: Speaker) -> &'static str {
    match label {
        Speaker::Me => "me",
        Speaker::Remote => "remote",
        Speaker::Unknown => "unknown",
    }
}

pub fn default_name(label: Speaker, settings_name: Option<&str>) -> &str {
    match label {
        Speaker::Me => settings_name.map(str::trim).filter(|name| !name.is_empty()).unwrap_or("Me"),
        Speaker::Remote => "Remote",
        Speaker::Unknown => "Unknown",
    }
}

pub fn carry_over(previous: &Transcript, next: &Transcript, participants: &Participants) -> SegmentParticipants {
    let mut segments = BTreeMap::new();
    for new_segment in &next.segments {
        let duration = new_segment.end_ms.saturating_sub(new_segment.start_ms);
        if duration == 0 { continue; }
        // Only Segments of the same Speaker label count, and overlapping old Segments count their time once.
        let mut spans = BTreeMap::<String, Vec<(u64, u64)>>::new();
        for old_segment in previous.segments.iter().filter(|old| old.speaker == new_segment.speaker) {
            let (start, end) = (new_segment.start_ms.max(old_segment.start_ms), new_segment.end_ms.min(old_segment.end_ms));
            if end > start { spans.entry(participants.name(old_segment, previous.revision)).or_default().push((start, end)); }
        }
        let coverage: BTreeMap<String, u64> = spans.into_iter().map(|(name, spans)| (name, covered_ms(spans))).collect();
        if let Some(maximum) = coverage.values().copied().max() {
            if maximum > duration / 2 && coverage.values().filter(|&&ms| ms == maximum).count() == 1 {
                let name = coverage.into_iter().find(|(_, ms)| *ms == maximum).unwrap().0;
                if name != participants.label_name(new_segment.speaker) { segments.insert(new_segment.id.clone(), name); }
            }
        }
    }
    SegmentParticipants { revision: next.revision, segments }
}

fn covered_ms(mut spans: Vec<(u64, u64)>) -> u64 {
    spans.sort_unstable();
    let (mut total, mut reached) = (0, 0);
    for (start, end) in spans {
        let start = start.max(reached);
        if end > start { total += end - start; reached = end; }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Language;

    fn transcript(revision: u32, spans: &[(u64, u64, Speaker)]) -> Transcript {
        Transcript { schema_version: 1, revision, created_at: "now".into(), model: "small".into(), language: Language::En, language_detected: false, duration_ms: 5000,
            segments: spans.iter().enumerate().map(|(index, &(start_ms, end_ms, speaker))| Segment { id: format!("s{index:05}"), start_ms, end_ms, text: "words".into(), speaker }).collect() }
    }

    #[test]
    fn carries_only_clear_majorities_and_ignores_ties_gaps_and_no_overlap() {
        let old = transcript(1, &[(0, 700, Speaker::Unknown), (700, 1400, Speaker::Unknown)]);
        let mut participants = Participants::new(MeetingParticipants::new(), None, None, false);
        participants.segments = Some(SegmentParticipants { revision: 1, segments: [("s00000".into(), "Anna".into()), ("s00001".into(), "Bao".into())].into() });
        let new = transcript(2, &[(0, 600, Speaker::Unknown), (400, 1000, Speaker::Unknown), (1400, 1800, Speaker::Unknown), (0, 1400, Speaker::Unknown), (600, 2000, Speaker::Unknown)]);
        let carried = carry_over(&old, &new, &participants);
        assert_eq!(carried.revision, 2);
        assert_eq!(carried.segments.get("s00000").map(String::as_str), Some("Anna"));
        assert_eq!(carried.segments.get("s00001"), None);
        assert_eq!(carried.segments.get("s00002"), None);
        assert_eq!(carried.segments.get("s00003"), None);
        assert_eq!(carried.segments.get("s00004"), None);
    }

    #[test]
    fn carry_over_counts_overlapping_old_segments_once_and_only_the_same_speaker_label() {
        let old = transcript(1, &[(0, 30, Speaker::Unknown), (20, 50, Speaker::Unknown), (0, 100, Speaker::Remote)]);
        let mut participants = Participants::new(MeetingParticipants::new(), None, None, false);
        participants.segments = Some(SegmentParticipants { revision: 1, segments: [("s00000".into(), "Anna".into()), ("s00001".into(), "Anna".into()), ("s00002".into(), "Bao".into())].into() });
        let new = transcript(2, &[(0, 100, Speaker::Unknown), (0, 40, Speaker::Unknown)]);
        let carried = carry_over(&old, &new, &participants);
        assert_eq!(carried.segments.get("s00000"), None);
        assert_eq!(carried.segments.get("s00001").map(String::as_str), Some("Anna"));
    }

    #[test]
    fn voice_notes_ignore_moves_and_list_keeps_label_participants() {
        let transcript = transcript(1, &[(0, 100, Speaker::Unknown), (100, 200, Speaker::Remote)]);
        let moved = Some(SegmentParticipants { revision: 1, segments: [("s00000".into(), "Bao".into()), ("s00001".into(), "Bao".into())].into() });
        let voice = Participants::new(MeetingParticipants::new(), moved.clone(), Some("Duc"), true);
        assert_eq!(voice.name(&transcript.segments[0], 1), "Duc");
        let labels: MeetingParticipants = [("remote".into(), "Anna".into())].into();
        assert_eq!(Participants::new(labels, moved, None, false).list(&transcript), vec!["Bao", "Unknown", "Anna"]);
    }

    #[test]
    fn lists_resolved_names_in_first_appearance_order_for_current_revision() {
        let transcript = transcript(2, &[(0, 100, Speaker::Unknown), (100, 200, Speaker::Remote), (200, 300, Speaker::Unknown)]);
        let participants = Participants::new(MeetingParticipants::new(), Some(SegmentParticipants { revision: 2, segments: [("s00000".into(), "  Anna  ".into()), ("s00002".into(), "  Anna  ".into())].into() }), None, false);
        assert_eq!(participants.list(&transcript), vec!["  Anna  ", "Unknown", "Remote"]);
        assert_eq!(participants.name(&transcript.segments[0], 1), "Unknown");
    }
}

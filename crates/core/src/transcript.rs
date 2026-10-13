use serde::{Deserialize, Serialize};
use crate::participant::{MeetingParticipants, Participants};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Me,
    Remote,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    En,
    Vi,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Speaker,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    pub schema_version: u32,
    pub revision: u32,
    pub created_at: String,
    pub model: String,
    pub language: Language,
    pub language_detected: bool,
    pub duration_ms: u64,
    pub segments: Vec<Segment>,
}

impl Transcript {
    pub fn segment(&self, segment_id: &str) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|segment| segment.id == segment_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub struct Activity {
    pub microphone: Option<Vec<f32>>,
    pub system_audio: Option<Vec<f32>>,
}
pub const ACTIVITY_FRAME_MS: u64 = 20;
pub const ACTIVE_RMS: f32 = 0.01;
pub const ACTIVE_FRACTION: f32 = 0.15;
pub const ACTIVE_MIN_FRAMES: usize = 25;

pub fn speaker_label(activity: &Activity, start_ms: u64, end_ms: u64) -> Speaker {
    fn active(values: &Option<Vec<f32>>, start_ms: u64, end_ms: u64) -> bool {
        let Some(values) = values else { return false };
        let start = (start_ms / ACTIVITY_FRAME_MS) as usize;
        let end = end_ms.div_ceil(ACTIVITY_FRAME_MS) as usize;
        let frames = values
            .get(start.min(values.len())..end.min(values.len()))
            .unwrap_or(&[]);
        let active = frames.iter().filter(|&&rms| rms >= ACTIVE_RMS).count();
        !frames.is_empty()
            && (active >= ACTIVE_MIN_FRAMES
                || active as f32 / frames.len() as f32 >= ACTIVE_FRACTION)
    }
    if activity.microphone.is_none() || activity.system_audio.is_none() {
        return Speaker::Unknown;
    }
    match (
        active(&activity.microphone, start_ms, end_ms),
        active(&activity.system_audio, start_ms, end_ms),
    ) {
        (true, false) => Speaker::Me,
        (false, true) => Speaker::Remote,
        _ => Speaker::Unknown,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacedSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Speaker,
}

pub fn place_segments(
    raw: &[RawSegment],
    recording_ms: u64,
    timeline_start_ms: u64,
    activity: &Activity,
) -> Vec<PlacedSegment> {
    let mut result = Vec::new();
    for segment in raw {
        let text = segment
            .text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let square_tag = text
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .is_some_and(|inner| !inner.contains('[') && !inner.contains(']'));
        let round_tag = text
            .strip_prefix('(')
            .and_then(|inner| inner.strip_suffix(')'))
            .is_some_and(|inner| !inner.contains('(') && !inner.contains(')'));
        if text.is_empty() || square_tag || round_tag {
            continue;
        }
        let start = segment.start_ms.min(recording_ms);
        let end = segment.end_ms.min(recording_ms);
        if start >= end {
            continue;
        }
        result.push(PlacedSegment {
            start_ms: timeline_start_ms.saturating_add(start),
            end_ms: timeline_start_ms.saturating_add(end),
            text,
            speaker: speaker_label(activity, start, end),
        });
    }
    result.sort_by_key(|s| (s.start_ms, s.end_ms));
    result
}

pub fn assign_ids(mut placed: Vec<PlacedSegment>, duration_ms: u64) -> Vec<Segment> {
    placed.sort_by_key(|s| (s.start_ms, s.end_ms));
    placed
        .into_iter()
        .filter_map(|s| {
            let end = s.end_ms.min(duration_ms);
            (s.start_ms < end).then_some((s, end))
        })
        .enumerate()
        .map(|(i, (s, end))| Segment {
            id: format!("s{:05}", i + 1),
            start_ms: s.start_ms,
            end_ms: end,
            text: s.text,
            speaker: s.speaker,
        })
        .collect()
}

pub fn render_markdown(title: &str, transcript: &Transcript) -> String {
    render_markdown_named(title, transcript, &Participants::new(MeetingParticipants::new(), None, None, false))
}

pub fn render_markdown_named(title: &str, transcript: &Transcript, participants: &Participants) -> String {
    let language = match transcript.language {
        Language::En => "English",
        Language::Vi => "Vietnamese",
    };
    let mut result = format!(
        "# {title}\n\nTranscript revision {} · model {} · {language}\n",
        transcript.revision, transcript.model
    );
    for s in &transcript.segments {
        let seconds = s.start_ms / 1000;
        let speaker = participants.name(s, transcript.revision);
        result.push_str(&format!(
            "\n**[{:02}:{:02}:{:02}] {speaker}:** {}\n",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60,
            s.text
        ));
    }
    result
}

use crate::participant::{MeetingParticipants, Participants};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EngineName {
    Managed,
    Groq,
    Gemini,
    Local(String),
}

impl EngineName {
    pub fn label(&self) -> &str {
        match self {
            Self::Managed => "Managed transcription",
            Self::Groq => "Groq",
            Self::Gemini => "Gemini",
            Self::Local(model) => model,
        }
    }
}
impl From<&str> for EngineName {
    fn from(name: &str) -> Self {
        match name {
            "managed" | "Managed transcription" => Self::Managed,
            "Groq" => Self::Groq,
            "Gemini" => Self::Gemini,
            model => Self::Local(model.to_owned()),
        }
    }
}
impl From<String> for EngineName {
    fn from(name: String) -> Self {
        Self::from(name.as_str())
    }
}
impl fmt::Display for EngineName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
impl Serialize for EngineName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.label())
    }
}
impl<'de> Deserialize<'de> for EngineName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    ManagedAllowance,
    ManagedCapacity(String),
    ManagedWaitlisted,
    ManagedSuspended,
    Quota(EngineName),
    Unauthorized(EngineName),
    Offline(EngineName),
    RateLimit(EngineName),
    Other(EngineName),
    Legacy(String),
}
impl FallbackReason {
    fn from_saved(value: String) -> Self {
        if value == "Managed allowance exhausted" {
            return Self::ManagedAllowance;
        }
        if value == "Managed Account waitlisted" {
            return Self::ManagedWaitlisted;
        }
        if value == "Managed Account suspended" {
            return Self::ManagedSuspended;
        }
        if let Some(date) = value.strip_prefix("Managed capacity exhausted until ") {
            return Self::ManagedCapacity(date.to_owned());
        }
        if let Some(reason) = value.strip_prefix("managed ") {
            return match reason {
                "quota" => Self::Quota(EngineName::Managed),
                "unauthorized" => Self::Unauthorized(EngineName::Managed),
                "offline" => Self::Offline(EngineName::Managed),
                "rate_limit" => Self::RateLimit(EngineName::Managed),
                "other" => Self::Other(EngineName::Managed),
                _ => Self::Legacy(value),
            };
        }
        if let Some(reason) = value.strip_prefix("Managed transcription ") {
            return match reason {
                "is busy" => Self::Quota(EngineName::Managed),
                "was disconnected" => Self::Unauthorized(EngineName::Managed),
                "is unavailable" => Self::Offline(EngineName::Managed),
                _ => Self::Legacy(value),
            };
        }
        let Some((engine, reason)) = value.split_once(' ') else {
            return Self::Legacy(value);
        };
        let engine = EngineName::from(engine);
        match reason {
            "daily limit reached" => Self::Quota(engine),
            "API key rejected" => Self::Unauthorized(engine),
            "unavailable" => Self::Offline(engine),
            "rate limit reached" => Self::RateLimit(engine),
            "transcription failed" => Self::Other(engine),
            _ => Self::Legacy(value),
        }
    }
}
impl fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManagedAllowance => f.write_str("Managed allowance exhausted"),
            Self::ManagedCapacity(date) => write!(f, "Managed capacity exhausted until {date}"),
            Self::ManagedWaitlisted => f.write_str("Managed Account waitlisted"),
            Self::ManagedSuspended => f.write_str("Managed Account suspended"),
            Self::Quota(EngineName::Managed) | Self::RateLimit(EngineName::Managed) => {
                f.write_str("Managed transcription is busy")
            }
            Self::Unauthorized(EngineName::Managed) => {
                f.write_str("Managed transcription was disconnected")
            }
            Self::Offline(EngineName::Managed) | Self::Other(EngineName::Managed) => {
                f.write_str("Managed transcription is unavailable")
            }
            Self::Quota(engine) => write!(f, "{engine} daily limit reached"),
            Self::Unauthorized(engine) => write!(f, "{engine} API key rejected"),
            Self::Offline(engine) => write!(f, "{engine} unavailable"),
            Self::RateLimit(engine) => write!(f, "{engine} rate limit reached"),
            Self::Other(engine) => write!(f, "{engine} transcription failed"),
            Self::Legacy(value) => f.write_str(value),
        }
    }
}
impl Serialize for FallbackReason {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for FallbackReason {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::from_saved)
    }
}

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
    Es,
    Fr,
    De,
    Pt,
    It,
    Ja,
    Ko,
    Zh,
    Hi,
}

impl Language {
    pub const ALL: [Self; 11] = [
        Self::En,
        Self::Vi,
        Self::Es,
        Self::Fr,
        Self::De,
        Self::Pt,
        Self::It,
        Self::Ja,
        Self::Ko,
        Self::Zh,
        Self::Hi,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Vi => "vi",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Pt => "pt",
            Self::It => "it",
            Self::Ja => "ja",
            Self::Ko => "ko",
            Self::Zh => "zh",
            Self::Hi => "hi",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Vi => "Vietnamese",
            Self::Es => "Spanish",
            Self::Fr => "French",
            Self::De => "German",
            Self::Pt => "Portuguese",
            Self::It => "Italian",
            Self::Ja => "Japanese",
            Self::Ko => "Korean",
            Self::Zh => "Chinese",
            Self::Hi => "Hindi",
        }
    }

    /// The language a Meeting's `language` code names, if it is one the app transcribes.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|language| language.code() == code)
    }
}

#[cfg(test)]
mod language_tests {
    use super::Language;

    #[test]
    fn supported_meeting_languages_keep_their_serialized_codes() {
        for (code, name) in [
            ("en", "English"),
            ("vi", "Vietnamese"),
            ("es", "Spanish"),
            ("fr", "French"),
            ("de", "German"),
            ("pt", "Portuguese"),
            ("it", "Italian"),
            ("ja", "Japanese"),
            ("ko", "Korean"),
            ("zh", "Chinese"),
            ("hi", "Hindi"),
        ] {
            let language = Language::from_code(code).unwrap();
            assert_eq!(language.code(), code);
            assert_eq!(language.name(), name);
            assert_eq!(
                serde_json::to_string(&language).unwrap(),
                format!("\"{code}\"")
            );
        }
    }
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
    #[serde(deserialize_with = "deserialize_engines")]
    pub model: Vec<EngineName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<FallbackReason>,
    pub language: Language,
    pub language_detected: bool,
    pub duration_ms: u64,
    pub segments: Vec<Segment>,
}

fn deserialize_engines<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<EngineName>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Engines {
        Old(String),
        Current(Vec<String>),
    }
    Ok(match Engines::deserialize(deserializer)? {
        Engines::Old(name) => vec![EngineName::from(name)],
        Engines::Current(names) => names.into_iter().map(EngineName::from).collect(),
    })
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
    pub words: Vec<RawWord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub fn shift_segment_times(
    start_ms: &mut u64,
    end_ms: &mut u64,
    words: &mut [RawWord],
    offset_ms: u64,
) {
    *start_ms += offset_ms;
    *end_ms += offset_ms;
    for word in words {
        word.start_ms += offset_ms;
        word.end_ms += offset_ms;
    }
}

pub const MAX_VAD_GAP_MS: u64 = 2_000;
pub const MIN_SEGMENT_MS: u64 = 1_000;
pub const MIN_SEGMENT_WORDS: usize = 3;
pub const MAX_SEGMENT_MS: u64 = 20_000;
/// Whisper can time a sentence's last word across the silence after it; no spoken word is longer.
pub const MAX_WORD_MS: u64 = 5_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlacedSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Speaker,
    #[serde(skip)]
    pub words: Vec<RawWord>,
}

pub fn place_segments(
    raw: &[RawSegment],
    recording_ms: u64,
    timeline_start_ms: u64,
    speaker: Speaker,
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
        let mut words = segment
            .words
            .iter()
            .filter_map(|word| {
                let word_start = word.start_ms.clamp(start, end);
                let word_end = word.end_ms.clamp(start, end);
                (word_start < word_end && !word.text.trim().is_empty()).then(|| RawWord {
                    start_ms: timeline_start_ms.saturating_add(word_start),
                    end_ms: timeline_start_ms.saturating_add(word_end),
                    text: word.text.trim().to_owned(),
                })
            })
            .collect::<Vec<_>>();
        if words.is_empty() {
            let parts = text.split_whitespace().collect::<Vec<_>>();
            words = parts
                .iter()
                .enumerate()
                .map(|(index, word)| RawWord {
                    start_ms: timeline_start_ms
                        .saturating_add(start + (end - start) * index as u64 / parts.len() as u64),
                    end_ms: timeline_start_ms.saturating_add(
                        start + (end - start) * (index + 1) as u64 / parts.len() as u64,
                    ),
                    text: (*word).to_owned(),
                })
                .collect();
        }
        result.push(PlacedSegment {
            start_ms: timeline_start_ms.saturating_add(start),
            end_ms: timeline_start_ms.saturating_add(end),
            text,
            speaker,
            words,
        });
    }
    result.sort_by_key(|s| (s.start_ms, s.end_ms));
    result
}

/// Whether a word closes a sentence, looking past closing quotes and brackets.
pub fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', '”', '’', ')', ']'])
        .ends_with(['.', '?', '!', '…', '。', '？', '！'])
}

pub fn segment_sentences(placed: Vec<PlacedSegment>) -> Vec<PlacedSegment> {
    let mut by_source: [Vec<RawWord>; 3] = std::array::from_fn(|_| Vec::new());
    let mut result = Vec::new();
    for segment in placed {
        if segment.words.is_empty() {
            result.push(segment);
            continue;
        }
        let index = match segment.speaker {
            Speaker::Me => 0,
            Speaker::Remote => 1,
            Speaker::Unknown => 2,
        };
        by_source[index].extend(segment.words);
    }
    for (index, mut words) in by_source.into_iter().enumerate() {
        let speaker = match index {
            0 => Speaker::Me,
            1 => Speaker::Remote,
            _ => Speaker::Unknown,
        };
        for word in &mut words {
            word.end_ms = word.end_ms.min(word.start_ms + MAX_WORD_MS);
        }
        words.sort_by_key(|word| (word.start_ms, word.end_ms));
        let mut sentence = Vec::new();
        let mut sentences = Vec::new();
        for word in words {
            if sentence.last().is_some_and(|previous: &RawWord| {
                word.start_ms.saturating_sub(previous.end_ms) > MAX_VAD_GAP_MS
            }) {
                sentences.push(std::mem::take(&mut sentence));
            }
            let ends = ends_sentence(&word.text);
            sentence.push(word);
            if ends {
                sentences.push(std::mem::take(&mut sentence));
            }
        }
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
        let mut cursor = 0;
        while cursor < sentences.len() && sentences.len() > 1 {
            let short = {
                let words = &sentences[cursor];
                words.last().unwrap().end_ms - words.first().unwrap().start_ms < MIN_SEGMENT_MS
                    || words.len() < MIN_SEGMENT_WORDS
            };
            if !short {
                cursor += 1;
                continue;
            }
            let left_gap = cursor.checked_sub(1).and_then(|left| {
                let gap = sentences[cursor][0]
                    .start_ms
                    .saturating_sub(sentences[left].last().unwrap().end_ms);
                (gap <= MAX_VAD_GAP_MS).then_some(gap)
            });
            let right_gap = sentences.get(cursor + 1).and_then(|right| {
                let gap = right[0]
                    .start_ms
                    .saturating_sub(sentences[cursor].last().unwrap().end_ms);
                (gap <= MAX_VAD_GAP_MS).then_some(gap)
            });
            match (left_gap, right_gap) {
                (Some(left), Some(right)) if left <= right => {
                    let words = sentences.remove(cursor);
                    sentences[cursor - 1].extend(words);
                    cursor -= 1;
                }
                (_, Some(_)) => {
                    let words = sentences.remove(cursor);
                    sentences[cursor].splice(0..0, words);
                }
                (Some(_), None) => {
                    let words = sentences.remove(cursor);
                    sentences[cursor - 1].extend(words);
                    cursor -= 1;
                }
                (None, None) => cursor += 1,
            }
        }
        let mut cursor = 0;
        while cursor < sentences.len() {
            let words = &sentences[cursor];
            if words.last().unwrap().end_ms - words[0].start_ms > MAX_SEGMENT_MS && words.len() > 1
            {
                let middle = words.len() / 2;
                // Neither part may be a fragment too short to stand alone, when the words allow it.
                let fragment = |part: &[RawWord]| {
                    part.len() < MIN_SEGMENT_WORDS
                        || part[part.len() - 1].end_ms - part[0].start_ms < MIN_SEGMENT_MS
                };
                let split = (1..words.len())
                    .max_by_key(|&index| {
                        (
                            !fragment(&words[..index]) && !fragment(&words[index..]),
                            words[index]
                                .start_ms
                                .saturating_sub(words[index - 1].end_ms),
                            usize::MAX - index.abs_diff(middle),
                        )
                    })
                    .unwrap();
                let right = sentences[cursor].split_off(split);
                sentences.insert(cursor + 1, right);
            } else {
                cursor += 1;
            }
        }
        for sentence in sentences {
            result.push(words_to_segment(sentence, speaker));
        }
    }
    result.sort_by_key(|segment| (segment.start_ms, segment.end_ms));
    result
}

fn words_to_segment(words: Vec<RawWord>, speaker: Speaker) -> PlacedSegment {
    PlacedSegment {
        start_ms: words.first().unwrap().start_ms,
        end_ms: words.last().unwrap().end_ms,
        text: words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        speaker,
        words,
    }
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
    render_markdown_named(
        title,
        transcript,
        &Participants::new(MeetingParticipants::new(), None, None, false),
    )
}

pub fn render_markdown_named(
    title: &str,
    transcript: &Transcript,
    participants: &Participants,
) -> String {
    let language = transcript.language.name();
    let mut result = format!(
        "# {title}\n\nTranscript revision {} · model {} · {language}\n",
        transcript.revision,
        transcript
            .model
            .iter()
            .map(EngineName::label)
            .collect::<Vec<_>>()
            .join(", ")
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

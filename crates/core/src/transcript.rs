use crate::participant::{MeetingParticipants, Participants};
use serde::{Deserialize, Serialize};

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
    pub words: Vec<RawWord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawWord {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

pub const MAX_VAD_GAP_MS: u64 = 2_000;
pub const MIN_SEGMENT_MS: u64 = 1_000;
pub const MIN_SEGMENT_WORDS: usize = 3;
pub const MAX_SEGMENT_MS: u64 = 20_000;

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

pub fn segment_sentences(placed: Vec<PlacedSegment>) -> Vec<PlacedSegment> {
    let mut by_source: [Vec<RawWord>; 3] = std::array::from_fn(|_| Vec::new());
    for segment in placed {
        let index = match segment.speaker {
            Speaker::Me => 0,
            Speaker::Remote => 1,
            Speaker::Unknown => 2,
        };
        by_source[index].extend(segment.words);
    }
    let mut result = Vec::new();
    for (index, mut words) in by_source.into_iter().enumerate() {
        let speaker = match index {
            0 => Speaker::Me,
            1 => Speaker::Remote,
            _ => Speaker::Unknown,
        };
        words.sort_by_key(|word| (word.start_ms, word.end_ms));
        let mut sentence = Vec::new();
        let mut sentences = Vec::new();
        for word in words {
            if sentence.last().is_some_and(|previous: &RawWord| {
                word.start_ms.saturating_sub(previous.end_ms) > MAX_VAD_GAP_MS
            }) {
                sentences.push(std::mem::take(&mut sentence));
            }
            let ends_sentence = word
                .text
                .trim_end_matches(['"', '\'', '”', '’', ')', ']'])
                .ends_with(['.', '?', '!', '…', '。', '？', '！']);
            sentence.push(word);
            if ends_sentence {
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
                let split = (1..words.len())
                    .max_by_key(|&index| {
                        (
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

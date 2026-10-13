use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    participant::Participants,
    transcript::{Segment, Transcript},
};

/// A Segment ID and its Timeline range, copied from the Transcript by app code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub segment_id: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CitationCheck {
    pub valid: Vec<Citation>,
    pub invalid: Vec<String>,
}

pub fn check_citations(
    transcript: &Transcript,
    allowed: Option<&HashSet<String>>,
    raw: &[String],
) -> CitationCheck {
    let mut check = CitationCheck::default();
    let mut seen = HashSet::new();
    for id in raw {
        let trimmed = id.trim();
        let normalized = trimmed
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(trimmed)
            .trim();
        if normalized.is_empty() || !seen.insert(normalized.to_owned()) {
            continue;
        }
        match transcript.segment(normalized) {
            Some(segment) if allowed.is_none_or(|ids| ids.contains(normalized)) => {
                check.valid.push(Citation {
                    segment_id: normalized.to_owned(),
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                });
            }
            _ => check.invalid.push(normalized.to_owned()),
        }
    }
    check
}

pub fn evidence_line(segment: &Segment, participants: &Participants, revision: u32) -> String {
    format!(
        "[{}] {}: {}",
        segment.id,
        participants.name(segment, revision),
        segment.text
    )
}

pub fn evidence_text<'a>(
    segments: impl IntoIterator<Item = &'a Segment>,
    participants: &Participants,
    revision: u32,
) -> String {
    segments
        .into_iter()
        .map(|segment| evidence_line(segment, participants, revision))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(3)
}

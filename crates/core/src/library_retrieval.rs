use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    library::LibraryError,
    participant::Participants,
    transcript::{Language, Transcript},
    Library,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSegment {
    pub segment_id: String,
    pub revision: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    pub participant: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingEvidence {
    pub meeting_id: String,
    pub title: String,
    pub started_at: String,
    pub language: Option<Language>,
    pub segments: Vec<EvidenceSegment>,
}

pub(crate) fn evidence_header(prefix: &str, meeting: &MeetingEvidence) -> String {
    format!(
        "{prefix} Title: {}; date: {}",
        meeting.title, meeting.started_at
    )
}

pub(crate) fn evidence_segment_line(prefix: &str, segment: &EvidenceSegment) -> String {
    format!(
        "[{prefix}-{}] {}: {}",
        segment.segment_id, segment.participant, segment.text
    )
}

struct MeetingSource {
    transcript: Transcript,
    participants: Participants,
    segment_positions: HashMap<String, usize>,
}

impl Library {
    pub fn meeting_languages(&self) -> Result<Vec<Language>, LibraryError> {
        let mut present = HashSet::new();
        for meeting in self.index.list()? {
            if let Some(language) = self
                .meeting(&meeting.id)?
                .language
                .as_deref()
                .and_then(Language::from_code)
            {
                present.insert(language.code());
            }
        }
        Ok(Language::ALL
            .into_iter()
            .filter(|language| present.contains(language.code()))
            .collect())
    }

    pub fn library_evidence(
        &self,
        terms: &[String],
        since_ms: Option<i64>,
        budget_tokens: usize,
    ) -> Result<Vec<MeetingEvidence>, LibraryError> {
        let hits = self.index.ranked_segments(terms, since_ms)?;
        let mut sources: HashMap<String, MeetingSource> = HashMap::new();
        let mut groups: Vec<(MeetingEvidence, HashSet<String>)> = Vec::new();
        let mut group_indices = HashMap::new();
        let mut selected_chars: usize = 0;
        let mut selected_count: usize = 0;
        for hit in hits {
            if selected_chars.div_ceil(3) >= budget_tokens {
                break;
            }
            if !sources.contains_key(&hit.meeting_id) {
                let Some(transcript) = self.transcript(&hit.meeting_id)? else {
                    continue;
                };
                let segment_positions = transcript
                    .segments
                    .iter()
                    .enumerate()
                    .map(|(index, segment)| (segment.id.clone(), index))
                    .collect();
                sources.insert(
                    hit.meeting_id.clone(),
                    MeetingSource {
                        transcript,
                        participants: self.participants(&hit.meeting_id)?,
                        segment_positions,
                    },
                );
            }
            let source = &sources[&hit.meeting_id];
            let transcript = &source.transcript;
            let participants = &source.participants;
            if transcript.revision != hit.revision {
                continue;
            }
            let Some(&hit_index) = source.segment_positions.get(&hit.segment_id) else {
                continue;
            };
            let group_index = if let Some(index) = group_indices.get(&hit.meeting_id) {
                *index
            } else {
                let meeting = self.meeting(&hit.meeting_id)?;
                let index = groups.len();
                groups.push((
                    MeetingEvidence {
                        meeting_id: hit.meeting_id.clone(),
                        title: meeting.title,
                        started_at: meeting.started_at,
                        language: meeting.language.as_deref().and_then(Language::from_code),
                        segments: Vec::new(),
                    },
                    HashSet::new(),
                ));
                group_indices.insert(hit.meeting_id.clone(), index);
                index
            };
            let (meeting, selected) = &mut groups[group_index];
            for index in [
                Some(hit_index),
                hit_index.checked_sub(1),
                (hit_index + 1 < transcript.segments.len()).then_some(hit_index + 1),
            ]
            .into_iter()
            .flatten()
            {
                let segment = &transcript.segments[index];
                if selected.contains(&segment.id) {
                    continue;
                }
                let evidence_segment = EvidenceSegment {
                    segment_id: segment.id.clone(),
                    revision: transcript.revision,
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                    participant: participants.name(segment, transcript.revision),
                    text: segment.text.clone(),
                };
                let prefix = format!("M{}", group_index + 1);
                let line_chars = evidence_segment_line(&prefix, &evidence_segment)
                    .chars()
                    .count();
                let extra_chars = if meeting.segments.is_empty() {
                    evidence_header(&prefix, meeting).chars().count()
                        + 1
                        + usize::from(selected_count > 0) * 2
                } else {
                    1
                };
                let trial_chars = selected_chars + line_chars + extra_chars;
                if trial_chars.div_ceil(3) > budget_tokens {
                    continue;
                }
                selected.insert(segment.id.clone());
                meeting.segments.push(evidence_segment);
                selected_chars = trial_chars;
                selected_count += 1;
            }
        }
        Ok(groups
            .into_iter()
            .filter_map(|(mut meeting, _)| {
                if meeting.segments.is_empty() {
                    return None;
                }
                meeting.segments.sort_by_key(|segment| segment.start_ms);
                Some(meeting)
            })
            .collect())
    }
}

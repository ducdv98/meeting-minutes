use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    ai::{generate_json, AiError, GenerateRequest, Message, OutputSchema, Role, TextProvider},
    transcript::{Language, Segment},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const DEFAULT_CHAR_BUDGET: usize = 2_000_000;
/// Gemini Flash returns up to 65,536 tokens; Vietnamese JSON runs about 2 characters per token.
const MAX_OUTPUT_CHARS: usize = 120_000;

#[derive(Clone, Debug, Deserialize)]
pub struct CorrectedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Deserialize)]
struct CorrectionResponse {
    segments: Vec<CorrectedSegment>,
}

#[derive(Serialize)]
struct RequestSegment<'a> {
    id: &'a str,
    speaker: crate::transcript::Speaker,
    text: &'a str,
}

fn schema() -> OutputSchema {
    OutputSchema {
        name: "meeting_correction".into(),
        schema: json!({
            "type": "object",
            "properties": {"segments": {"type": "array", "items": {
                "type": "object", "properties": {"id": {"type": "string"}, "text": {"type": "string"}},
                "required": ["id", "text"], "additionalProperties": false
            }}},
            "required": ["segments"], "additionalProperties": false
        }),
    }
}

pub fn correct(
    provider: &dyn TextProvider,
    segments: &[Segment],
    language: Language,
    vocabulary: &[String],
    participant_names: &[String],
    cancel: &AtomicBool,
) -> Result<Vec<Segment>, AiError> {
    correct_with_budget(
        provider,
        segments,
        language,
        vocabulary,
        participant_names,
        DEFAULT_CHAR_BUDGET,
        cancel,
    )
}

/// The budget is a conservative character estimate of both input and expected output.
pub fn correct_with_budget(
    provider: &dyn TextProvider,
    segments: &[Segment],
    language: Language,
    vocabulary: &[String],
    participant_names: &[String],
    char_budget: usize,
    cancel: &AtomicBool,
) -> Result<Vec<Segment>, AiError> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let language = match language {
        Language::En => "English",
        Language::Vi => "Vietnamese",
    };
    let system = format!(
        "Correct this {language} Meeting Transcript. Fix only misheard words, Participant names, Vocabulary terms, punctuation and diacritics. Never rephrase, summarise, translate, add, remove, merge or split Segments. Return one id and corrected text for each Segment. Vocabulary: {}. Participants: {}.",
        vocabulary.join(", "), participant_names.join(", ")
    );
    let costs = segments
        .iter()
        .map(|segment| {
            let item = RequestSegment {
                id: &segment.id,
                speaker: segment.speaker,
                text: &segment.text,
            };
            serde_json::to_string(&item).map(|json| json.chars().count() * 2 + 1)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AiError::InvalidOutput(error.to_string()))?;
    let output_costs = segments
        .iter()
        .map(|segment| segment.text.chars().count() + segment.id.len() + 32)
        .collect::<Vec<_>>();
    let overhead = system.chars().count() + 64;
    let mut corrections = Vec::new();
    let mut start = 0;
    while start < segments.len() {
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        let mut end = start;
        let mut used = overhead;
        let mut output_chars = 0usize;
        while end < segments.len()
            && (end == start
                || (used.saturating_add(costs[end]) <= char_budget
                    && output_chars.saturating_add(output_costs[end]) <= MAX_OUTPUT_CHARS))
        {
            used = used.saturating_add(costs[end]);
            output_chars = output_chars.saturating_add(output_costs[end]);
            end += 1;
        }
        let batch = &segments[start..end];
        let input = batch
            .iter()
            .map(|segment| RequestSegment {
                id: &segment.id,
                speaker: segment.speaker,
                text: &segment.text,
            })
            .collect::<Vec<_>>();
        let content = serde_json::to_string(&input)
            .map_err(|error| AiError::InvalidOutput(error.to_string()))?;
        let request = GenerateRequest {
            system: system.clone(),
            messages: vec![Message {
                role: Role::User,
                content,
            }],
            schema: Some(schema()),
            max_tokens: (output_chars / 2 + 1024).min(65_536) as u32,
            inline_audio: None,
        };
        let response: CorrectionResponse = generate_json(provider, &request, &mut |_| {}, cancel)?;
        let ids: HashSet<&str> = batch.iter().map(|segment| segment.id.as_str()).collect();
        corrections.extend(
            response
                .segments
                .into_iter()
                .filter(|segment| ids.contains(segment.id.as_str())),
        );
        start = end;
    }
    Ok(apply(segments, &corrections))
}

/// Accept a replacement only when exactly one nonempty result names a known Segment.
pub fn apply(segments: &[Segment], corrections: &[CorrectedSegment]) -> Vec<Segment> {
    let known: HashSet<&str> = segments.iter().map(|segment| segment.id.as_str()).collect();
    let mut by_id = HashMap::<&str, Option<&str>>::new();
    for correction in corrections {
        if !known.contains(correction.id.as_str()) {
            continue;
        }
        by_id
            .entry(&correction.id)
            .and_modify(|text| *text = None)
            .or_insert_with(|| Some(correction.text.trim()));
    }
    segments
        .iter()
        .cloned()
        .map(|mut segment| {
            if let Some(Some(text)) = by_id.get(segment.id.as_str()) {
                if !text.is_empty() {
                    segment.text = (*text).to_owned();
                }
            }
            segment
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ProviderKind;
    use crate::transcript::Speaker;
    use std::sync::{atomic::AtomicBool, Mutex};

    struct FakeProvider {
        requests: Mutex<Vec<GenerateRequest>>,
        responses: Mutex<Vec<Result<String, AiError>>>,
    }
    impl TextProvider for FakeProvider {
        fn kind(&self) -> ProviderKind {
            ProviderKind::Gemini
        }
        fn model(&self) -> &str {
            "gemini-2.5-flash"
        }
        fn generate(
            &self,
            request: &GenerateRequest,
            _: &mut dyn FnMut(&str),
            _: &AtomicBool,
        ) -> Result<String, AiError> {
            self.requests.lock().unwrap().push(request.clone());
            self.responses.lock().unwrap().remove(0)
        }
    }

    fn segment(id: &str, start_ms: u64, text: &str) -> Segment {
        Segment {
            id: id.into(),
            start_ms,
            end_ms: start_ms + 1000,
            text: text.into(),
            speaker: Speaker::Remote,
        }
    }

    #[test]
    fn correction_changes_only_text_for_unique_known_ids() {
        let original = vec![
            segment("s00001", 100, "Tran"),
            segment("s00002", 1100, "was here"),
        ];
        let result = apply(
            &original,
            &[
                CorrectedSegment {
                    id: "s00001".into(),
                    text: "Trần".into(),
                },
                CorrectedSegment {
                    id: "unknown".into(),
                    text: "invented".into(),
                },
            ],
        );
        assert_eq!(
            result,
            vec![
                segment("s00001", 100, "Trần"),
                segment("s00002", 1100, "was here")
            ]
        );
    }

    #[test]
    fn correction_keeps_missing_duplicate_and_empty_segments() {
        let original = vec![
            segment("s1", 0, "One"),
            segment("s2", 1000, "Two"),
            segment("s3", 2000, "Three"),
            segment("s4", 3000, "Four"),
        ];
        let result = apply(
            &original,
            &[
                CorrectedSegment {
                    id: "s1".into(),
                    text: "First".into(),
                },
                CorrectedSegment {
                    id: "s1".into(),
                    text: "Also first".into(),
                },
                CorrectedSegment {
                    id: "s2".into(),
                    text: "  ".into(),
                },
                CorrectedSegment {
                    id: "s4".into(),
                    text: "Fourth".into(),
                },
            ],
        );
        assert_eq!(
            result,
            vec![
                segment("s1", 0, "One"),
                segment("s2", 1000, "Two"),
                segment("s3", 2000, "Three"),
                segment("s4", 3000, "Fourth")
            ]
        );
    }

    #[test]
    fn correction_sends_one_structured_request_with_meeting_context() {
        let provider = FakeProvider {
            requests: Mutex::new(vec![]),
            responses: Mutex::new(vec![Ok(
                r#"{"segments":[{"id":"s00001","text":"Nguyễn spoke."}]}"#.into(),
            )]),
        };
        let original = vec![segment("s00001", 0, "Nuwen spoke.")];
        let result = correct(
            &provider,
            &original,
            Language::Vi,
            &["Nguyễn".into()],
            &["Minh".into()],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(result[0].text, "Nguyễn spoke.");
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.schema.as_ref().unwrap().name, "meeting_correction");
        assert!(request.messages[0].content.contains("Nuwen spoke."));
        assert!(request.messages[0].content.contains("remote"));
        assert!(request.system.contains("Vietnamese"));
        assert!(request.system.contains("Nguyễn"));
        assert!(request.system.contains("Minh"));
    }

    #[test]
    fn correction_batches_consecutive_segments_when_context_is_small() {
        let provider = FakeProvider {
            requests: Mutex::new(vec![]),
            responses: Mutex::new(vec![
                Ok(
                    r#"{"segments":[{"id":"s1","text":"First"},{"id":"s2","text":"Second"}]}"#
                        .into(),
                ),
                Ok(r#"{"segments":[{"id":"s3","text":"Third"}]}"#.into()),
            ]),
        };
        let original = vec![
            segment("s1", 0, "One"),
            segment("s2", 1000, "Two"),
            segment("s3", 2000, "Three"),
        ];
        let result = correct_with_budget(
            &provider,
            &original,
            Language::En,
            &[],
            &[],
            600,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            result.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
            ["First", "Second", "Third"]
        );
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].messages[0].content.contains("s1"));
        assert!(requests[0].messages[0].content.contains("s2"));
        assert!(!requests[0].messages[0].content.contains("s3"));
        assert!(requests[1].messages[0].content.contains("s3"));
    }

    #[test]
    fn an_hour_long_meeting_is_corrected_in_one_request_within_gemini_output_limits() {
        // About 700 sentence Segments of about 80 characters: an hour of speech.
        let original: Vec<Segment> = (0..700)
            .map(|index| {
                segment(
                    &format!("s{index:05}"),
                    index * 5000,
                    "Chúng ta sẽ chốt lịch phát hành vào thứ sáu tuần sau nhé, rồi gửi cho anh Minh.",
                )
            })
            .collect();
        let provider = FakeProvider {
            requests: Mutex::new(vec![]),
            responses: Mutex::new(vec![Ok(r#"{"segments":[]}"#.into())]),
        };
        correct(
            &provider,
            &original,
            Language::Vi,
            &[],
            &[],
            &AtomicBool::new(false),
        )
        .unwrap();
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].max_tokens <= 65_536);
    }
}

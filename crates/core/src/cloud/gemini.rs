use crate::{
    ai::{
        generate_json, AiError, ApiKey, GenerateRequest, HttpClient, HttpProvider, InlineAudio,
        Message, OutputSchema, Role,
    },
    cloud::{encode_flac, CloudEngine, EngineError},
    processing::cloud_span_bounds,
    transcript::{shift_segment_times, Language, RawSegment},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

const SAMPLE_RATE: usize = 16_000;
const PIECE_SAMPLES: usize = 30 * SAMPLE_RATE;
const CUT_SEARCH_SAMPLES: usize = 5 * SAMPLE_RATE;

#[derive(Deserialize)]
struct Sentences {
    sentences: Vec<Sentence>,
}
#[derive(Deserialize)]
struct DetectedLanguage {
    language: String,
}
#[derive(Deserialize)]
struct Sentence {
    start_s: f64,
    end_s: f64,
    text: String,
}

pub struct GeminiEngine {
    provider: HttpProvider,
}
impl GeminiEngine {
    pub fn new(http: Arc<dyn HttpClient>, key: ApiKey, model: impl Into<String>) -> Self {
        Self {
            provider: HttpProvider::gemini(http, model, key),
        }
    }

    fn transcribe_piece(
        &self,
        samples: &[f32],
        language: Language,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let language = language.name();
        let request = GenerateRequest {
            system: format!("Transcribe this Meeting audio verbatim in {language}. Use the Vocabulary and Participant names only to spell spoken words accurately: {}. Return sentences with start_s and end_s in seconds relative to this audio. Do not translate or summarise.", vocabulary.unwrap_or("")),
            messages: vec![Message { role: Role::User, content: "Transcribe the attached audio into timed sentences.".into() }],
            schema: Some(OutputSchema {
                name: "meeting_transcription".into(),
                schema: json!({"type":"object","properties":{"sentences":{"type":"array","items":{"type":"object","properties":{"start_s":{"type":"number"},"end_s":{"type":"number"},"text":{"type":"string"}},"required":["start_s","end_s","text"]}}},"required":["sentences"]}),
            }),
            max_tokens: 8192,
            inline_audio: Some(InlineAudio { mime_type: "audio/flac".into(), data: STANDARD.encode(encode_flac(samples)?) }),
        };
        let response: Sentences =
            generate_json(&self.provider, &request, &mut |_| {}, cancel).map_err(map_error)?;
        let duration_s = samples.len() as f64 / SAMPLE_RATE as f64;
        let duration_ms = samples.len() as u64 / 16;
        let mut previous_end = 0;
        let mut result = Vec::new();
        for sentence in response.sentences {
            let text = sentence.text.trim();
            if text.is_empty() {
                continue;
            }
            // Small timestamp overruns are clamped; a larger claim means the response is for different audio.
            if !sentence.start_s.is_finite()
                || !sentence.end_s.is_finite()
                || sentence.start_s < 0.0
                || sentence.end_s < sentence.start_s
                || sentence.start_s > duration_s + 1.0
                || sentence.end_s > duration_s + 1.0
            {
                return Err(EngineError::InvalidResponse);
            }
            let start_ms = ((sentence.start_s * 1000.0).round() as u64)
                .min(duration_ms)
                .max(previous_end);
            let end_ms = ((sentence.end_s * 1000.0).round() as u64)
                .min(duration_ms)
                .max(start_ms);
            if start_ms == end_ms {
                continue;
            }
            result.push(RawSegment {
                start_ms,
                end_ms,
                text: text.into(),
                words: vec![],
            });
            previous_end = end_ms;
        }
        Ok(result)
    }
}

fn map_error(error: AiError) -> EngineError {
    match error {
        AiError::GeminiLimit => EngineError::QuotaExhausted,
        AiError::RateLimited { retry_after } => EngineError::RateLimited { retry_after },
        AiError::Auth(_) => EngineError::Unauthorized,
        AiError::Unreachable(_) => EngineError::Offline,
        AiError::Cancelled => EngineError::Cancelled,
        AiError::Http { status: 429, .. } => EngineError::RateLimited {
            retry_after: Duration::from_secs(60),
        },
        AiError::Http { status, .. } => EngineError::HttpStatus(status),
        AiError::Io(_) => EngineError::Transport,
        AiError::InvalidOutput(_) | AiError::Refused | AiError::Truncated => {
            EngineError::InvalidResponse
        }
        _ => EngineError::Other,
    }
}

impl CloudEngine for GeminiEngine {
    fn name(&self) -> &str {
        "Gemini"
    }
    fn detect_language(
        &mut self,
        samples_16k: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        let opening = &samples_16k[..samples_16k.len().min(30 * SAMPLE_RATE)];
        let languages = Language::ALL.map(|language| language.code());
        let names = Language::ALL.map(|language| language.name()).join(", ");
        let request = GenerateRequest {
            system: format!("Identify the spoken Meeting language from this audio. Choose one of {names}. Return its two-letter language code only in the JSON object."),
            messages: vec![Message { role: Role::User, content: "Identify the spoken language of the attached audio.".into() }],
            schema: Some(OutputSchema {
                name: "meeting_language".into(),
                schema: json!({"type":"object","properties":{"language":{"type":"string","enum":languages}},"required":["language"]}),
            }),
            max_tokens: 256,
            inline_audio: Some(InlineAudio { mime_type: "audio/flac".into(), data: STANDARD.encode(encode_flac(opening)?) }),
        };
        let response: DetectedLanguage =
            generate_json(&self.provider, &request, &mut |_| {}, cancel).map_err(map_error)?;
        Language::from_code(&response.language)
            .map(Some)
            .ok_or(EngineError::InvalidResponse)
    }
    fn transcribe_span(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        let mut result = Vec::new();
        for piece in cloud_span_bounds(samples_16k, PIECE_SAMPLES, CUT_SEARCH_SAMPLES) {
            let mut segments =
                self.transcribe_piece(&samples_16k[piece.clone()], language, vocabulary, cancel)?;
            let offset_ms = piece.start as u64 / 16;
            for segment in &mut segments {
                shift_segment_times(
                    &mut segment.start_ms,
                    &mut segment.end_ms,
                    &mut segment.words,
                    offset_ms,
                );
            }
            result.extend(segments);
        }
        Ok(result)
    }
}

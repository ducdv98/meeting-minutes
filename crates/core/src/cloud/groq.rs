use crate::{
    ai::{AiError, ApiKey, HttpBody, HttpClient, HttpRequest, HttpResponse, Method, UreqClient},
    cloud::{encode_flac, CloudEngine},
    processing::{ProcessingError, SpeechEngine},
    transcript::{Language, RawSegment, RawWord},
};
use serde_json::Value;
use std::{
    io::{Cursor, Read},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

const URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const MODEL: &str = "whisper-large-v3";
const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
/// Matches the engine chain's longest span, so spans it cut at a pause are not cut again.
const MAX_SPAN_SAMPLES: usize = 10 * 60 * 16_000;
static BOUNDARY_ID: AtomicU64 = AtomicU64::new(0);

pub use crate::cloud::EngineError;

pub struct GroqEngine {
    http: Arc<dyn HttpClient>,
    key: ApiKey,
    boundary: String,
}
impl GroqEngine {
    pub fn new(http: Arc<dyn HttpClient>, key: ApiKey) -> Self {
        let id = BOUNDARY_ID.fetch_add(1, Ordering::Relaxed);
        Self::with_boundary(
            http,
            key,
            format!("meeting-minutes-{}-{id}", std::process::id()),
        )
    }

    pub fn with_boundary(
        http: Arc<dyn HttpClient>,
        key: ApiKey,
        boundary: impl Into<String>,
    ) -> Self {
        Self {
            http,
            key,
            boundary: boundary.into(),
        }
    }

    pub fn transcribe_typed(
        &self,
        samples_16k: &[f32],
        language: Option<Language>,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.transcribe_with_language(samples_16k, language, vocabulary, cancel)
            .map(|(segments, _)| segments)
    }

    fn transcribe_with_language(
        &self,
        samples: &[f32],
        language: Option<Language>,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<(Vec<RawSegment>, Option<Language>), EngineError> {
        let mut all = Vec::new();
        let mut detected = None;
        for (chunk_number, chunk) in samples.chunks(MAX_SPAN_SAMPLES).enumerate() {
            let offset_ms = chunk_number as u64 * MAX_SPAN_SAMPLES as u64 * 1000 / 16_000;
            let (mut segments, found) = self.upload(chunk, language, vocabulary, cancel)?;
            detected = detected.or(found);
            for segment in &mut segments {
                segment.start_ms += offset_ms;
                segment.end_ms += offset_ms;
                for word in &mut segment.words {
                    word.start_ms += offset_ms;
                    word.end_ms += offset_ms;
                }
            }
            all.extend(segments);
        }
        Ok((all, detected))
    }

    fn upload(
        &self,
        samples: &[f32],
        language: Option<Language>,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<(Vec<RawSegment>, Option<Language>), EngineError> {
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let audio = encode_flac(samples)?;
        if audio.len() > MAX_FILE_BYTES {
            if samples.len() < 2 {
                return Err(EngineError::Other);
            }
            let half = samples.len() / 2;
            let (mut first, first_lang) =
                self.upload(&samples[..half], language, vocabulary, cancel)?;
            let (mut second, second_lang) =
                self.upload(&samples[half..], language, vocabulary, cancel)?;
            let offset = half as u64 * 1000 / 16_000;
            for segment in &mut second {
                segment.start_ms += offset;
                segment.end_ms += offset;
                for word in &mut segment.words {
                    word.start_ms += offset;
                    word.end_ms += offset;
                }
            }
            first.extend(second);
            return Ok((first, first_lang.or(second_lang)));
        }
        let body = multipart(&self.boundary, &audio, language, vocabulary);
        let response = send_cancellable(
            self.http.clone(),
            HttpRequest {
                method: Method::Post,
                url: URL.into(),
                headers: vec![(
                    "Authorization".into(),
                    format!("Bearer {}", self.key.expose()),
                )],
                body: Some(HttpBody::Multipart {
                    boundary: self.boundary.clone(),
                    bytes: body,
                }),
            },
            cancel,
        )
        .map_err(|error| match error {
            AiError::Unreachable(_) => EngineError::Offline,
            AiError::Cancelled => EngineError::Cancelled,
            _ => EngineError::Other,
        })?;
        if !(200..300).contains(&response.status) {
            let status = response.status;
            let retry_after = response
                .header("retry-after")
                .and_then(|v| v.parse::<u64>().ok());
            let daily_header = response.headers.iter().any(|(name, _)| {
                name.to_ascii_lowercase().starts_with("x-ratelimit-")
                    && name.to_ascii_lowercase().contains("day")
            });
            let mut raw = String::new();
            response
                .body
                .take(64 * 1024)
                .read_to_string(&mut raw)
                .map_err(|_| EngineError::Other)?;
            let value: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            let message = value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or(&raw)
                .to_ascii_lowercase();
            let daily_message = [
                "tokens per day",
                "requests per day",
                "audio seconds per day",
            ]
            .iter()
            .any(|term| message.contains(term));
            return Err(match status {
                401 | 403 => EngineError::Unauthorized,
                429 if daily_header || daily_message => EngineError::QuotaExhausted,
                429 => EngineError::RateLimited {
                    retry_after: Duration::from_secs(retry_after.unwrap_or(60)),
                },
                _ => EngineError::Other,
            });
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(EngineError::Cancelled);
        }
        let value: Value =
            serde_json::from_reader(response.body).map_err(|_| EngineError::Other)?;
        let found = value["language"].as_str().and_then(language_from_groq);
        let top_words = value["words"]
            .as_array()
            .map(|words| words.iter().map(parse_word).collect::<Result<Vec<_>, _>>())
            .transpose()?;
        let segments = value["segments"].as_array().ok_or(EngineError::Other)?;
        let segments = segments
            .iter()
            .map(|segment| {
                let start_ms = millis(&segment["start"])?;
                let end_ms = millis(&segment["end"])?;
                let words = if let Some(words) = &top_words {
                    words
                        .iter()
                        .filter(|word| {
                            let middle = word.start_ms / 2 + word.end_ms / 2;
                            start_ms <= middle && middle < end_ms
                        })
                        .cloned()
                        .collect()
                } else {
                    segment["words"]
                        .as_array()
                        .map(|words| words.iter().map(parse_word).collect::<Result<Vec<_>, _>>())
                        .transpose()?
                        .unwrap_or_default()
                };
                Ok(RawSegment {
                    start_ms,
                    end_ms,
                    text: segment["text"].as_str().ok_or(EngineError::Other)?.into(),
                    words,
                })
            })
            .collect::<Result<Vec<_>, EngineError>>()?;
        Ok((segments, found))
    }
}

fn send_cancellable(
    http: Arc<dyn HttpClient>,
    request: HttpRequest,
    cancel: &AtomicBool,
) -> Result<HttpResponse, AiError> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = http.send(request).and_then(|mut response| {
            let mut body = Vec::new();
            (&mut response.body)
                .take(16 * 1024 * 1024)
                .read_to_end(&mut body)?;
            response.body = Box::new(Cursor::new(body));
            Ok(response)
        });
        let _ = sender.send(result);
    });
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(AiError::Cancelled);
        }
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(AiError::Unreachable("Provider request ended".into()))
            }
        }
    }
}

impl SpeechEngine for GroqEngine {
    fn backend(&self) -> &str {
        "Groq"
    }
    fn thread_count(&self) -> usize {
        0
    }
    fn device(&self) -> Option<&str> {
        Some(MODEL)
    }
    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String> {
        let opening = &samples_16k[..samples_16k.len().min(30 * 16_000)];
        let (_, language) = self
            .transcribe_with_language(opening, None, None, &AtomicBool::new(false))
            .map_err(|error| error.to_string())?;
        language.ok_or_else(|| "Groq did not return a supported Meeting language".into())
    }
    fn transcribe(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        let result = self
            .transcribe_typed(samples_16k, Some(language), initial_prompt, cancel)
            .map_err(|error| match error {
                EngineError::Cancelled => ProcessingError::Cancelled,
                _ => ProcessingError::Engine(error.to_string()),
            })?;
        progress(1.0);
        Ok(result)
    }
}

impl CloudEngine for GroqEngine {
    fn name(&self) -> &str {
        "Groq"
    }
    fn transcribe_span(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError> {
        self.transcribe_typed(samples_16k, Some(language), vocabulary, cancel)
    }
    fn detect_language(
        &mut self,
        samples_16k: &[f32],
        cancel: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        let opening = &samples_16k[..samples_16k.len().min(30 * 16_000)];
        self.transcribe_with_language(opening, None, None, cancel)
            .map(|(_, language)| language)
    }
}

fn language_from_groq(code: &str) -> Option<Language> {
    match code.to_ascii_lowercase().as_str() {
        "en" | "english" => Some(Language::En),
        "vi" | "vietnamese" => Some(Language::Vi),
        _ => None,
    }
}

fn millis(value: &Value) -> Result<u64, EngineError> {
    let seconds = value.as_f64().ok_or(EngineError::Other)?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(EngineError::Other);
    }
    Ok((seconds * 1000.0).round() as u64)
}

fn parse_word(word: &Value) -> Result<RawWord, EngineError> {
    Ok(RawWord {
        start_ms: millis(&word["start"])?,
        end_ms: millis(&word["end"])?,
        text: word["word"].as_str().ok_or(EngineError::Other)?.into(),
    })
}

fn multipart(
    boundary: &str,
    audio: &[u8],
    language: Option<Language>,
    vocabulary: Option<&str>,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(audio.len() + 1024);
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    field("model", MODEL);
    field("response_format", "verbose_json");
    field("timestamp_granularities[]", "word");
    field("timestamp_granularities[]", "segment");
    if let Some(language) = language {
        field(
            "language",
            match language {
                Language::En => "en",
                Language::Vi => "vi",
            },
        );
    }
    if let Some(prompt) = vocabulary {
        let trimmed: String = prompt.chars().take(80).collect();
        if !trimmed.trim().is_empty() {
            field("prompt", trimmed.trim());
        }
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"span.flac\"\r\nContent-Type: audio/flac\r\n\r\n").as_bytes());
    body.extend_from_slice(audio);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

pub fn with_default_http(key: ApiKey) -> GroqEngine {
    GroqEngine::new(Arc::new(UreqClient), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flac_round_trips_a_span_with_a_partial_last_block() {
        let samples: Vec<f32> = (0..10_000)
            .map(|i| ((i % 200) as f32 / 200.0) - 0.5)
            .collect();
        let audio = encode_flac(&samples).unwrap();
        let mut reader = claxon::FlacReader::new(std::io::Cursor::new(audio)).unwrap();
        let decoded: Vec<i32> = reader.samples().map(Result::unwrap).collect();
        assert_eq!(decoded.len(), samples.len());
        assert_eq!(decoded[150], (samples[150] * 32767.0).round() as i32);
    }
}

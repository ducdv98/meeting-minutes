pub mod groq;
use crate::transcript::{Language, RawSegment};
use std::{fmt, sync::atomic::AtomicBool, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    RateLimited { retry_after: Duration },
    QuotaExhausted,
    Unauthorized,
    Offline,
    Cancelled,
    Other,
}
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RateLimited { .. } => "Rate limit reached",
            Self::QuotaExhausted => "Daily limit reached",
            Self::Unauthorized => "API key rejected",
            Self::Offline => "Provider unavailable",
            Self::Cancelled => "Processing cancelled",
            Self::Other => "Transcription failed",
        })
    }
}
impl std::error::Error for EngineError {}

pub trait CloudEngine: Send {
    fn name(&self) -> &str;
    fn transcribe_span(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        vocabulary: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, EngineError>;
    fn detect_language(
        &mut self,
        _samples_16k: &[f32],
        _cancel: &AtomicBool,
    ) -> Result<Option<Language>, EngineError> {
        Ok(None)
    }
}

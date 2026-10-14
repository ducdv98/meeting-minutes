pub mod gemini;
pub mod groq;
pub mod managed;
use crate::{
    capture::Source,
    transcript::{Language, RawSegment},
};
use flacenc::{
    bitsink::MemSink,
    component::{BitRepr, StreamInfo},
    config,
    error::Verify,
    source::{Fill, FrameBuf},
};
use std::{fmt, sync::atomic::AtomicBool, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    RateLimited { retry_after: Duration },
    QuotaExhausted,
    Unauthorized,
    Offline,
    Cancelled,
    InvalidResponse,
    HttpStatus(u16),
    Transport,
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
            Self::InvalidResponse => "Invalid provider response",
            Self::HttpStatus(_) => "Provider HTTP error",
            Self::Transport => "Provider transport error",
            Self::Other => "Transcription failed",
        })
    }
}
impl EngineError {
    pub fn diagnostic_kind(&self) -> &'static str {
        match self {
            Self::RateLimited { .. } => "rate_limit",
            Self::QuotaExhausted => "quota",
            Self::Unauthorized => "unauthorized",
            Self::Offline => "offline",
            Self::Cancelled => "cancelled",
            Self::InvalidResponse => "invalid_response",
            Self::HttpStatus(_) => "http_status",
            Self::Transport => "transport",
            Self::Other => "other",
        }
    }
}
impl std::error::Error for EngineError {}

pub trait CloudEngine: Send {
    fn name(&self) -> &str;
    fn set_source(&mut self, _source: Option<Source>) {}
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

pub(crate) fn encode_flac(samples: &[f32]) -> Result<Vec<u8>, EngineError> {
    const BLOCK: usize = 4096;
    let config = config::Encoder::default()
        .into_verified()
        .map_err(|_| EngineError::Other)?;
    let mut info = StreamInfo::new(16_000, 1, 16).map_err(|_| EngineError::Other)?;
    info.set_total_samples(samples.len());
    info.set_block_sizes(16, BLOCK)
        .map_err(|_| EngineError::Other)?;
    info.set_frame_sizes(0, 0).map_err(|_| EngineError::Other)?;
    let mut sink = MemSink::<u8>::new();
    info.write(&mut sink).map_err(|_| EngineError::Other)?;
    let mut audio = b"fLaC\x80\0\0\x22".to_vec();
    audio.extend_from_slice(sink.as_slice());
    let mut framebuf = FrameBuf::with_size(1, BLOCK).map_err(|_| EngineError::Other)?;
    for (frame_number, chunk) in samples.chunks(BLOCK).enumerate() {
        let pcm: Vec<i32> = chunk
            .iter()
            .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0).round() as i32)
            .collect();
        framebuf
            .fill_interleaved(&pcm)
            .map_err(|_| EngineError::Other)?;
        let frame = flacenc::encode_fixed_size_frame(&config, &framebuf, frame_number, &info)
            .map_err(|_| EngineError::Other)?;
        sink.clear();
        frame.write(&mut sink).map_err(|_| EngineError::Other)?;
        audio.extend_from_slice(sink.as_slice());
    }
    Ok(audio)
}

//! Disposable CPU int8 Whisper engine for the transcription benchmark.

use ct2rs::{
    sys::{ComputeType, Config, Device},
    Whisper, WhisperOptions,
};
use meeting_minutes_core::{
    processing::{ProcessingError, SpeechDetector, SpeechEngine},
    transcript::{Language, RawSegment, RawWord},
};
use meeting_minutes_whisper::WhisperSpeechDetector;
use std::{
    ops::Range,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

const SAMPLE_RATE: usize = 16_000;
const OVERLAP_SAMPLES: usize = SAMPLE_RATE / 10;
const GAP_SAMPLES: usize = SAMPLE_RATE / 10;

#[derive(Clone, Copy)]
struct TimeMapping {
    processed_start: usize,
    processed_end: usize,
    original_start: usize,
    original_end: usize,
    gap: bool,
}

struct SpeechAudio {
    samples: Vec<f32>,
    mapping: Vec<TimeMapping>,
}

impl SpeechAudio {
    fn original_ms(&self, processed_ms: u64) -> u64 {
        let sample = (processed_ms as usize).saturating_mul(SAMPLE_RATE) / 1000;
        let index = self
            .mapping
            .partition_point(|part| part.processed_end < sample);
        let Some(part) = self.mapping.get(index) else {
            return self.mapping.last().map_or(0, |part| {
                (part.original_end as u64) * 1000 / SAMPLE_RATE as u64
            });
        };
        let offset = sample.saturating_sub(part.processed_start);
        let span = part.processed_end.saturating_sub(part.processed_start);
        let original = if part.gap {
            if offset * 2 < span {
                part.original_start
            } else {
                part.original_end
            }
        } else {
            part.original_start + offset
        };
        original as u64 * 1000 / SAMPLE_RATE as u64
    }
}

// Matches crates/whisper/src/lib.rs:filter_speech. Both engines use the same
// Silero detector, 100 ms overlap, 100 ms inserted gap, and original-time map.
fn filter_speech(samples: &[f32], segments: &[Range<usize>]) -> SpeechAudio {
    let mut filtered = SpeechAudio {
        samples: Vec::new(),
        mapping: Vec::new(),
    };
    for (index, range) in segments.iter().enumerate() {
        let start = range.start.min(samples.len());
        let end = range.end.min(samples.len());
        if end <= start {
            continue;
        }
        let copy_end = if index + 1 < segments.len() {
            end.saturating_add(OVERLAP_SAMPLES).min(samples.len())
        } else {
            end
        };
        let processed_start = filtered.samples.len();
        filtered
            .samples
            .extend_from_slice(&samples[start..copy_end]);
        filtered.mapping.push(TimeMapping {
            processed_start,
            processed_end: filtered.samples.len(),
            original_start: start,
            original_end: copy_end,
            gap: false,
        });
        if let Some(next) = segments.get(index + 1) {
            let processed_start = filtered.samples.len();
            filtered.samples.resize(processed_start + GAP_SAMPLES, 0.0);
            filtered.mapping.push(TimeMapping {
                processed_start,
                processed_end: filtered.samples.len(),
                original_start: copy_end,
                original_end: next.start.min(samples.len()),
                gap: true,
            });
        }
    }
    filtered
}

fn threads() -> usize {
    let logical = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    sysinfo::System::physical_core_count()
        .filter(|&count| count > 0)
        .unwrap_or(logical / 2)
        .clamp(1, 8)
}

fn milliseconds(seconds: f32) -> u64 {
    (seconds.max(0.0) as f64 * 1000.0).round() as u64
}

pub struct Ct2Engine {
    model: Whisper,
    vad: WhisperSpeechDetector,
    threads: usize,
    audio_after_vad_ms: Option<u64>,
}

impl Ct2Engine {
    pub fn load(model_dir: &Path, vad_path: &Path) -> Result<Self, String> {
        for name in [
            "model.bin",
            "config.json",
            "vocabulary.json",
            "tokenizer.json",
            "preprocessor_config.json",
        ] {
            if !model_dir.join(name).is_file() {
                return Err(format!(
                    "CT2 model directory {} is missing {name}",
                    model_dir.display()
                ));
            }
        }
        let vad = WhisperSpeechDetector::load(vad_path)?;
        let threads = threads();
        let config = Config {
            device: Device::CPU,
            compute_type: ComputeType::INT8,
            num_threads_per_replica: threads,
            ..Default::default()
        };
        let model = Whisper::new(model_dir, config).map_err(|e| e.to_string())?;
        if model.sampling_rate() != SAMPLE_RATE {
            return Err(format!(
                "CT2 model requires {} Hz; benchmark audio is 16000 Hz",
                model.sampling_rate()
            ));
        }
        Ok(Self {
            model,
            vad,
            threads,
            audio_after_vad_ms: None,
        })
    }

    fn speech_only(&mut self, samples_16k: &[f32]) -> Result<SpeechAudio, String> {
        let ranges = self.vad.speech_ranges(samples_16k)?;
        Ok(filter_speech(samples_16k, &ranges))
    }
}

impl SpeechEngine for Ct2Engine {
    fn backend(&self) -> &str {
        "CTranslate2 CPU int8"
    }

    fn thread_count(&self) -> usize {
        self.threads
    }

    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.audio_after_vad_ms
    }

    fn detect_language(
        &mut self,
        _samples_16k: &[f32],
        _cancel: &AtomicBool,
    ) -> Result<Language, String> {
        Err("ct2rs high-level Whisper API does not expose the detected language; use a Meeting with an explicit language for this spike".into())
    }

    fn has_speech(&mut self, samples_16k: &[f32]) -> bool {
        self.speech_only(samples_16k).map_or_else(
            |_| samples_16k.iter().any(|sample| sample.abs() > 0.01),
            |speech| !speech.samples.is_empty(),
        )
    }

    fn transcribe(
        &mut self,
        samples_16k: &[f32],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        self.audio_after_vad_ms = None;
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        if initial_prompt.is_some_and(|prompt| !prompt.is_empty()) {
            return Err(ProcessingError::Engine(
                "ct2rs segment generation does not accept an initial prompt".into(),
            ));
        }
        let speech = self
            .speech_only(samples_16k)
            .map_err(ProcessingError::Engine)?;
        self.audio_after_vad_ms = Some((speech.samples.len() as u64) * 1000 / SAMPLE_RATE as u64);
        if speech.samples.is_empty() {
            return Ok(Vec::new());
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let options = WhisperOptions {
            beam_size: 1,
            ..Default::default()
        };
        let generated = self
            .model
            .generate_segments(&speech.samples, Some(language.code()), &options)
            .map_err(|e| ProcessingError::Engine(e.to_string()))?;
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let raw = generated
            .into_iter()
            .filter_map(|segment| {
                let start_ms = speech.original_ms(milliseconds(segment.start));
                let end_ms = speech.original_ms(milliseconds(segment.end));
                if segment.text.trim().is_empty() || start_ms >= end_ms {
                    return None;
                }
                let words: Vec<RawWord> = segment
                    .words
                    .unwrap_or_default()
                    .into_iter()
                    .map(|word| RawWord {
                        start_ms: speech.original_ms(milliseconds(word.start)),
                        end_ms: speech.original_ms(milliseconds(word.end)),
                        text: word.word,
                    })
                    .collect();
                let normalized =
                    |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
                let joined = words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let valid_words = words
                    .iter()
                    .all(|word| word.start_ms.max(start_ms) < word.end_ms.min(end_ms));
                Some(RawSegment {
                    start_ms,
                    end_ms,
                    text: segment.text.clone(),
                    words: if valid_words && normalized(&joined) == normalized(&segment.text) {
                        words
                    } else {
                        Vec::new()
                    },
                })
            })
            .collect();
        progress(1.0);
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_filter_restores_original_ms_after_removed_silence() {
        let samples = vec![1.0; SAMPLE_RATE * 5];
        let speech = filter_speech(
            &samples,
            &[
                SAMPLE_RATE..2 * SAMPLE_RATE,
                4 * SAMPLE_RATE..5 * SAMPLE_RATE,
            ],
        );
        assert_eq!(
            speech.samples.len(),
            2 * SAMPLE_RATE + OVERLAP_SAMPLES + GAP_SAMPLES
        );
        assert_eq!(speech.original_ms(0), 1000);
        assert_eq!(speech.original_ms(1000), 2000);
        assert_eq!(speech.original_ms(1200), 4000);
        assert_eq!(speech.original_ms(2200), 5000);
    }

    #[test]
    fn no_speech_produces_empty_audio() {
        let speech = filter_speech(&[0.0; SAMPLE_RATE], &[]);
        assert!(speech.samples.is_empty());
        assert_eq!(speech.original_ms(0), 0);
    }
}

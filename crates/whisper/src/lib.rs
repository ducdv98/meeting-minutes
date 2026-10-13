use meeting_minutes_core::{
    diagnostics::{log_event, LogEvent},
    processing::{ProcessingError, SpeechEngine},
    transcript::{Language, RawSegment},
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};
use whisper_rs::{
    get_lang_id, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

pub struct WhisperEngine {
    context: WhisperContext,
    threads: usize,
    vad: Option<WhisperVadContext>,
    audio_after_vad_ms: Option<u64>,
}

const SAMPLE_RATE: usize = 16_000;
const OVERLAP_SAMPLES: usize = SAMPLE_RATE / 10;
const GAP_SAMPLES: usize = SAMPLE_RATE / 10;
const LANGUAGE_SCAN_SAMPLES: usize = 5 * 60 * SAMPLE_RATE;

struct TimeMapping {
    processed_start: usize,
    processed_end: usize,
    original_start: usize,
    original_end: usize,
    /// Inserted silence standing in for a removed pause; times inside it snap to a speech edge.
    gap: bool,
}

struct SpeechAudio {
    samples: Vec<f32>,
    mapping: Vec<TimeMapping>,
}

impl SpeechAudio {
    fn original_ms(&self, processed_ms: u64) -> u64 {
        let sample = (processed_ms as usize).saturating_mul(SAMPLE_RATE) / 1000;
        let Some(part) = self
            .mapping
            .iter()
            .find(|part| sample <= part.processed_end)
        else {
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

// whisper-rs 0.16 calls whisper_full_with_state(), which does not run the VAD branch
// in whisper_full(). Build the same speech-only buffer here and retain its time map.
fn filter_speech(samples: &[f32], segments: &[(usize, usize)]) -> SpeechAudio {
    let mut filtered = SpeechAudio {
        samples: Vec::new(),
        mapping: Vec::new(),
    };
    for (index, &(start, end)) in segments.iter().enumerate() {
        let start = start.min(samples.len());
        let end = end.min(samples.len());
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
        if let Some(&(next_start, _)) = segments.get(index + 1) {
            let processed_start = filtered.samples.len();
            filtered.samples.resize(processed_start + GAP_SAMPLES, 0.0);
            filtered.mapping.push(TimeMapping {
                processed_start,
                processed_end: filtered.samples.len(),
                original_start: copy_end,
                original_end: next_start.min(samples.len()),
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
    thread_count(sysinfo::System::physical_core_count(), logical)
}

fn thread_count(physical: Option<usize>, logical: usize) -> usize {
    physical
        .filter(|&count| count > 0)
        .unwrap_or(logical / 2)
        .clamp(1, 8)
}

impl WhisperEngine {
    /// Speech-only audio with its time map, or None when VAD is unavailable.
    fn speech_only(&mut self, samples_16k: &[f32]) -> Option<SpeechAudio> {
        let segments = self
            .vad
            .as_mut()?
            .segments_from_samples(WhisperVadParams::default(), samples_16k);
        let Ok(segments) = segments else {
            self.vad = None;
            log_event(LogEvent::VadUnavailable);
            return None;
        };
        // VAD segment times are in centiseconds.
        let ranges = segments
            .map(|segment| {
                (
                    (segment.start.max(0.0) * (SAMPLE_RATE as f32 / 100.0)) as usize,
                    (segment.end.max(0.0) * (SAMPLE_RATE as f32 / 100.0)) as usize,
                )
            })
            .collect::<Vec<_>>();
        Some(filter_speech(samples_16k, &ranges))
    }

    pub fn system_info() -> &'static str {
        whisper_rs::print_system_info()
    }

    pub fn backend_description() -> &'static str {
        "CPU"
    }

    pub fn load(model_path: &Path) -> Result<Self, String> {
        Self::load_with_vad(model_path, None)
    }

    pub fn load_with_vad(model_path: &Path, vad_path: Option<&Path>) -> Result<Self, String> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(false);
        let vad = vad_path.and_then(|path| {
            let context = path.to_str().and_then(|text| {
                WhisperVadContext::new(text, WhisperVadContextParams::default()).ok()
            });
            if context.is_none() {
                log_event(LogEvent::VadUnavailable);
            }
            context
        });
        WhisperContext::new_with_params(model_path, params)
            .map(|context| Self {
                context,
                threads: threads(),
                vad,
                audio_after_vad_ms: None,
            })
            .map_err(|e| e.to_string())
    }
}

impl SpeechEngine for WhisperEngine {
    fn backend(&self) -> &str {
        Self::backend_description()
    }

    fn thread_count(&self) -> usize {
        self.threads
    }

    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.audio_after_vad_ms
    }

    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String> {
        let mut state = self.context.create_state().map_err(|e| e.to_string())?;
        // Detect from the first speech, so a Meeting that opens with a long pause still detects.
        let opening = &samples_16k[..samples_16k.len().min(LANGUAGE_SCAN_SAMPLES)];
        let speech = self
            .speech_only(opening)
            .filter(|speech| !speech.samples.is_empty());
        let audio = speech
            .as_ref()
            .map_or(samples_16k, |speech| &speech.samples);
        let first = &audio[..audio.len().min(30 * SAMPLE_RATE)];
        state
            .pcm_to_mel(first, self.threads)
            .map_err(|e| e.to_string())?;
        let (_, probabilities) = state
            .lang_detect(0, self.threads)
            .map_err(|e| e.to_string())?;
        let en = get_lang_id("en").ok_or("English language id unavailable")? as usize;
        let vi = get_lang_id("vi").ok_or("Vietnamese language id unavailable")? as usize;
        if probabilities.get(vi).unwrap_or(&0.0) > probabilities.get(en).unwrap_or(&0.0) {
            Ok(Language::Vi)
        } else {
            Ok(Language::En)
        }
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
        let speech = self.speech_only(samples_16k);
        if let Some(speech) = &speech {
            self.audio_after_vad_ms =
                Some((speech.samples.len() as u64) * 1000 / SAMPLE_RATE as u64);
            if speech.samples.is_empty() {
                return Ok(Vec::new());
            }
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let whisper_samples = speech.as_ref().map_or(samples_16k, |audio| &audio.samples);
        let mut state = self
            .context
            .create_state()
            .map_err(|e| ProcessingError::Engine(e.to_string()))?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(match language {
            Language::En => "en",
            Language::Vi => "vi",
        }));
        params.set_translate(false);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        if let Some(prompt) = initial_prompt {
            params.set_initial_prompt(prompt);
        }
        params.set_n_threads(self.threads as i32);
        let (sender, receiver) = mpsc::channel::<i32>();
        params.set_progress_callback_safe(move |percent| {
            let _ = sender.send(percent);
        });
        let abort = Arc::new(AtomicBool::new(false));
        let abort_for_callback = Arc::clone(&abort);
        // whisper-rs stores a boxed trait object; matching F to that type keeps its trampoline sound.
        let abort_callback: Box<dyn FnMut() -> bool> =
            Box::new(move || abort_for_callback.load(Ordering::SeqCst));
        params.set_abort_callback_safe(abort_callback);
        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| state.full(params, whisper_samples));
            while !worker.is_finished() {
                abort.store(cancel.load(Ordering::SeqCst), Ordering::SeqCst);
                if let Ok(percent) = receiver.recv_timeout(Duration::from_millis(25)) {
                    progress((percent as f32 / 100.0).clamp(0.0, 1.0));
                }
            }
            for percent in receiver.try_iter() {
                progress((percent as f32 / 100.0).clamp(0.0, 1.0));
            }
            worker
                .join()
                .map_err(|_| ProcessingError::Engine("Whisper worker panicked".into()))?
                .map_err(|e| ProcessingError::Engine(e.to_string()))
        });
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        result?;
        state
            .as_iter()
            .map(|segment| {
                Ok(RawSegment {
                    start_ms: speech.as_ref().map_or_else(
                        || (segment.start_timestamp().max(0) as u64) * 10,
                        |audio| audio.original_ms((segment.start_timestamp().max(0) as u64) * 10),
                    ),
                    end_ms: speech.as_ref().map_or_else(
                        || (segment.end_timestamp().max(0) as u64) * 10,
                        |audio| audio.original_ms((segment.end_timestamp().max(0) as u64) * 10),
                    ),
                    text: segment
                        .to_str_lossy()
                        .map_err(|e| ProcessingError::Engine(e.to_string()))?
                        .into_owned(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_after_long_silence_maps_back_to_original_timeline() {
        let samples = vec![0.0; 125 * SAMPLE_RATE];
        let speech = filter_speech(
            &samples,
            &[(0, SAMPLE_RATE), (120 * SAMPLE_RATE, 122 * SAMPLE_RATE)],
        );
        assert!(speech.samples.len() < samples.len() / 10);
        for (processed_ms, original_ms) in [(1_200, 120_000), (1_500, 120_300), (2_000, 120_800)] {
            assert_eq!(speech.original_ms(processed_ms), original_ms);
        }
    }

    #[test]
    fn copied_overlap_keeps_its_real_time() {
        let samples = vec![0.0; 125 * SAMPLE_RATE];
        let speech = filter_speech(
            &samples,
            &[(0, SAMPLE_RATE), (120 * SAMPLE_RATE, 122 * SAMPLE_RATE)],
        );
        // The first range is copied with 100 ms of overlap: 0–1,100 ms play at real time.
        for ms in [500, 1_000, 1_100] {
            assert_eq!(speech.original_ms(ms), ms);
        }
    }

    #[test]
    fn times_inside_an_inserted_gap_snap_to_the_nearest_speech_edge() {
        let samples = vec![0.0; 125 * SAMPLE_RATE];
        let speech = filter_speech(
            &samples,
            &[(0, SAMPLE_RATE), (120 * SAMPLE_RATE, 122 * SAMPLE_RATE)],
        );
        // The inserted gap spans processed 1,100–1,200 ms.
        assert_eq!(speech.original_ms(1_120), 1_100);
        assert_eq!(speech.original_ms(1_180), 120_000);
    }

    #[test]
    fn no_detected_speech_has_no_audio_for_whisper() {
        let speech = filter_speech(&vec![0.0; SAMPLE_RATE], &[]);
        assert!(speech.samples.is_empty());
    }

    #[test]
    #[ignore = "set MM_VAD_MODEL to a downloaded Silero model"]
    fn native_vad_rejects_silence() {
        let path = std::env::var("MM_VAD_MODEL").expect("set MM_VAD_MODEL");
        let mut vad = WhisperVadContext::new(&path, WhisperVadContextParams::default()).unwrap();
        let segments = vad
            .segments_from_samples(WhisperVadParams::default(), &vec![0.0; 5 * SAMPLE_RATE])
            .unwrap();
        assert_eq!(segments.num_segments(), 0);
    }

    #[test]
    #[ignore = "set MM_WHISPER_MODEL and MM_VAD_MODEL to local model files"]
    fn vad_does_not_invent_text_during_a_two_minute_pause() {
        let whisper_path = std::env::var("MM_WHISPER_MODEL").expect("set MM_WHISPER_MODEL");
        let vad_path = std::env::var("MM_VAD_MODEL").expect("set MM_VAD_MODEL");
        let mut engine =
            WhisperEngine::load_with_vad(Path::new(&whisper_path), Some(Path::new(&vad_path)))
                .unwrap();
        let segments = engine
            .transcribe(
                &vec![0.0; 120 * SAMPLE_RATE],
                Language::En,
                None,
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .unwrap();
        assert!(segments.is_empty());
        assert_eq!(engine.audio_after_vad_ms(), Some(0));
    }

    #[test]
    #[ignore = "set MM_WHISPER_MODEL to a local model file"]
    fn missing_vad_model_still_transcribes() {
        let whisper_path = std::env::var("MM_WHISPER_MODEL").expect("set MM_WHISPER_MODEL");
        let mut engine = WhisperEngine::load_with_vad(
            Path::new(&whisper_path),
            Some(Path::new("missing-vad-model.bin")),
        )
        .unwrap();
        engine
            .transcribe(
                &vec![0.0; 2 * SAMPLE_RATE],
                Language::En,
                None,
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(engine.audio_after_vad_ms(), None);
    }

    #[test]
    fn uses_physical_cores_with_a_cap_of_eight() {
        assert_eq!(thread_count(Some(1), 16), 1);
        assert_eq!(thread_count(Some(4), 16), 4);
        assert_eq!(thread_count(Some(16), 16), 8);
    }

    #[test]
    fn falls_back_to_half_the_logical_cores() {
        assert_eq!(thread_count(None, 16), 8);
        assert_eq!(thread_count(None, 6), 3);
        assert_eq!(thread_count(None, 1), 1);
        assert_eq!(thread_count(Some(0), 0), 1);
    }

    #[test]
    #[ignore]
    fn transcribes_silence_when_a_local_model_is_supplied() {
        let model =
            std::env::var("MM_WHISPER_MODEL").expect("set MM_WHISPER_MODEL to a ggml model file");
        let mut engine = WhisperEngine::load(Path::new(&model)).unwrap();
        engine
            .transcribe(
                &vec![0.0; 32_000],
                Language::En,
                None,
                &mut |_| {},
                &AtomicBool::new(false),
            )
            .unwrap();
    }
}

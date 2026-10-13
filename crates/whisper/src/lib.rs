use meeting_minutes_core::{
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
};

pub struct WhisperEngine {
    context: WhisperContext,
    threads: usize,
}

fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).clamp(1, 8))
        .unwrap_or(1)
}

impl WhisperEngine {
    pub fn system_info() -> &'static str {
        whisper_rs::print_system_info()
    }

    pub fn backend_description() -> &'static str {
        "CPU"
    }

    pub fn load(model_path: &Path) -> Result<Self, String> {
        let mut params = WhisperContextParameters::default();
        params.use_gpu(false);
        WhisperContext::new_with_params(model_path, params)
            .map(|context| Self {
                context,
                threads: threads(),
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

    fn detect_language(&mut self, samples_16k: &[f32]) -> Result<Language, String> {
        let mut state = self.context.create_state().map_err(|e| e.to_string())?;
        let first = &samples_16k[..samples_16k.len().min(30 * 16_000)];
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
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
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
            let worker = scope.spawn(|| state.full(params, samples_16k));
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
                    start_ms: (segment.start_timestamp().max(0) as u64) * 10,
                    end_ms: (segment.end_timestamp().max(0) as u64) * 10,
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

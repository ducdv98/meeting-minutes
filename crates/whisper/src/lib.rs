use meeting_minutes_core::{
    diagnostics::{log_event, LogEvent},
    processing::{
        PowerState, ProcessingError, SpeechEngine, StreamTranscription, LANGUAGE_SCAN_SAMPLES,
    },
    transcript::{Language, RawSegment, RawWord},
};
pub mod benchmark;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use whisper_rs::{
    get_lang_id, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    WhisperState, WhisperVadContext, WhisperVadContextParams, WhisperVadParams,
};

pub struct WhisperEngine {
    context: WhisperContext,
    backend: &'static str,
    device: Option<String>,
    power: PowerState,
    threads: usize,
    vad: Option<WhisperVadContext>,
    audio_after_vad_ms: Option<u64>,
}

static LAST_BACKEND: OnceLock<Mutex<String>> = OnceLock::new();

const SAMPLE_RATE: usize = 16_000;
const OVERLAP_SAMPLES: usize = SAMPLE_RATE / 10;
const GAP_SAMPLES: usize = SAMPLE_RATE / 10;

#[cfg(any(windows, test))]
#[derive(Debug, PartialEq, Eq)]
enum BackendPlan {
    Cpu,
    Vulkan(i32),
}

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceType {
    Discrete,
    Integrated,
    Cpu,
}

#[cfg(any(windows, test))]
fn plan_backend(devices: &[(i32, DeviceType, usize)], power: PowerState) -> BackendPlan {
    let mut best = None;
    for &(id, kind, memory) in devices {
        let rank = match kind {
            DeviceType::Discrete if power != PowerState::Battery => 2,
            DeviceType::Integrated => 1,
            _ => continue,
        };
        if best.is_none_or(|(_, best_rank, best_memory)| (rank, memory) > (best_rank, best_memory))
        {
            best = Some((id, rank, memory));
        }
    }
    best.map_or(BackendPlan::Cpu, |(id, _, _)| BackendPlan::Vulkan(id))
}

#[cfg(windows)]
fn available_devices() -> Vec<whisper_rs::vulkan::VkDeviceInfo> {
    // ggml's Vulkan initializer throws a C++ exception when vkCreateInstance fails.
    // Check the loader and ICD through Vulkan's fallible API before crossing that FFI.
    if vulkan_devices().is_some() {
        whisper_rs::vulkan::list_devices()
    } else {
        Vec::new()
    }
}

#[cfg(windows)]
fn vulkan_devices() -> Option<Vec<(String, DeviceType)>> {
    use ash::{vk, Entry};
    use std::ffi::CStr;

    let Ok(entry) = (unsafe { Entry::load() }) else {
        return None;
    };
    let Ok(Some(version)) = (unsafe { entry.try_enumerate_instance_version() }) else {
        return None;
    };
    if version < vk::API_VERSION_1_2 {
        return None;
    }
    let application = vk::ApplicationInfo {
        api_version: vk::API_VERSION_1_2,
        ..Default::default()
    };
    let create = vk::InstanceCreateInfo {
        p_application_info: &application,
        ..Default::default()
    };
    let instance = unsafe { entry.create_instance(&create, None) }.ok()?;
    let devices = unsafe { instance.enumerate_physical_devices() }
        .ok()
        .map(|physical| {
            physical
                .iter()
                .map(|&device| {
                    let properties = unsafe { instance.get_physical_device_properties(device) };
                    let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
                        .to_string_lossy()
                        .into_owned();
                    let kind = match properties.device_type {
                        vk::PhysicalDeviceType::DISCRETE_GPU => DeviceType::Discrete,
                        vk::PhysicalDeviceType::INTEGRATED_GPU => DeviceType::Integrated,
                        _ => DeviceType::Cpu,
                    };
                    (name, kind)
                })
                .collect()
        });
    unsafe { instance.destroy_instance(None) };
    devices
}

#[cfg(windows)]
fn selected_device(devices: &[whisper_rs::vulkan::VkDeviceInfo], power: PowerState) -> BackendPlan {
    plan_backend(&ranked_devices(devices), power)
}

/// ggml's devices with the Vulkan device type, matched by name in enumeration order.
#[cfg(windows)]
fn ranked_devices(devices: &[whisper_rs::vulkan::VkDeviceInfo]) -> Vec<(i32, DeviceType, usize)> {
    let Some(physical) = vulkan_devices() else {
        return Vec::new();
    };
    let mut used = vec![false; physical.len()];
    devices
        .iter()
        .filter_map(|device| {
            let index = physical
                .iter()
                .enumerate()
                .position(|(index, (name, _))| !used[index] && name == &device.name)?;
            used[index] = true;
            Some((device.id, physical[index].1, device.vram.total))
        })
        .collect()
}

/// Whether a plan runs on a discrete GPU, the only device background transcription uses (ADR 0010).
#[cfg(any(windows, test))]
fn runs_on_discrete_gpu(plan: &BackendPlan, devices: &[(i32, DeviceType, usize)]) -> bool {
    matches!(plan, BackendPlan::Vulkan(id) if devices.iter().any(|&(device, kind, _)| device == *id && kind == DeviceType::Discrete))
}

#[cfg(any(windows, test))]
fn requested_plan(
    setting: Option<&str>,
    available_ids: &[i32],
    automatic: BackendPlan,
) -> Result<BackendPlan, String> {
    match setting {
        None | Some("auto") => Ok(automatic),
        Some("cpu") => Ok(BackendPlan::Cpu),
        Some(value) => {
            let id = value
                .parse::<i32>()
                .map_err(|_| format!("Invalid MM_WHISPER_DEVICE: {value}"))?;
            if available_ids.contains(&id) {
                Ok(BackendPlan::Vulkan(id))
            } else {
                Err(format!("Vulkan device {id} is unavailable"))
            }
        }
    }
}

// whisper.cpp#3750: in static MSVC builds the Vulkan backend can fail to register during
// static initialization, so ggml silently loads on CPU. After list_devices() has set up
// the Vulkan instance, registering it again succeeds.
#[cfg(windows)]
fn ensure_vulkan_registered() -> bool {
    unsafe {
        if !whisper_rs_sys::ggml_backend_reg_by_name(c"Vulkan".as_ptr()).is_null() {
            return true;
        }
        let registration = whisper_rs_sys::ggml_backend_vk_reg();
        if registration.is_null() {
            return false;
        }
        whisper_rs_sys::ggml_backend_register(registration);
    }
    true
}

fn load_context(
    model_path: &Path,
) -> Result<(WhisperContext, &'static str, Option<String>, PowerState), String> {
    let power = PowerState::current();
    #[cfg(windows)]
    {
        let devices = available_devices();
        let setting = std::env::var("MM_WHISPER_DEVICE").ok();
        let ids = devices.iter().map(|device| device.id).collect::<Vec<_>>();
        let selected = requested_plan(setting.as_deref(), &ids, selected_device(&devices, power))?;
        match selected {
            BackendPlan::Vulkan(id) if ensure_vulkan_registered() => {
                let mut params = WhisperContextParameters::default();
                params.use_gpu(true).gpu_device(id).flash_attn(true);
                if let Ok(context) = WhisperContext::new_with_params(model_path, params) {
                    let name = devices
                        .iter()
                        .find(|device| device.id == id)
                        .and_then(|device| meeting_minutes_core::diagnostics::safe_device_name(&device.name).map(str::to_owned));
                    return Ok((context, "Vulkan", name, power));
                }
                log_event(LogEvent::GpuLoadFailed);
            }
            _ => log_event(LogEvent::GpuUnavailable),
        }
    }

    #[cfg(not(windows))]
    if let Ok(value) = std::env::var("MM_WHISPER_DEVICE") {
        if value != "auto" && value != "cpu" {
            return Err("Vulkan device selection needs a Windows Vulkan build".into());
        }
    }

    let mut params = WhisperContextParameters::default();
    params.use_gpu(false);
    WhisperContext::new_with_params(model_path, params)
        .map(|context| (context, "CPU", None, power))
        .map_err(|error| error.to_string())
}

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

    pub fn available_device_descriptions() -> Vec<String> {
        #[cfg(windows)]
        {
            return available_devices()
                .iter()
                .map(|device| format!("{}: {}", device.id, device.name))
                .collect();
        }
        #[cfg(not(windows))]
        Vec::new()
    }

    /// Whether transcription on AC power would run on a discrete GPU, the only device background
    /// transcription during a Recording uses (ADR 0010). The power state is the caller's to check.
    pub fn discrete_gpu_available() -> bool {
        #[cfg(windows)]
        {
            let ranked = ranked_devices(&available_devices());
            return runs_on_discrete_gpu(&plan_backend(&ranked, PowerState::Ac), &ranked);
        }
        #[cfg(not(windows))]
        false
    }

    pub fn backend_description() -> String {
        if let Some(backend) = LAST_BACKEND.get().and_then(|last| last.lock().ok()) {
            return backend.clone();
        }
        let power = PowerState::current();
        #[cfg(windows)]
        {
            let devices = available_devices();
            let setting = std::env::var("MM_WHISPER_DEVICE").ok();
            let ids = devices.iter().map(|device| device.id).collect::<Vec<_>>();
            let selected = requested_plan(setting.as_deref(), &ids, selected_device(&devices, power));
            if let Ok(BackendPlan::Vulkan(id)) = selected {
                if let Some(device) = devices.iter().find(|device| device.id == id) {
                    let name =
                        meeting_minutes_core::diagnostics::safe_device_name(&device.name).map(str::to_owned).unwrap_or_else(|| "none".to_owned());
                    return format!("Vulkan device=\"{name}\" power={}", power.label());
                }
            }
        }
        format!("CPU device=\"none\" power={}", power.label())
    }

    pub fn load(model_path: &Path) -> Result<Self, String> {
        Self::load_with_vad(model_path, None)
    }

    pub fn load_with_vad(model_path: &Path, vad_path: Option<&Path>) -> Result<Self, String> {
        let vad = vad_path.and_then(|path| {
            let context = path.to_str().and_then(|text| {
                WhisperVadContext::new(text, WhisperVadContextParams::default()).ok()
            });
            if context.is_none() {
                log_event(LogEvent::VadUnavailable);
            }
            context
        });
        load_context(model_path).map(|(context, backend, device, power)| {
            let description = format!(
                "{backend} device=\"{}\" power={}",
                device.as_deref().unwrap_or("none"),
                power.label()
            );
            *LAST_BACKEND.get_or_init(|| Mutex::new(String::new())).lock().unwrap() = description;
            Self {
                context,
                backend,
                device,
                power,
                threads: threads(),
                vad,
                audio_after_vad_ms: None,
            }
        })
    }
}

#[derive(Clone, Copy)]
struct TranscriptionSettings<'a> {
    language: Language,
    prompt: Option<&'a str>,
    threads: usize,
}

struct TokenPiece {
    text: Vec<u8>,
    start_ms: u64,
    end_ms: u64,
}

fn join_token_pieces(pieces: Vec<TokenPiece>) -> Vec<RawWord> {
    let mut words = Vec::new();
    let mut bytes = Vec::new();
    let mut start = 0;
    let mut end = 0;
    let flush = |words: &mut Vec<RawWord>, bytes: &mut Vec<u8>, start, end| {
        if !bytes.is_empty() {
            words.push(RawWord {
                start_ms: start,
                end_ms: end,
                text: String::from_utf8_lossy(bytes).into_owned(),
            });
            bytes.clear();
        }
    };
    for piece in pieces {
        if piece.text.starts_with(b"<|") && piece.text.ends_with(b"|>") {
            continue;
        }
        for byte in piece.text {
            if byte.is_ascii_whitespace() {
                flush(&mut words, &mut bytes, start, end);
            } else {
                if bytes.is_empty() {
                    start = piece.start_ms;
                }
                bytes.push(byte);
                end = end.max(piece.end_ms);
            }
        }
    }
    flush(&mut words, &mut bytes, start, end);
    words
}

fn transcribe_with_state(
    mut state: WhisperState,
    samples_16k: &[f32],
    speech: Option<SpeechAudio>,
    settings: TranscriptionSettings<'_>,
    progress: &mut dyn FnMut(f32),
    cancel: &AtomicBool,
) -> Result<Vec<RawSegment>, ProcessingError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
    if speech
        .as_ref()
        .is_some_and(|audio| audio.samples.is_empty())
    {
        return Ok(Vec::new());
    }
    let whisper_samples = speech.as_ref().map_or(samples_16k, |audio| &audio.samples);
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(settings.language.code()));
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_token_timestamps(true);
    if let Some(prompt) = settings.prompt {
        params.set_initial_prompt(prompt);
    }
    params.set_n_threads(settings.threads as i32);
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
            let map_time = |centiseconds: i64| {
                let ms = (centiseconds.max(0) as u64) * 10;
                speech.as_ref().map_or(ms, |audio| audio.original_ms(ms))
            };
            let start_ms = map_time(segment.start_timestamp());
            let end_ms = map_time(segment.end_timestamp());
            let text = segment
                .to_str_lossy()
                .map_err(|e| ProcessingError::Engine(e.to_string()))?
                .into_owned();
            let pieces = (0..segment.n_tokens())
                .map(|index| {
                    let token = segment.get_token(index).unwrap();
                    let data = token.token_data();
                    Ok(TokenPiece {
                        text: token
                            .to_bytes()
                            .map_err(|e| ProcessingError::Engine(e.to_string()))?
                            .to_vec(),
                        start_ms: map_time(data.t0),
                        end_ms: map_time(data.t1),
                    })
                })
                .collect::<Result<Vec<_>, ProcessingError>>()?;
            let words = join_token_pieces(pieces);
            let normalized = |value: &str| value.split_whitespace().collect::<Vec<_>>().join(" ");
            let joined = words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let valid_words = words
                .iter()
                .all(|word| word.start_ms.max(start_ms) < word.end_ms.min(end_ms));
            Ok(RawSegment {
                start_ms,
                end_ms,
                words: if valid_words && normalized(&joined) == normalized(&text) {
                    words
                } else {
                    Vec::new()
                },
                text,
            })
        })
        .collect()
}

impl SpeechEngine for WhisperEngine {
    fn backend(&self) -> &str {
        self.backend
    }

    fn thread_count(&self) -> usize {
        self.threads
    }

    fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }

    fn power_state(&self) -> PowerState {
        self.power
    }

    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.audio_after_vad_ms
    }

    fn detect_language(
        &mut self,
        samples_16k: &[f32],
        _cancel: &AtomicBool,
    ) -> Result<Language, String> {
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
        Language::ALL
            .into_iter()
            .filter_map(|language| {
                get_lang_id(language.code()).and_then(|id| {
                    probabilities
                        .get(id as usize)
                        .map(|probability| (language, *probability))
                })
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(language, _)| language)
            .ok_or_else(|| "Supported language ids unavailable".to_owned())
    }

    fn has_speech(&mut self, samples_16k: &[f32]) -> bool {
        self.speech_only(samples_16k).map_or_else(
            || samples_16k.iter().any(|sample| sample.abs() > 0.01),
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
        let state = self
            .context
            .create_state()
            .map_err(|e| ProcessingError::Engine(e.to_string()))?;
        transcribe_with_state(
            state,
            samples_16k,
            speech,
            TranscriptionSettings {
                language,
                prompt: initial_prompt,
                threads: self.threads,
            },
            progress,
            cancel,
        )
    }

    fn transcribe_streams(
        &mut self,
        streams: &[&[f32]],
        language: Language,
        initial_prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<StreamTranscription>, ProcessingError> {
        self.audio_after_vad_ms = None;
        // Speech is found in every Source first, so progress can weigh each by the speech Whisper
        // transcribes; a mostly silent Microphone then counts for little.
        let mut speech = Vec::with_capacity(streams.len());
        for (index, samples) in streams.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            speech.push(self.speech_only(samples));
            progress(StreamProgress::scanned(index + 1, streams.len()));
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let speech_ms = speech
            .iter()
            .map(|audio| {
                audio
                    .as_ref()
                    .map(|audio| audio.samples.len() as u64 * 1000 / SAMPLE_RATE as u64)
            })
            .collect::<Vec<_>>();
        let work = streams
            .iter()
            .zip(&speech)
            .map(|(samples, audio)| {
                audio
                    .as_ref()
                    .map_or(samples.len(), |audio| audio.samples.len())
            })
            .collect::<Vec<_>>();
        let weights = StreamProgress::new(&work);
        let settings = TranscriptionSettings {
            language,
            prompt: initial_prompt,
            threads: self.threads,
        };
        let parallel_states = if streams.len() == 2 && self.backend == "Vulkan" {
            let first = self
                .context
                .create_state()
                .map_err(|e| ProcessingError::Engine(e.to_string()))?;
            self.context
                .create_state()
                .ok()
                .map(|second| (first, second))
        } else {
            None
        };
        let segments = if let Some((first_state, second_state)) = parallel_states {
            let mut speech = speech.into_iter();
            let (first_speech, second_speech) = (speech.next().flatten(), speech.next().flatten());
            let (sender, receiver) = mpsc::channel::<(usize, f32)>();
            let (first, second) = std::thread::scope(|scope| {
                let first_sender = sender.clone();
                let first = scope.spawn(move || {
                    let result = transcribe_with_state(
                        first_state,
                        streams[0],
                        first_speech,
                        settings,
                        &mut |fraction| {
                            let _ = first_sender.send((0, fraction));
                        },
                        cancel,
                    );
                    let _ = first_sender.send((0, 1.0));
                    result
                });
                let second = scope.spawn(move || {
                    let result = transcribe_with_state(
                        second_state,
                        streams[1],
                        second_speech,
                        settings,
                        &mut |fraction| {
                            let _ = sender.send((1, fraction));
                        },
                        cancel,
                    );
                    let _ = sender.send((1, 1.0));
                    result
                });
                let mut fractions = [0.0_f32; 2];
                let mut reported = StreamProgress::scanned(2, 2);
                let mut record = |index: usize, fraction: f32| {
                    fractions[index] = fractions[index].max(fraction.clamp(0.0, 1.0));
                    reported = reported.max(weights.overall(&fractions));
                    reported
                };
                while !first.is_finished() || !second.is_finished() {
                    if let Ok((index, fraction)) = receiver.recv_timeout(Duration::from_millis(25))
                    {
                        progress(record(index, fraction));
                    }
                }
                for (index, fraction) in receiver.try_iter() {
                    record(index, fraction);
                }
                progress(record(0, 0.0));
                let first = first
                    .join()
                    .map_err(|_| ProcessingError::Engine("Whisper worker panicked".into()))?;
                let second = second
                    .join()
                    .map_err(|_| ProcessingError::Engine("Whisper worker panicked".into()))?;
                Ok::<_, ProcessingError>((first?, second?))
            })?;
            vec![first, second]
        } else {
            let mut fractions = vec![0.0_f32; streams.len()];
            let mut all = Vec::with_capacity(streams.len());
            for (index, (samples, audio)) in streams.iter().zip(speech).enumerate() {
                if cancel.load(Ordering::SeqCst) {
                    return Err(ProcessingError::Cancelled);
                }
                let state = self
                    .context
                    .create_state()
                    .map_err(|e| ProcessingError::Engine(e.to_string()))?;
                all.push(transcribe_with_state(
                    state,
                    samples,
                    audio,
                    settings,
                    &mut |fraction| {
                        fractions[index] = fractions[index].max(fraction.clamp(0.0, 1.0));
                        progress(weights.overall(&fractions));
                    },
                    cancel,
                )?);
                fractions[index] = 1.0;
                progress(weights.overall(&fractions));
            }
            all
        };
        self.audio_after_vad_ms = speech_ms.iter().copied().sum();
        Ok(segments
            .into_iter()
            .zip(speech_ms)
            .map(|(segments, audio_after_vad_ms)| StreamTranscription {
                segments,
                audio_after_vad_ms,
            })
            .collect())
    }
}

/// Share of the progress bar spent finding speech in each Source before Whisper starts.
const SPEECH_SCAN_SHARE: f32 = 0.25;

/// Overall progress across Sources, each weighted by the audio Whisper transcribes from it.
struct StreamProgress {
    weights: Vec<f32>,
}

impl StreamProgress {
    fn new(work: &[usize]) -> Self {
        let total = work.iter().sum::<usize>();
        Self {
            weights: work
                .iter()
                .map(|&samples| {
                    if total == 0 {
                        0.0
                    } else {
                        samples as f32 / total as f32
                    }
                })
                .collect(),
        }
    }

    /// Progress after speech has been found in `done` of `total` Sources.
    fn scanned(done: usize, total: usize) -> f32 {
        SPEECH_SCAN_SHARE * done as f32 / total.max(1) as f32
    }

    fn overall(&self, fractions: &[f32]) -> f32 {
        if self.weights.iter().all(|&weight| weight == 0.0) {
            return 1.0;
        }
        let transcribed = self
            .weights
            .iter()
            .zip(fractions)
            .map(|(weight, fraction)| weight * fraction.clamp(0.0, 1.0))
            .sum::<f32>();
        (SPEECH_SCAN_SHARE + (1.0 - SPEECH_SCAN_SHARE) * transcribed).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_source_finishing_first_does_not_jump_progress_to_half() {
        // Equal-length Sources: the Microphone has no speech, System audio has 20 minutes of it.
        let progress = StreamProgress::new(&[0, 20 * 60 * SAMPLE_RATE]);
        let microphone_done = progress.overall(&[1.0, 0.0]);
        assert!(
            microphone_done <= SPEECH_SCAN_SHARE + 0.01,
            "{microphone_done}"
        );
        assert!(
            (progress.overall(&[1.0, 0.5]) - (SPEECH_SCAN_SHARE + (1.0 - SPEECH_SCAN_SHARE) * 0.5))
                .abs()
                < 1e-4
        );
        assert_eq!(progress.overall(&[1.0, 1.0]), 1.0);
    }

    #[test]
    fn progress_moves_while_speech_is_found_before_whisper_starts() {
        assert_eq!(StreamProgress::scanned(0, 2), 0.0);
        assert!(StreamProgress::scanned(1, 2) > 0.0);
        assert_eq!(StreamProgress::scanned(2, 2), SPEECH_SCAN_SHARE);
        assert_eq!(
            StreamProgress::new(&[5, 15]).overall(&[0.0, 0.0]),
            SPEECH_SCAN_SHARE
        );
    }

    #[test]
    fn sources_with_no_speech_at_all_complete_the_progress() {
        assert_eq!(StreamProgress::new(&[0, 0]).overall(&[0.0, 0.0]), 1.0);
    }

    #[test]
    fn token_pieces_join_subwords_and_keep_word_timestamps() {
        let pieces = vec![
            TokenPiece {
                text: b" Hel".to_vec(),
                start_ms: 0,
                end_ms: 400,
            },
            TokenPiece {
                text: b"lo".to_vec(),
                start_ms: 400,
                end_ms: 600,
            },
            TokenPiece {
                text: b" world".to_vec(),
                start_ms: 700,
                end_ms: 1100,
            },
            TokenPiece {
                text: b"!".to_vec(),
                start_ms: 1100,
                end_ms: 1200,
            },
        ];
        let words = join_token_pieces(pieces);
        assert_eq!(
            words
                .iter()
                .map(|word| (word.start_ms, word.end_ms, word.text.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, 600, "Hello"), (700, 1200, "world!")]
        );
    }

    #[test]
    fn token_pieces_can_split_a_vietnamese_character_between_tokens() {
        let words = join_token_pieces(vec![
            TokenPiece {
                text: b" \xe1\xba".to_vec(),
                start_ms: 100,
                end_ms: 200,
            },
            TokenPiece {
                text: b"\xa1.".to_vec(),
                start_ms: 200,
                end_ms: 300,
            },
        ]);
        assert_eq!(words[0].text, "ạ.");
        assert_eq!((words[0].start_ms, words[0].end_ms), (100, 300));
    }

    #[test]
    fn chooses_largest_gpu_and_keeps_ggml_order_for_ties() {
        assert_eq!(
            plan_backend(
                &[
                    (0, DeviceType::Discrete, 4),
                    (1, DeviceType::Discrete, 8),
                    (2, DeviceType::Discrete, 2)
                ],
                PowerState::Ac
            ),
            BackendPlan::Vulkan(1)
        );
        assert_eq!(
            plan_backend(
                &[
                    (0, DeviceType::Integrated, 8),
                    (1, DeviceType::Integrated, 8)
                ],
                PowerState::Ac
            ),
            BackendPlan::Vulkan(0)
        );
    }

    #[test]
    fn background_transcription_needs_the_discrete_gpu_on_ac_power() {
        let devices = [
            (0, DeviceType::Integrated, 16),
            (1, DeviceType::Discrete, 6),
        ];
        assert!(runs_on_discrete_gpu(&plan_backend(&devices, PowerState::Ac), &devices));
        assert!(!runs_on_discrete_gpu(&plan_backend(&devices, PowerState::Battery), &devices));
        let integrated_only = [(0, DeviceType::Integrated, 16)];
        assert!(!runs_on_discrete_gpu(&plan_backend(&integrated_only, PowerState::Ac), &integrated_only));
    }

    #[test]
    fn uses_cpu_without_a_device() {
        assert_eq!(plan_backend(&[], PowerState::Ac), BackendPlan::Cpu);
    }

    #[test]
    fn prefers_discrete_on_ac_and_integrated_on_battery() {
        let devices = [
            (0, DeviceType::Integrated, 16),
            (1, DeviceType::Discrete, 6),
        ];
        assert_eq!(
            plan_backend(&devices, PowerState::Ac),
            BackendPlan::Vulkan(1)
        );
        assert_eq!(
            plan_backend(&devices, PowerState::Battery),
            BackendPlan::Vulkan(0)
        );
        assert_eq!(
            plan_backend(&devices, PowerState::Unknown),
            BackendPlan::Vulkan(1)
        );
    }

    #[test]
    fn battery_uses_cpu_without_integrated_gpu() {
        assert_eq!(
            plan_backend(&[(0, DeviceType::Discrete, 6), (1, DeviceType::Cpu, 32)], PowerState::Battery),
            BackendPlan::Cpu
        );
    }

    #[test]
    fn benchmark_device_override_requires_an_available_id() {
        assert_eq!(
            requested_plan(Some("cpu"), &[0, 1], BackendPlan::Vulkan(1)).unwrap(),
            BackendPlan::Cpu
        );
        assert_eq!(
            requested_plan(Some("0"), &[0, 1], BackendPlan::Vulkan(1)).unwrap(),
            BackendPlan::Vulkan(0)
        );
        assert!(requested_plan(Some("2"), &[0, 1], BackendPlan::Vulkan(1)).is_err());
    }

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

use chrono::DateTime;
use meeting_minutes_core::{
    processing::{transcribe_meeting, ProcessingError, SpeechEngine, TranscribeRequest},
    transcript::{Language, RawSegment, Segment},
    Library,
};
use meeting_minutes_whisper::{
    benchmark::{segment_stats, word_errors},
    WhisperEngine,
};
use serde::Deserialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

#[derive(Deserialize)]
struct Clip {
    id: String,
    language: String,
    path: PathBuf,
    reference: String,
    #[serde(rename = "long")]
    is_long: bool,
}

struct MeasuredEngine<'a> {
    inner: &'a mut WhisperEngine,
    whisper_time: Duration,
    after_vad_ms: Option<u64>,
}

impl SpeechEngine for MeasuredEngine<'_> {
    fn backend(&self) -> &str {
        self.inner.backend()
    }
    fn thread_count(&self) -> usize {
        self.inner.thread_count()
    }
    fn audio_after_vad_ms(&self) -> Option<u64> {
        self.inner.audio_after_vad_ms()
    }
    fn detect_language(&mut self, samples: &[f32]) -> Result<Language, String> {
        self.inner.detect_language(samples)
    }
    fn transcribe(
        &mut self,
        samples: &[f32],
        language: Language,
        prompt: Option<&str>,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Vec<RawSegment>, ProcessingError> {
        let started = Instant::now();
        let result = self
            .inner
            .transcribe(samples, language, prompt, progress, cancel);
        self.whisper_time += started.elapsed();
        self.after_vad_ms = self
            .after_vad_ms
            .and_then(|sum| self.inner.audio_after_vad_ms().map(|ms| sum + ms));
        result
    }
}

struct ResultRow {
    model: String,
    backend: String,
    set: String,
    errors: usize,
    reference_words: usize,
    audio_ms: u64,
    after_vad_ms: Option<u64>,
    whisper_time: Duration,
    segments: Vec<Segment>,
    count: usize,
}

struct ClipResult {
    hypothesis: String,
    audio_ms: u64,
    after_vad_ms: Option<u64>,
    whisper_time: Duration,
    segments: Vec<Segment>,
}

impl ResultRow {
    fn new(model: &str, backend: &str, set: &str) -> Self {
        Self {
            model: model.into(),
            backend: backend.into(),
            set: set.into(),
            errors: 0,
            reference_words: 0,
            audio_ms: 0,
            after_vad_ms: Some(0),
            whisper_time: Duration::ZERO,
            segments: Vec::new(),
            count: 0,
        }
    }

    fn print(&self) {
        let accuracy = self.errors as f64 / self.reference_words as f64;
        let rtf = self.whisper_time.as_secs_f64() / (self.audio_ms as f64 / 1000.0);
        let vad = self
            .after_vad_ms
            .map_or_else(|| "n/a".into(), |ms| format!("{:.1}s", ms as f64 / 1000.0));
        let stats = segment_stats(&self.segments);
        let (median, p10, p90, short, long) = stats.map_or(
            (
                "n/a".into(),
                "n/a".into(),
                "n/a".into(),
                "n/a".into(),
                "n/a".into(),
            ),
            |s| {
                (
                    format!("{:.1}", s.median_s),
                    format!("{:.1}", s.p10_s),
                    format!("{:.1}", s.p90_s),
                    format!("{:.0}%", s.short_share * 100.0),
                    format!("{:.0}%", s.long_share * 100.0),
                )
            },
        );
        println!(
            "| {} | {} | {} ({}) | {:.1}% | {:.3} | {} | {} | {} | {} | {} | {} |",
            self.model,
            self.backend,
            self.set,
            self.count,
            accuracy * 100.0,
            rtf,
            vad,
            median,
            p10,
            p90,
            short,
            long
        );
    }
}

fn run_clip(
    root: &Path,
    clip: &Clip,
    model: &str,
    engine: &mut WhisperEngine,
) -> Result<ClipResult, String> {
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut library = Library::open(
        temp.path().join("Library"),
        temp.path().join("index.sqlite3"),
    )
    .map_err(|e| e.to_string())?;
    let at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap();
    let cancel = AtomicBool::new(false);
    let meeting = library
        .import_audio(&root.join(&clip.path), at, &cancel)
        .map_err(|e| e.to_string())?;
    let language = match clip.language.as_str() {
        "en" => Language::En,
        "vi" => Language::Vi,
        other => return Err(format!("Unknown language {other}")),
    };
    let mut measured = MeasuredEngine {
        inner: engine,
        whisper_time: Duration::ZERO,
        after_vad_ms: Some(0),
    };
    let transcript = transcribe_meeting(
        &TranscribeRequest {
            meeting_dir: &library.meeting_dir(&meeting.id),
            model,
            vocabulary: &[],
            default_language: Some(language),
        },
        &mut measured,
        &mut |_| {},
        &cancel,
    )
    .map_err(|e| e.to_string())?;
    let text = transcript
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(ClipResult {
        hypothesis: text,
        audio_ms: meeting.duration_ms,
        after_vad_ms: measured.after_vad_ms,
        whisper_time: measured.whisper_time,
        segments: transcript.segments,
    })
}

fn run() -> Result<(), String> {
    if env::args().nth(1).as_deref() == Some("--list-devices") {
        for device in WhisperEngine::available_device_descriptions() {
            println!("{device}");
        }
        return Ok(());
    }
    let mut args = env::args().skip(1);
    let mut manifest = None;
    let mut models = Vec::new();
    let mut devices = Vec::new();
    let mut vad = None;
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("Missing value for {flag}"))?;
        match flag.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--model" => {
                let (name, path) = value.split_once('=').ok_or("Use --model NAME=PATH")?;
                models.push((name.to_owned(), PathBuf::from(path)));
            }
            "--device" => devices.push(value),
            "--vad" => vad = Some(PathBuf::from(value)),
            _ => return Err(format!("Unknown argument {flag}")),
        }
    }
    let manifest = manifest.ok_or("Pass --manifest PATH")?;
    if models.is_empty() {
        return Err("Pass at least one --model NAME=PATH".into());
    }
    if devices.is_empty() {
        devices.push("cpu".into());
    }
    let clips: Vec<Clip> = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let root = manifest
        .parent()
        .ok_or("Manifest has no parent directory")?;
    println!("| Model | Backend / device | Set | WER | RTF | After VAD | Median s | p10 s | p90 s | <1 s | >20 s |");
    println!("|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for (model, path) in models {
        for device in &devices {
            if device == "auto" {
                env::remove_var("MM_WHISPER_DEVICE");
            } else {
                env::set_var("MM_WHISPER_DEVICE", device);
            }
            let mut engine = WhisperEngine::load_with_vad(&path, vad.as_deref())?;
            let backend = if engine.backend() == "CPU" {
                "CPU".to_owned()
            } else {
                WhisperEngine::backend_description()
            };
            if device != "auto" && device != "cpu" && engine.backend() != "Vulkan" {
                return Err(format!(
                    "Requested Vulkan device {device}, but Whisper loaded on CPU"
                ));
            }
            let mut rows = [
                ResultRow::new(&model, &backend, "English short"),
                ResultRow::new(&model, &backend, "Vietnamese short"),
                ResultRow::new(&model, &backend, "English stitched"),
            ];
            for clip in &clips {
                eprintln!("{} / {} / {}", model, backend, clip.id);
                let result = run_clip(root, clip, &model, &mut engine)?;
                let row = &mut rows[if clip.is_long {
                    2
                } else if clip.language == "vi" {
                    1
                } else {
                    0
                }];
                let (errors, words) = word_errors(&clip.reference, &result.hypothesis);
                row.errors += errors;
                row.reference_words += words;
                row.audio_ms += result.audio_ms;
                row.after_vad_ms = row
                    .after_vad_ms
                    .and_then(|sum| result.after_vad_ms.map(|ms| sum + ms));
                row.whisper_time += result.whisper_time;
                row.segments.extend(result.segments);
                row.count += 1;
            }
            for row in rows.iter().filter(|row| row.count > 0) {
                row.print();
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("benchmark-transcription: {error}");
        std::process::exit(1);
    }
}

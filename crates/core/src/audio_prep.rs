use std::{
    collections::VecDeque,
    f64::consts::PI,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    capture::Source, meeting_file::RecordingFile, processing::ProcessingError, transcript::Speaker,
};

pub const WHISPER_RATE: u32 = 16_000;
const INPUT_RATE: u32 = 48_000;
const TAPS: usize = 63;

pub struct PreparedRecording {
    pub samples_16k: Vec<f32>,
    pub speaker: Speaker,
    pub duration_ms: u64,
    pub timeline_start_ms: u64,
}

fn coefficients() -> [f32; TAPS] {
    let mut taps = [0.0; TAPS];
    let cutoff = 7_500.0 / INPUT_RATE as f64;
    for (i, tap) in taps.iter_mut().enumerate() {
        let x = i as f64 - (TAPS - 1) as f64 / 2.0;
        let sinc = if x == 0.0 {
            2.0 * cutoff
        } else {
            (2.0 * std::f64::consts::PI * cutoff * x).sin() / (std::f64::consts::PI * x)
        };
        let window =
            0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / (TAPS - 1) as f64).cos();
        *tap = (sinc * window) as f32;
    }
    let sum: f32 = taps.iter().sum();
    for tap in &mut taps {
        *tap /= sum;
    }
    taps
}

/// Low-pass filters 48 kHz mono audio and keeps every third sample, one sample at a time,
/// so audio read in pieces (such as Chunks) gives the same 16 kHz stream as one read.
pub struct Decimator {
    taps: [f32; TAPS],
    history: VecDeque<f32>,
    frames: usize,
    pub output: Vec<f32>,
}

impl Default for Decimator {
    fn default() -> Self {
        Self::new()
    }
}

impl Decimator {
    pub fn new() -> Self {
        Self {
            taps: coefficients(),
            history: VecDeque::with_capacity(TAPS),
            frames: 0,
            output: Vec::new(),
        }
    }

    pub fn push(&mut self, sample: f32) {
        self.history.push_front(sample);
        if self.history.len() > TAPS {
            self.history.pop_back();
        }
        if self.frames.is_multiple_of((INPUT_RATE / WHISPER_RATE) as usize) {
            self.output.push(
                self.taps
                    .iter()
                    .zip(self.history.iter())
                    .map(|(tap, sample)| tap * sample)
                    .sum(),
            );
        }
        self.frames += 1;
    }
}

pub(crate) fn prepare_recording(
    meeting_dir: &Path,
    recording: &RecordingFile,
    cancel: &AtomicBool,
) -> Result<Vec<PreparedRecording>, ProcessingError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
    fn open(
        meeting_dir: &Path,
        recording: &RecordingFile,
        source: Source,
    ) -> Result<Option<claxon::FlacReader<std::fs::File>>, ProcessingError> {
        let Some(file) = recording.files.iter().find(|f| f.source == source) else {
            return Ok(None);
        };
        if std::path::Path::new(&file.file).file_name() != Some(std::ffi::OsStr::new(&file.file)) {
            return Err(ProcessingError::Audio("Invalid Source file name".into()));
        }
        let reader = claxon::FlacReader::open(meeting_dir.join(&file.file))
            .map_err(|e| ProcessingError::Audio(e.to_string()))?;
        if reader.streaminfo().sample_rate != INPUT_RATE
            || reader.streaminfo().channels != file.channels as u32
        {
            return Err(ProcessingError::Audio("Unexpected FLAC format".into()));
        }
        Ok(Some(reader))
    }
    let mut prepared = Vec::new();
    for (source, speaker) in [
        (Source::Microphone, Speaker::Me),
        (Source::SystemAudio, Speaker::Remote),
    ] {
        let Some(mut reader) = open(meeting_dir, recording, source)? else {
            continue;
        };
        let mut samples = reader.samples();
        let mut decimator = Decimator::new();
        let mut frames = 0usize;
        loop {
            if frames.is_multiple_of(INPUT_RATE as usize) && cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            let Some(first) = samples
                .next()
                .transpose()
                .map_err(|e| ProcessingError::Audio(e.to_string()))?
            else {
                break;
            };
            let sample = if source == Source::SystemAudio {
                let second = samples
                    .next()
                    .transpose()
                    .map_err(|e| ProcessingError::Audio(e.to_string()))?
                    .ok_or_else(|| ProcessingError::Audio("Incomplete stereo frame".into()))?;
                (first as f32 + second as f32) / 65534.0
            } else {
                first as f32 / 32767.0
            };
            decimator.push(sample);
            frames += 1;
        }
        let output = decimator.output;
        prepared.push(PreparedRecording {
            samples_16k: output,
            speaker,
            duration_ms: recording.duration_ms,
            timeline_start_ms: recording.timeline_start_ms,
        });
    }
    if prepared.is_empty() {
        return Err(ProcessingError::NoAudio);
    }
    Ok(prepared)
}

/// Exact polyphase windowed-sinc resampler from an import's rate to 16 kHz. Output n sits at input position n * down / up,
/// so it uses the weights of phase (n * down) % up, all precomputed once (at most 16 000 phases).
struct ImportResampler {
    up: u64,
    down: u64,
    half: usize,
    weights: Vec<f32>,
    history: Vec<f32>,
    first: u64,
    frames: u64,
    produced: u64,
    output: Vec<f32>,
}

impl ImportResampler {
    fn new(input_rate: u32) -> Self {
        fn gcd(a: u64, b: u64) -> u64 {
            if b == 0 {
                a
            } else {
                gcd(b, a % b)
            }
        }
        let rate = input_rate as u64;
        let divisor = gcd(rate, WHISPER_RATE as u64);
        let (up, down) = (WHISPER_RATE as u64 / divisor, rate / divisor);
        let lower = input_rate.min(WHISPER_RATE) as f64;
        let cutoff = 0.45 * lower / input_rate as f64;
        // 16 zero crossings of the lower rate on each side of the centre.
        let half = (16.0 * input_rate as f64 / lower).ceil() as usize;
        let taps = 2 * half;
        let mut weights = vec![0.0f32; up as usize * taps];
        for (phase, row) in weights.chunks_exact_mut(taps).enumerate() {
            let fraction = phase as f64 / up as f64;
            let mut values = vec![0.0f64; taps];
            for (k, value) in values.iter_mut().enumerate() {
                let distance = k as f64 - (half as f64 - 1.0) - fraction;
                let x = distance / half as f64;
                let window = if x.abs() >= 1.0 {
                    0.0
                } else {
                    0.42 + 0.5 * (PI * x).cos() + 0.08 * (2.0 * PI * x).cos()
                };
                let sinc = if distance == 0.0 {
                    2.0 * cutoff
                } else {
                    (2.0 * PI * cutoff * distance).sin() / (PI * distance)
                };
                *value = sinc * window;
            }
            let sum: f64 = values.iter().sum();
            for (weight, value) in row.iter_mut().zip(values) {
                *weight = (value / sum) as f32;
            }
        }
        Self {
            up,
            down,
            half,
            weights,
            history: Vec::new(),
            first: 0,
            frames: 0,
            produced: 0,
            output: Vec::new(),
        }
    }

    fn push(&mut self, samples: &[f32]) {
        self.history.extend_from_slice(samples);
        self.frames += samples.len() as u64;
        while self.produced * self.down / self.up + (self.half as u64) < self.frames {
            self.emit();
        }
        let keep_from = (self.produced * self.down / self.up).saturating_sub(self.half as u64 - 1);
        if keep_from > self.first + 65_536 {
            self.history.drain(..(keep_from - self.first) as usize);
            self.first = keep_from;
        }
    }

    /// Inputs before the start or past the end of the audio count as silence.
    fn emit(&mut self) {
        let position = self.produced * self.down;
        let taps = 2 * self.half;
        let row = &self.weights[(position % self.up) as usize * taps..][..taps];
        let start = (position / self.up) as i64 - self.half as i64 + 1;
        let first = self.first as i64;
        let value = if start >= first && start + taps as i64 <= self.frames as i64 {
            let inputs = &self.history[(start - first) as usize..][..taps];
            row.iter().zip(inputs).map(|(w, x)| w * x).sum()
        } else {
            row.iter()
                .enumerate()
                .map(|(k, w)| {
                    let index = start + k as i64;
                    if index >= first && index < self.frames as i64 {
                        w * self.history[(index - first) as usize]
                    } else {
                        0.0
                    }
                })
                .sum()
        };
        self.output.push(value);
        self.produced += 1;
    }

    fn finish(mut self) -> Vec<f32> {
        let expected = (self.frames * self.up).div_ceil(self.down);
        while self.produced < expected {
            self.emit();
        }
        self.output
    }
}

pub fn prepare_import(
    meeting_dir: &Path,
    file: &str,
    cancel: &AtomicBool,
) -> Result<PreparedRecording, ProcessingError> {
    if Path::new(file).file_name() != Some(std::ffi::OsStr::new(file)) {
        return Err(ProcessingError::Unsupported(
            "invalid imported file name".into(),
        ));
    }
    let mut resampler: Option<ImportResampler> = None;
    let mut rate = 0u32;
    crate::import::decode_audio(
        &meeting_dir.join(file),
        cancel,
        &mut |samples, sample_rate| {
            rate = sample_rate;
            resampler
                .get_or_insert_with(|| ImportResampler::new(sample_rate))
                .push(samples);
        },
    )?;
    let resampler = resampler.ok_or(ProcessingError::NoAudio)?;
    let frames = resampler.frames;
    Ok(PreparedRecording {
        samples_16k: resampler.finish(),
        speaker: Speaker::Unknown,
        duration_ms: frames.saturating_mul(1000) / rate as u64,
        timeline_start_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_preparation_stops_before_opening_sources() {
        let recording = RecordingFile {
            number: 1,
            started_at: "2026-10-12T09:30:00Z".into(),
            timeline_start_ms: 0,
            duration_ms: 0,
            files: vec![],
        };
        assert!(matches!(
            prepare_recording(Path::new("missing"), &recording, &AtomicBool::new(true)),
            Err(ProcessingError::Cancelled)
        ));
    }
}

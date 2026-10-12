use std::{
    collections::VecDeque,
    f64::consts::PI,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    capture::Source,
    meeting_file::RecordingFile,
    processing::ProcessingError,
    transcript::{Activity, ACTIVITY_FRAME_MS},
};

pub const WHISPER_RATE: u32 = 16_000;
const INPUT_RATE: u32 = 48_000;
const TAPS: usize = 63;

pub struct PreparedRecording {
    pub samples_16k: Vec<f32>,
    pub activity: Activity,
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

pub(crate) fn prepare_recording(
    meeting_dir: &Path,
    recording: &RecordingFile,
    cancel: &AtomicBool,
) -> Result<PreparedRecording, ProcessingError> {
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
    let mut mic = open(meeting_dir, recording, Source::Microphone)?;
    let mut sys = open(meeting_dir, recording, Source::SystemAudio)?;
    if mic.is_none() && sys.is_none() {
        return Err(ProcessingError::NoAudio);
    }
    let mut mic_samples = mic.as_mut().map(|reader| reader.samples());
    let mut sys_samples = sys.as_mut().map(|reader| reader.samples());
    let mut output = Vec::new();
    let mut mic_activity = mic_samples.as_ref().map(|_| Vec::new());
    let mut sys_activity = sys_samples.as_ref().map(|_| Vec::new());
    let mut mic_power = 0.0f64;
    let mut sys_power = 0.0f64;
    let mut frame_count = 0usize;
    let mut history = VecDeque::with_capacity(TAPS);
    let taps = coefficients();
    let mut frames = 0usize;
    loop {
        if frames.is_multiple_of(INPUT_RATE as usize) && cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let m = mic_samples
            .as_mut()
            .map(|s| s.next().transpose())
            .transpose()
            .map_err(|e| ProcessingError::Audio(e.to_string()))?
            .flatten();
        let left = sys_samples
            .as_mut()
            .map(|s| s.next().transpose())
            .transpose()
            .map_err(|e| ProcessingError::Audio(e.to_string()))?
            .flatten();
        let right = if left.is_some() {
            sys_samples
                .as_mut()
                .map(|s| s.next().transpose())
                .transpose()
                .map_err(|e| ProcessingError::Audio(e.to_string()))?
                .flatten()
        } else {
            None
        };
        if m.is_none() && left.is_none() {
            break;
        }
        if sys_samples.is_some() && left.is_some() && right.is_none() {
            return Err(ProcessingError::Audio("Incomplete stereo frame".into()));
        }
        let microphone = m.unwrap_or(0) as f32 / 32767.0;
        let system = (left.unwrap_or(0) as f32 + right.unwrap_or(0) as f32) / 65534.0;
        mic_power += (microphone as f64).powi(2);
        sys_power += (system as f64).powi(2);
        frame_count += 1;
        let mixed = (microphone + system).clamp(-1.0, 1.0);
        history.push_front(mixed);
        if history.len() > TAPS {
            history.pop_back();
        }
        if frames.is_multiple_of((INPUT_RATE / WHISPER_RATE) as usize) {
            output.push(
                taps.iter()
                    .zip(history.iter())
                    .map(|(tap, sample)| tap * sample)
                    .sum(),
            );
        }
        frames += 1;
        if frame_count == (INPUT_RATE as u64 * ACTIVITY_FRAME_MS / 1000) as usize {
            if let Some(values) = &mut mic_activity {
                values.push((mic_power / frame_count as f64).sqrt() as f32);
            }
            if let Some(values) = &mut sys_activity {
                values.push((sys_power / frame_count as f64).sqrt() as f32);
            }
            mic_power = 0.0;
            sys_power = 0.0;
            frame_count = 0;
        }
    }
    if frame_count > 0 {
        if let Some(values) = &mut mic_activity {
            values.push((mic_power / frame_count as f64).sqrt() as f32);
        }
        if let Some(values) = &mut sys_activity {
            values.push((sys_power / frame_count as f64).sqrt() as f32);
        }
    }
    Ok(PreparedRecording {
        samples_16k: output,
        activity: Activity {
            microphone: mic_activity,
            system_audio: sys_activity,
        },
        duration_ms: recording.duration_ms,
        timeline_start_ms: recording.timeline_start_ms,
    })
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
    crate::import::decode_audio(&meeting_dir.join(file), cancel, &mut |samples, sample_rate| {
        rate = sample_rate;
        resampler
            .get_or_insert_with(|| ImportResampler::new(sample_rate))
            .push(samples);
    })?;
    let resampler = resampler.ok_or(ProcessingError::NoAudio)?;
    let frames = resampler.frames;
    Ok(PreparedRecording {
        samples_16k: resampler.finish(),
        activity: Activity {
            microphone: None,
            system_audio: None,
        },
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

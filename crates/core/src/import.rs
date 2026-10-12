use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    errors::Error,
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

use crate::processing::ProcessingError;

pub const IMPORT_EXTENSIONS: &[&str] = &["mp3", "m4a", "wav", "flac"];
/// Sample rates the 16 kHz preparation for whisper accepts; decoding stops at the first packet outside it.
pub const IMPORT_RATES: std::ops::RangeInclusive<u32> = 8_000..=192_000;

pub fn import_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    IMPORT_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

fn decode_error(error: Error) -> ProcessingError {
    match error {
        Error::IoError(error) => ProcessingError::Io(error),
        other => ProcessingError::Unsupported(other.to_string()),
    }
}

pub fn decode_audio(
    path: &Path,
    cancel: &AtomicBool,
    on_samples: &mut dyn FnMut(&[f32], u32),
) -> Result<(), ProcessingError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(ProcessingError::Cancelled);
    }
    let extension = import_extension(path)
        .ok_or_else(|| ProcessingError::Unsupported("not an mp3, m4a, wav or flac file".into()))?;
    let source = std::fs::File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(source), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(&extension);
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(decode_error)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| ProcessingError::Unsupported("no audio track".into()))?;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or_else(|| ProcessingError::Unsupported("missing audio codec parameters".into()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(decode_error)?;
    let track_id = track.id;
    let mut expected_rate = None;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(ProcessingError::Cancelled);
        }
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(error) => return Err(decode_error(error)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(Error::DecodeError(_)) => continue,
            Err(error) => return Err(decode_error(error)),
        };
        let rate = decoded.spec().rate();
        if !IMPORT_RATES.contains(&rate) {
            return Err(ProcessingError::Unsupported(format!(
                "sample rate {rate} Hz (supported: 8000 to 192000 Hz)"
            )));
        }
        if expected_rate.is_some_and(|previous| previous != rate) {
            return Err(ProcessingError::Unsupported(
                "sample rate changes in the file".into(),
            ));
        }
        expected_rate = Some(rate);
        let channels = decoded.num_planes();
        if channels == 0 {
            return Err(ProcessingError::Unsupported("no audio channels".into()));
        }
        let mut interleaved = vec![0.0f32; decoded.samples_interleaved()];
        decoded.copy_to_slice_interleaved(&mut interleaved);
        let mono: Vec<f32> = interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        on_samples(&mono, rate);
    }
    Ok(())
}

pub fn audio_duration_ms(path: &Path, cancel: &AtomicBool) -> Result<u64, ProcessingError> {
    let mut frames = 0u64;
    let mut rate = 0u32;
    decode_audio(path, cancel, &mut |samples, sample_rate| {
        frames += samples.len() as u64;
        rate = sample_rate;
    })?;
    if rate == 0 {
        return Ok(0);
    }
    Ok(frames.saturating_mul(1000) / rate as u64)
}

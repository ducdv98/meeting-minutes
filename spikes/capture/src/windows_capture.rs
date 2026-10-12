use capture_spike::args::{Args, CaptureRole};
use capture_spike::log::{AudioFormat, Event, PacketFlags, Source};
use capture_spike::timeline::{Timeline, CHANNELS, SAMPLE_RATE};
use hound::{SampleFormat, WavSpec, WavWriter};
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use wasapi::{
    AudioCaptureClient, AudioClient, BufferInfo, DeviceEnumerator, Direction, Role, SampleType,
    StreamMode, WaveFormat,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

struct LogWriter {
    file: BufWriter<File>,
    last_flush: Instant,
}

type SharedLog = Arc<Mutex<LogWriter>>;

fn log_event(log: &SharedLog, event: Event) -> Result<()> {
    let mut writer = log.lock().map_err(|_| "log mutex poisoned")?;
    serde_json::to_writer(&mut writer.file, &event)?;
    writer.file.write_all(b"\n")?;
    if writer.last_flush.elapsed() >= Duration::from_secs(2) || matches!(event, Event::Stop { .. })
    {
        writer.file.flush()?;
        writer.last_flush = Instant::now();
    }
    Ok(())
}

fn clock_frequency() -> Result<u64> {
    let mut value = 0i64;
    unsafe { QueryPerformanceFrequency(&mut value)? };
    if value <= 0 {
        return Err("invalid QPC frequency".into());
    }
    Ok(value as u64)
}

fn qpc_100ns(frequency: u64) -> Result<u64> {
    let mut value = 0i64;
    unsafe { QueryPerformanceCounter(&mut value)? };
    Ok((value as u128 * 10_000_000 / frequency as u128) as u64)
}

fn output_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS as u16,
        bits: 32,
        sample_type: "float".into(),
    }
}

fn direction(source: Source) -> Direction {
    match source {
        Source::Microphone => Direction::Capture,
        Source::SystemAudio => Direction::Render,
    }
}

fn default_device(
    enumerator: &DeviceEnumerator,
    source: Source,
    role: CaptureRole,
) -> Result<wasapi::Device> {
    let role = match role {
        CaptureRole::Console => Role::Console,
        CaptureRole::Communications => Role::Communications,
    };
    Ok(enumerator.get_default_device_for_role(&direction(source), &role)?)
}

struct Active {
    id: String,
    audio: AudioClient,
    capture: AudioCaptureClient,
    buffer: Vec<u8>,
}

impl Drop for Active {
    fn drop(&mut self) {
        let _ = self.audio.stop_stream();
    }
}

fn open_device(
    enumerator: &DeviceEnumerator,
    source: Source,
    role: CaptureRole,
) -> Result<(Active, Event)> {
    let device = default_device(enumerator, source, role)?;
    let id = device.get_id()?;
    let name = device.get_friendlyname()?;
    let mut audio = device.get_iaudioclient()?;
    let mix = audio.get_mixformat()?;
    let mix_format = AudioFormat {
        sample_rate: mix.get_samplespersec(),
        channels: mix.get_nchannels(),
        bits: mix.get_bitspersample(),
        sample_type: mix.get_subformat()?.to_string().to_lowercase(),
    };
    let output = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNELS,
        None,
    );
    let mode = StreamMode::PollingShared {
        autoconvert: true,
        buffer_duration_hns: 1_000_000,
    };
    let capture_direction = Direction::Capture;
    audio.initialize_client(&output, &capture_direction, &mode)?;
    let buffer_frames = audio.get_buffer_size()?;
    let capture = audio.get_audiocaptureclient()?;
    audio.start_stream()?;
    Ok((
        Active {
            id: id.clone(),
            audio,
            capture,
            buffer: vec![0; buffer_frames as usize * CHANNELS * 4],
        },
        Event::DeviceFormat {
            source,
            role: role.as_str(),
            device_id: id,
            device_name: name,
            mix_format,
            output_format: output_format(),
        },
    ))
}

fn read_packet(device: &mut Active) -> Result<Option<(u32, BufferInfo)>> {
    let Some(frames) = device.capture.get_next_packet_size()? else {
        return Ok(None);
    };
    if frames == 0 {
        return Ok(None);
    }
    device.buffer.resize(frames as usize * CHANNELS * 4, 0);
    let (read_frames, info) = device.capture.read_from_device(&mut device.buffer)?;
    Ok((read_frames != 0).then_some((read_frames, info)))
}

fn write_samples(writer: &mut WavWriter<BufWriter<File>>, samples: &[f32]) -> Result<()> {
    for &sample in samples {
        writer.write_sample(sample)?;
    }
    Ok(())
}

fn pad(
    writer: &mut WavWriter<BufWriter<File>>,
    timeline: &mut Timeline,
    now: u64,
    final_pad: bool,
    source: Source,
    log: &SharedLog,
) -> Result<()> {
    let samples = timeline.pad_to(now, final_pad);
    if !samples.is_empty() {
        write_samples(writer, &samples)?;
        log_event(
            log,
            Event::IdlePad {
                source,
                frames: (samples.len() / CHANNELS) as u64,
            },
        )?;
    }
    Ok(())
}

fn record_source(
    source: Source,
    path: &Path,
    start: u64,
    stop: &AtomicU64,
    frequency: u64,
    log: &SharedLog,
    role: CaptureRole,
) -> Result<u64> {
    wasapi::initialize_mta().ok()?;
    let enumerator = DeviceEnumerator::new()?;
    let spec = WavSpec {
        channels: CHANNELS as u16,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)?;
    let mut timeline = Timeline::new(start);
    let mut active: Option<Active> = None;
    let mut previous_id: Option<String> = None;
    let mut switch_reason = String::from("default_changed");
    let mut last_device_check = Instant::now() - Duration::from_secs(1);
    let mut last_flush = Instant::now();
    let mut last_error = Instant::now() - Duration::from_secs(2);
    loop {
        let now = qpc_100ns(frequency)?;
        let stop_at = stop.load(Ordering::Acquire);
        if stop_at != 0 {
            break;
        }
        if last_device_check.elapsed() >= Duration::from_millis(500) {
            last_device_check = Instant::now();
            let current_id =
                default_device(&enumerator, source, role).and_then(|device| Ok(device.get_id()?));
            match current_id {
                Ok(id) if active.as_ref().is_some_and(|device| device.id != id) => {
                    previous_id = active.take().map(|device| device.id.clone());
                    switch_reason = "default_changed".into();
                }
                Err(error) if active.is_some() => {
                    previous_id = active.take().map(|device| device.id.clone());
                    switch_reason = "default_unavailable".into();
                    log_event(
                        log,
                        Event::Error {
                            source: Some(source),
                            message: error.to_string(),
                        },
                    )?;
                }
                _ => {}
            }
        }
        if active.is_none() {
            match open_device(&enumerator, source, role) {
                Ok((device, format_event)) => {
                    log_event(log, format_event)?;
                    if previous_id.is_some() {
                        log_event(
                            log,
                            Event::DeviceSwitch {
                                source,
                                from_device: previous_id.take(),
                                to_device: Some(device.id.clone()),
                                reason: switch_reason.clone(),
                            },
                        )?;
                    }
                    active = Some(device);
                }
                Err(error) => {
                    if last_error.elapsed() >= Duration::from_secs(2) {
                        log_event(
                            log,
                            Event::Error {
                                source: Some(source),
                                message: error.to_string(),
                            },
                        )?;
                        last_error = Instant::now();
                    }
                }
            }
        }
        if active.is_some() {
            let mut read_error = None;
            while let Some(device) = active.as_mut() {
                if stop.load(Ordering::Acquire) != 0 {
                    break;
                }
                let (read_frames, info) = match read_packet(device) {
                    Ok(Some(packet)) => packet,
                    Ok(None) => break,
                    Err(error) => {
                        read_error = Some(error);
                        break;
                    }
                };
                let arrival = qpc_100ns(frequency)?;
                let mut samples = Vec::with_capacity(read_frames as usize * CHANNELS);
                if info.flags.silent {
                    samples.resize(read_frames as usize * CHANNELS, 0.0);
                } else {
                    for &bytes in device.buffer[..read_frames as usize * CHANNELS * 4]
                        .as_chunks::<4>()
                        .0
                    {
                        samples.push(f32::from_le_bytes(bytes));
                    }
                }
                let valid_qpc = if info.flags.timestamp_error {
                    None
                } else {
                    Some(info.timestamp)
                };
                let result = timeline.packet(
                    valid_qpc,
                    arrival,
                    &samples,
                    info.flags.silent,
                    info.flags.data_discontinuity,
                );
                write_samples(&mut writer, &result.samples)?;
                log_event(
                    log,
                    Event::Packet {
                        source,
                        qpc_100ns: info.timestamp,
                        arrival_qpc_100ns: arrival,
                        placed_qpc_100ns: result.placed_qpc_100ns,
                        timestamp_source: result.timestamp_source,
                        device_position: info.index,
                        frames: read_frames,
                        flags: PacketFlags {
                            silent: info.flags.silent,
                            discontinuity: info.flags.data_discontinuity,
                            timestamp_error: info.flags.timestamp_error,
                        },
                        gap_frames_inserted: result.gap_frames_inserted,
                        overlap_frames_dropped: result.overlap_frames_dropped,
                    },
                )?;
            }
            if let Some(error) = read_error {
                log_event(
                    log,
                    Event::Error {
                        source: Some(source),
                        message: error.to_string(),
                    },
                )?;
                previous_id = active.take().map(|device| device.id.clone());
                switch_reason = "capture_error".into();
            }
        }
        pad(&mut writer, &mut timeline, now, false, source, log)?;
        if last_flush.elapsed() >= Duration::from_secs(3) {
            writer.flush()?;
            last_flush = Instant::now();
        }
        thread::sleep(Duration::from_millis(10));
    }
    let stop_at = stop.load(Ordering::Acquire);
    pad(&mut writer, &mut timeline, stop_at, true, source, log)?;
    writer.finalize()?;
    Ok(timeline.frames_written())
}

pub fn run(args: Args) -> Result<()> {
    fs::create_dir_all(&args.out)?;
    let frequency = clock_frequency()?;
    let start = qpc_100ns(frequency)?;
    let log = Arc::new(Mutex::new(LogWriter {
        file: BufWriter::new(File::create(args.out.join("log.jsonl"))?),
        last_flush: Instant::now(),
    }));
    log_event(
        &log,
        Event::Start {
            minutes: args.minutes,
            out: args.out.display().to_string(),
            role: args.role.as_str(),
            start_qpc_100ns: start,
        },
    )?;
    let stop = Arc::new(AtomicU64::new(0));
    let interrupted = Arc::new(AtomicBool::new(false));
    let ctrl_flag = interrupted.clone();
    ctrlc::set_handler(move || {
        ctrl_flag.store(true, Ordering::Release);
    })?;
    let mut handles = Vec::new();
    for (source, filename) in [
        (Source::Microphone, "mic.wav"),
        (Source::SystemAudio, "system.wav"),
    ] {
        let path = args.out.join(filename);
        let stop = stop.clone();
        let log = log.clone();
        let role = args.role;
        handles.push(thread::spawn(move || {
            record_source(source, &path, start, &stop, frequency, &log, role)
        }));
    }
    let duration_hns = (args.minutes * 60.0 * 10_000_000.0) as u64;
    let deadline = start.saturating_add(duration_hns);
    let mut wait_error = None;
    loop {
        if interrupted.load(Ordering::Acquire)
            || handles.iter().any(thread::JoinHandle::is_finished)
        {
            break;
        }
        match qpc_100ns(frequency) {
            Ok(now) if now >= deadline => break,
            Ok(_) => {}
            Err(error) => {
                wait_error = Some(error);
                break;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let stop_at = match qpc_100ns(frequency) {
        Ok(now) => now,
        Err(error) => {
            if wait_error.is_none() {
                wait_error = Some(error);
            }
            start
        }
    };
    stop.store(stop_at, Ordering::Release);
    let mut totals = [0; 2];
    let mut first_error = None;
    for (index, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(frames)) => totals[index] = frames,
            Ok(Err(error)) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
            Err(_) => {
                if first_error.is_none() {
                    first_error = Some("capture thread panicked".into());
                }
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }
    if let Some(error) = wait_error {
        return Err(error);
    }
    log_event(
        &log,
        Event::Stop {
            microphone_frames: totals[0],
            system_audio_frames: totals[1],
            elapsed_qpc_100ns: stop_at - start,
        },
    )?;
    Ok(())
}

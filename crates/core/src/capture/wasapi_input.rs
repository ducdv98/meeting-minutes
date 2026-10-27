//! WASAPI inputs and the QPC clock, as measured in the capture spike (ticket 03).

use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, Direction, Role, SampleType, StreamMode,
    WaveFormat,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

use super::{
    Clock, DeviceInfo, InputDevice, InputOpener, Packet, Source, CAPTURE_CHANNELS, HNS_PER_SECOND,
    SAMPLE_RATE,
};

pub type PlatformOpener = WasapiOpener;
pub type PlatformClock = QpcClock;

/// Selects the Windows capture adapter and its host clock.
pub fn platform_capture() -> Result<(PlatformOpener, PlatformClock), String> {
    Ok((WasapiOpener, QpcClock::new()?))
}

/// List active capture and render endpoints for diagnostics. A failed lookup is distinct from
/// an empty collection so diagnostics can report that device information is unavailable.
pub fn audio_devices() -> Result<Vec<String>, String> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new().map_err(|error| error.to_string())?;
    let mut names = Vec::new();
    for (label, direction) in [
        ("Microphone", Direction::Capture),
        ("System audio", Direction::Render),
    ] {
        let devices = enumerator
            .get_device_collection(&direction)
            .map_err(|error| error.to_string())?;
        let count = devices
            .get_nbr_devices()
            .map_err(|error| error.to_string())?;
        for index in 0..count {
            let device = devices
                .get_device_at_index(index)
                .map_err(|error| error.to_string())?;
            let name = device
                .get_friendlyname()
                .map_err(|error| error.to_string())?;
            names.push(format!("{label}: {name}"));
        }
    }
    Ok(names)
}

/// QueryPerformanceCounter in 100 ns units; WASAPI packet timestamps use the same clock.
#[derive(Clone, Copy, Debug)]
pub struct QpcClock {
    frequency: u64,
}

impl QpcClock {
    pub fn new() -> Result<Self, String> {
        let mut value = 0i64;
        unsafe { QueryPerformanceFrequency(&mut value) }.map_err(|error| error.to_string())?;
        if value <= 0 {
            return Err("invalid QPC frequency".into());
        }
        Ok(Self {
            frequency: value as u64,
        })
    }
}

impl Clock for QpcClock {
    fn now_100ns(&self) -> u64 {
        let mut value = 0i64;
        // QueryPerformanceCounter cannot fail on Windows XP and later.
        let _ = unsafe { QueryPerformanceCounter(&mut value) };
        (value as u128 * HNS_PER_SECOND as u128 / self.frequency as u128) as u64
    }
}

/// Opens the default communications device: the Microphone as a capture endpoint, System audio as
/// loopback of the render endpoint. Both are converted to 48 kHz stereo f32 by WASAPI.
#[derive(Clone, Copy, Debug, Default)]
pub struct WasapiOpener;

pub struct WasapiDevice {
    info: DeviceInfo,
    audio: AudioClient,
    capture: AudioCaptureClient,
    buffer: Vec<u8>,
}

impl Drop for WasapiDevice {
    fn drop(&mut self) {
        let _ = self.audio.stop_stream();
    }
}

impl InputOpener for WasapiOpener {
    type Device = WasapiDevice;

    fn open(&self, source: Source) -> Result<WasapiDevice, String> {
        open(source).map_err(|error| error.to_string())
    }
    fn default_device_id(&self, source: Source) -> Result<String, String> {
        let device = default_device(source).map_err(|error| error.to_string())?;
        device.get_id().map_err(|error| error.to_string())
    }
}

fn direction(source: Source) -> Direction {
    match source {
        Source::Microphone => Direction::Capture,
        Source::SystemAudio => Direction::Render,
    }
}

fn default_device(source: Source) -> Result<wasapi::Device, Box<dyn std::error::Error>> {
    let _ = wasapi::initialize_mta();
    let enumerator = DeviceEnumerator::new()?;
    Ok(enumerator.get_default_device_for_role(&direction(source), &Role::Communications)?)
}

fn open(source: Source) -> Result<WasapiDevice, Box<dyn std::error::Error>> {
    let device = default_device(source)?;
    let info = DeviceInfo {
        id: device.get_id()?,
        name: device.get_friendlyname()?,
    };
    let mut audio = device.get_iaudioclient()?;
    let format = WaveFormat::new(
        32,
        32,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CAPTURE_CHANNELS,
        None,
    );
    let mode = StreamMode::PollingShared {
        autoconvert: true,
        buffer_duration_hns: 1_000_000,
    };
    // Loopback is a capture stream opened on the render endpoint.
    audio.initialize_client(&format, &Direction::Capture, &mode)?;
    let buffer_frames = audio.get_buffer_size()?;
    let capture = audio.get_audiocaptureclient()?;
    audio.start_stream()?;
    Ok(WasapiDevice {
        info,
        audio,
        capture,
        buffer: vec![0; buffer_frames as usize * CAPTURE_CHANNELS * 4],
    })
}

impl InputDevice for WasapiDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        let frames = match self.capture.get_next_packet_size() {
            Ok(Some(frames)) if frames > 0 => frames,
            Ok(_) => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        self.buffer
            .resize(frames as usize * CAPTURE_CHANNELS * 4, 0);
        let (read_frames, info) = self
            .capture
            .read_from_device(&mut self.buffer)
            .map_err(|error| error.to_string())?;
        if read_frames == 0 {
            return Ok(None);
        }
        let len = read_frames as usize * CAPTURE_CHANNELS;
        let samples = if info.flags.silent {
            vec![0.0; len]
        } else {
            self.buffer[..len * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f32::from_le_bytes(*bytes))
                .collect()
        };
        Ok(Some(Packet {
            samples,
            host_100ns: (!info.flags.timestamp_error).then_some(info.timestamp),
            silent: info.flags.silent,
            discontinuity: info.flags.data_discontinuity,
        }))
    }
}

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod wasapi_input;
#[cfg(windows)]
pub use self::wasapi_input::{audio_devices, QpcClock, WasapiDevice, WasapiOpener};

#[cfg(not(windows))]
pub fn audio_devices() -> Result<Vec<String>, String> {
    Err("WASAPI is only available on Windows".into())
}

pub const SAMPLE_RATE: u32 = 48_000;
pub const CAPTURE_CHANNELS: usize = 2;
pub const HNS_PER_SECOND: u64 = 10_000_000;

/// Peak level on a perceptual meter: silence below -60 dBFS, full scale at 0 dBFS.
pub fn buffer_level(samples: &[f32]) -> f32 {
    let peak = samples
        .iter()
        .copied()
        .filter(|sample| sample.is_finite())
        .map(f32::abs)
        .fold(0.0_f32, f32::max);
    if peak <= 0.001 {
        0.0
    } else {
        ((20.0 * peak.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
    }
}
const GAP_TOLERANCE_FRAMES: u64 = 480;
const IDLE_MARGIN_HNS: u64 = 2_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Microphone,
    SystemAudio,
}

impl Source {
    pub(crate) fn stem(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::SystemAudio => "system",
        }
    }
    pub(crate) fn channels(self) -> usize {
        match self {
            Self::Microphone => 1,
            Self::SystemAudio => 2,
        }
    }
}

pub struct Packet {
    pub samples: Vec<f32>,
    pub qpc_100ns: Option<u64>,
    pub silent: bool,
    pub discontinuity: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

pub trait Clock: Clone + Send + 'static {
    fn now_100ns(&self) -> u64;
}
pub trait InputDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String>;
    fn info(&self) -> &DeviceInfo;
}
pub trait InputOpener: Clone + Send + 'static {
    type Device: InputDevice;
    fn open(&self, source: Source) -> Result<Self::Device, String>;
    fn default_device_id(&self, source: Source) -> Result<String, String>;
}

pub struct SourceClock {
    start_100ns: u64,
    paused_total: u64,
    paused_at: Option<u64>,
    resume_at: u64,
    frames_written: u64,
    previous_packet_qpc: Option<u64>,
}

impl SourceClock {
    pub fn new(start_100ns: u64) -> Self {
        Self {
            start_100ns,
            paused_total: 0,
            paused_at: None,
            resume_at: start_100ns,
            frames_written: 0,
            previous_packet_qpc: None,
        }
    }
    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }
    pub(crate) fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
    pub(crate) fn active_ms(&self, at: u64) -> u64 {
        self.frame_at(at).saturating_mul(1000) / SAMPLE_RATE as u64
    }
    fn frame_at(&self, at: u64) -> u64 {
        let active = at
            .saturating_sub(self.start_100ns)
            .saturating_sub(self.paused_total);
        ((active as u128 * SAMPLE_RATE as u128) / HNS_PER_SECOND as u128) as u64
    }
    pub fn packet(&mut self, packet: &Packet, arrival_100ns: u64) -> Vec<f32> {
        assert_eq!(packet.samples.len() % CAPTURE_CHANNELS, 0);
        if self.paused_at.is_some()
            || arrival_100ns < self.resume_at
            || packet.qpc_100ns.is_some_and(|t| t < self.resume_at)
        {
            return Vec::new();
        }
        let usable = packet.qpc_100ns.filter(|&t| {
            t >= self.start_100ns
                && t.abs_diff(arrival_100ns) <= 2 * HNS_PER_SECOND
                && self.previous_packet_qpc.is_none_or(|previous| t > previous)
        });
        if let Some(t) = usable {
            self.previous_packet_qpc = Some(t);
        }
        let packet_start = self.frame_at(usable.unwrap_or(arrival_100ns));
        let packet_frames = (packet.samples.len() / CAPTURE_CHANNELS) as u64;
        let gap = packet_start.saturating_sub(self.frames_written);
        let mut result = Vec::new();
        if gap > GAP_TOLERANCE_FRAMES || (packet.discontinuity && gap > 0) {
            result.resize(gap as usize * CAPTURE_CHANNELS, 0.0);
            self.frames_written = packet_start;
        }
        let dropped = self
            .frames_written
            .saturating_sub(packet_start)
            .min(packet_frames);
        let remaining = packet_frames - dropped;
        if packet.silent {
            result.resize(result.len() + remaining as usize * CAPTURE_CHANNELS, 0.0);
        } else {
            result.extend_from_slice(&packet.samples[dropped as usize * CAPTURE_CHANNELS..]);
        }
        self.frames_written += remaining;
        result
    }
    pub fn pad_to(&mut self, now_100ns: u64, final_pad: bool) -> Vec<f32> {
        if self.paused_at.is_some() {
            return Vec::new();
        }
        let at = if final_pad {
            now_100ns
        } else {
            now_100ns.saturating_sub(IDLE_MARGIN_HNS)
        };
        let target = self.frame_at(at);
        let frames = target.saturating_sub(self.frames_written);
        self.frames_written += frames;
        vec![0.0; frames as usize * CAPTURE_CHANNELS]
    }
    pub fn pause(&mut self, at_100ns: u64) -> Vec<f32> {
        if self.paused_at.is_some() {
            return Vec::new();
        }
        let padded = self.pad_to(at_100ns, true);
        self.paused_at = Some(at_100ns);
        padded
    }
    pub fn resume(&mut self, at_100ns: u64) {
        if let Some(paused_at) = self.paused_at.take() {
            self.paused_total += at_100ns.saturating_sub(paused_at);
            self.resume_at = at_100ns;
            self.previous_packet_qpc = None;
        }
    }
}

#[cfg(test)]
mod level_tests {
    use super::buffer_level;

    #[test]
    fn maps_peak_to_meter_with_a_minus_sixty_db_floor() {
        assert_eq!(buffer_level(&[]), 0.0);
        assert_eq!(buffer_level(&[0.0, 0.0001]), 0.0);
        assert!((buffer_level(&[-0.1, 0.05]) - 2.0 / 3.0).abs() < 0.001);
        assert_eq!(buffer_level(&[1.0, 0.5]), 1.0);
        assert_eq!(buffer_level(&[f32::NAN, f32::INFINITY]), 0.0);
    }
}

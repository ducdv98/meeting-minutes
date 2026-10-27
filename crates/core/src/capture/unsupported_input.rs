use std::time::Instant;

use super::{Clock, DeviceInfo, InputDevice, InputOpener, Packet, Source};

const UNAVAILABLE: &str = "Recording isn't available on this platform yet";

pub type PlatformOpener = UnsupportedOpener;
pub type PlatformClock = HostClock;

/// Selects the placeholder capture adapter and its host clock.
pub fn platform_capture() -> Result<(PlatformOpener, PlatformClock), String> {
    Ok((UnsupportedOpener, HostClock::default()))
}

/// Monotonic host time in 100 ns ticks until a platform capture adapter is available.
#[derive(Clone, Debug)]
pub struct HostClock {
    origin: Instant,
}

impl Default for HostClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Clock for HostClock {
    fn now_100ns(&self) -> u64 {
        (self.origin.elapsed().as_nanos() / 100) as u64
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedOpener;

pub struct UnsupportedDevice {
    info: DeviceInfo,
}

impl InputDevice for UnsupportedDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String> {
        Err(UNAVAILABLE.into())
    }

    fn info(&self) -> &DeviceInfo {
        &self.info
    }
}

impl InputOpener for UnsupportedOpener {
    type Device = UnsupportedDevice;

    fn open(&self, _: Source) -> Result<Self::Device, String> {
        Err(UNAVAILABLE.into())
    }

    fn default_device_id(&self, _: Source) -> Result<String, String> {
        Err(UNAVAILABLE.into())
    }
}

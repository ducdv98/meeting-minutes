# Ticket 07 design: follow the default audio device mid-Recording

Ticket: .scratch/windows-mvp/issues/07-follow-default-device.md. Vocabulary: CONTEXT.md. Earlier designs: 05-core-design.md, 06-core-design.md.
Existing code: crates/core/src/{capture.rs, capture/wasapi_input.rs, recording.rs, recorder.rs, timeline.rs}, src-tauri/src/main.rs, src/{recording.ts, RecordingBar.tsx}.

## Behaviour

- Each Source thread checks about twice a second (`DEFAULT_CHECK_INTERVAL = 500 ms`, real time like `REOPEN_INTERVAL`) whether its open
  device is still the default endpoint for its Source (same role as today: Communications). If the default id differs, or the default
  cannot be read (no device), the device is dropped and reopened right away (do not wait `REOPEN_INTERVAL` for that first reopen).
- A read error drops the device as today (a headset unplug usually shows up this way first); reopen as today.
- A **device switch** is when a device opens whose id differs from the last device this Source opened in this Recording. The first open is
  never a switch; reopening the same device after a read error is not a switch.
- On a switch, the Source thread reports `{ source, device name, at_100ns = clock.now_100ns() }` over a channel to the CaptureSession.
- The Recording keeps going on the same Chunks: the SourceClock already pads the gap with silence while no device is open (unchanged).
- The CaptureSession turns each reported switch into a Marker at `RecordingClock::elapsed_ms(at_100ns)` and writes recording.json, so a
  crash keeps it (crash recovery and `place_recording` carry it to meeting.json).
- The app shows a non-blocking notice in the window ("Microphone switched to <name>" / "System audio switched to <name>"), dismissable,
  no modal, `role="status"`.

## Types

```rust
// recording.rs: Marker gains a variant.
pub enum Marker {
    Pause { at_ms: u64, paused_ms: u64 },
    Gap { at_ms: u64 },
    DeviceSwitch { at_ms: u64, source: Source, device: String },  // json: {"kind":"device_switch","at_ms":..,"source":"microphone","device":"Logi USB Headset"}
}

// capture.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo { pub id: String, pub name: String }   // name = friendly name shown to the user

pub trait InputDevice {
    fn read_packet(&mut self) -> Result<Option<Packet>, String>;
    fn info(&self) -> &DeviceInfo;
}
pub trait InputOpener: Clone + Send + 'static {
    type Device: InputDevice;
    fn open(&self, source: Source) -> Result<Self::Device, String>;
    /// Id of the current default endpoint for this Source.
    fn default_device_id(&self, source: Source) -> Result<String, String>;
}
```

Windows (`wasapi_input.rs`): `open` fills `DeviceInfo` from `device.get_id()` and `device.get_friendlyname()`; `default_device_id` calls
`wasapi::initialize_mta()` (ignore result), creates a `DeviceEnumerator`, and returns `get_default_device_for_role(direction, Communications)?.get_id()`.
Share the direction/role lookup with `open`.

## timeline.rs

`place_recording` offsets and clamps `DeviceSwitch` like `Pause` (keep source and device). Unit test it in tests/timeline.rs.

## recording.rs

- `RecordingClock::add_device_switch(&mut self, at_100ns: u64, source: Source, device: String)` pushes
  `Marker::DeviceSwitch { at_ms: self.elapsed_ms(at_100ns), .. }`.
- `CaptureSession` holds the receiving end of the switch channel (one `mpsc::Sender` cloned into each worker).
- `pub fn collect_device_switches(&mut self) -> io::Result<()>`: drains the channel; if anything arrived, adds the Markers to the
  RecordingClock, sets `manifest.markers = timeline.markers().to_vec()` and writes recording.json. Markers stay in memory if the write fails.
- `pause`, `resume`, `stop` call it after `control(...)` (workers report a switch before replying to a command, so nothing is lost), and
  all three set `manifest.markers` from the RecordingClock before writing.
- `pub fn device_switches(&self) -> Vec<DeviceSwitch>`: the DeviceSwitch Markers of this Recording, in order.
- Worker: track `last_opened: Option<DeviceInfo>` and `last_default_check: Option<Instant>` alongside `device`/`last_open_attempt`
  (a small struct is fine).

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceSwitch { pub at_ms: u64, pub source: Source, pub device: String }
```

## recorder.rs

- `RecorderStatus` gains `pub device_switches: Vec<DeviceSwitch>` (the active Recording's switches; empty when idle).
- `pub fn collect_device_switches(&mut self) -> Result<(), RecorderError>`: no-op when idle, else the session's method.

## App

- main.rs: the 500 ms loop calls `collect_device_switches()` (ignore the error; the Markers are kept in memory and written on the next
  manifest write) before `publish` while the phase is Recording.
- recording.ts: `DeviceSwitch { at_ms, source: "microphone" | "system_audio", device }`, `RecorderStatus.device_switches`.
- RecordingBar: while capturing, show the latest switch as `<p role="status" className="device-switch">Microphone switched to Logi USB Headset
  <button>Dismiss</button></p>`. Dismiss hides switches up to that count (local state); a newer switch shows again. Not shown when idle.

## Tests (TDD, through the public API)

- tests/timeline.rs: DeviceSwitch offset by timeline_start and clamped; source/device kept; JSON shape `kind: "device_switch"`.
- tests/recording.rs (fake opener whose default id per Source lives in `Arc<Mutex<..>>`, devices built with that id/name):
  - switching the Microphone default mid-Recording: the old device is dropped, the new one opened, one DeviceSwitch Marker for
    Microphone at the clock time of the switch (fake clock set before the switch; poll `collect_device_switches`/`device_switches` with a
    timeout of a few seconds), manifest on disk has it, `stop()` manifest has it, frames still cover the whole duration.
  - reopening the same device after a read error adds no Marker.
  - default unavailable (Err) then back to a different device: one Marker when the new device opens.
- tests/recorder.rs: `status().device_switches` shows the switch while recording; after stop + save, meeting.json markers include it.
- Update every existing fake opener/device for the new trait methods.
- src/RecordingBar.test.tsx: notice shows the latest switch, Dismiss hides it, a newer switch shows again, nothing when idle.

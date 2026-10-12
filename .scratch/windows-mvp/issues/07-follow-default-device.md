# Follow the default audio device mid-Recording

Status: resolved
Blocked by: 05
Spec: ../spec.md

## What

When the Windows default capture or render device changes during a Recording, switch capture to the new default device, keep the same Recording, add a device-switch Marker and show a non-blocking notice.

## Acceptance criteria

- [x] Switching to or from a headset mid-Recording keeps recording without manual action.
- [x] A device-switch Marker appears on the Timeline at the right time.
- [x] Manual check on the laptop with a USB or Bluetooth headset.

## Comments

### 2026-10-12: implemented; laptop check pending

Code is done and tested on Linux. The Windows build passes `cargo clippy --target x86_64-pc-windows-gnu`. The design is in `../07-core-design.md`.

- Core (`crates/core`):
  - Each Source thread checks every 500 ms whether its device is still the default Communications endpoint. If the default changed or is gone, it drops the device and reopens right away. A read error (usually how an unplug shows up first) drops the device as before.
  - A device switch is an open whose device id differs from the last device that Source opened. The first open is not a switch, and neither is reopening the same device after an error. While no device is open, the Source is padded with silence on the QPC clock, so the Recording and its Chunks carry on.
  - A switch becomes `{"kind":"device_switch","at_ms":..,"source":"microphone","device":"<friendly name>"}` in `recording.json`, kept in Timeline order. `place_recording` carries it into `meeting.json`, crash recovery included.
  - `RecorderStatus.device_switches` lists the switches of the active Recording.
- App: the 500 ms tick writes reported switches to `recording.json`. While recording, the window shows "Microphone switched to <name>" (or System audio), with Dismiss. It is not modal.
- Known gaps:
  - A kill within about 500 ms of a switch can drop that Marker from `recording.json`. The audio itself is kept.
  - The notice is only in the window, not a Windows toast, so a hidden window will not show it.
  - A device change during a pause is picked up at resume.
- Manual check on the laptop with a USB or Bluetooth headset:
  1. Start recording on the laptop speakers and mic, talk, and play some audio.
  2. Plug in the headset (or connect it over Bluetooth) and keep talking. The window should show the switch notice within about a second.
  3. Unplug it again, then Stop.
  4. Both FLACs should play through without a break in the Timeline. `meeting.json` should have `device_switch` Markers at the plug and unplug times.

### 2026-10-12: laptop check passed; resolved

The user ran the headset check on the laptop and it worked.

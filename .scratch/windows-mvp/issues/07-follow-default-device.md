# Follow the default audio device mid-Recording

Status: ready-for-agent
Blocked by: 05
Spec: ../spec.md

## What

When the Windows default capture or render device changes during a Recording, switch capture to the new default device, keep the same Recording, add a device-switch Marker and show a non-blocking notice.

## Acceptance criteria

- [ ] Switching to or from a headset mid-Recording keeps recording without manual action.
- [ ] A device-switch Marker appears on the Timeline at the right time.
- [ ] Manual check on the laptop with a USB or Bluetooth headset.

## Comments

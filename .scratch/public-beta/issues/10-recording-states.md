# Redesign recording controls, indicator and tray states

Status: ready-for-human
Blocked by: 07
Spec: ../spec.md

## What

Rebuild the recording bar (Record, Pause/Resume, Stop, elapsed time, Source levels, device-switch notice) and the always-visible recording indicator to the mockup. Tray menu labels and tooltip reflect the state.

## Acceptance criteria

- [x] Recording state is unmistakable (red) in light and dark, in the window and tray.
- [x] Hotkeys shown in tooltips.
- [ ] Existing behaviour and tests unchanged; manual check on the laptop.

## Comments

- 2026-10-13: Rebuilt the Recording bar with a reduced-motion-aware red pulse, elapsed time, real Microphone and System audio device names, Pause/Resume and Stop controls, device-switch toast, save progress and Retry. The sidebar opens the live Meeting; the app header, native window title and tray reflect Recording/Paused/Idle. Restyled the first-Recording notice with the shared Dialog. The recorder does not expose live audio levels, so no level meters are shown; the Source names and actual devices are shown instead. Verified `pnpm typecheck`, `pnpm test` (93 passed), `pnpm build`, the 25 recording core tests, and the Windows check (`Finished` with no warnings or errors). Laptop check remains: confirm both themes, the real tray icon/menu/tooltip and window title, device switches, hotkeys, and save failure/Retry with real devices.

### 2026-10-13: review fixes

Real Source levels now come from the recorder: `buffer_level` (peak with a -60 dBFS floor, unit tested) per captured buffer, exposed per Source in `RecorderStatus` with a "Silent" flag, drawn by the reusable `src/components/ui/LevelMeter.tsx` (ticket 12 reuses it). The main pane shows the mockup's Recording card while a Recording runs or is paused. Tray and window title only update on change and log `TrayUpdateFailed` on setter errors. Laptop check: meters move with speech and system sound; a muted Source shows Silent after about 3 s.

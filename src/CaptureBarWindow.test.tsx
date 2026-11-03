import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import CaptureBarWindow from "./CaptureBarWindow";
import {
  dismissSourceWarning,
  openSoundSettings,
  onRecordingStatus,
  onSnapshotError,
  pauseRecording,
  recordingStatus,
  resumeRecording,
  stopRecording,
  takeSnapshot,
  type RecorderStatus,
} from "./recording";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { usePlatformCapabilities } from "./platform-capabilities";

vi.mock("./recording", async (importOriginal) => ({
  ...await importOriginal<typeof import("./recording")>(),
  onRecordingStatus: vi.fn(),
  dismissSourceWarning: vi.fn(),
  openSoundSettings: vi.fn(),
  onSnapshotError: vi.fn(),
  recordingStatus: vi.fn(),
  pauseRecording: vi.fn(),
  resumeRecording: vi.fn(),
  stopRecording: vi.fn(),
  takeSnapshot: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(),
  PhysicalSize: class {
    constructor(public width: number, public height: number) {}
  },
  PhysicalPosition: class {
    constructor(public x: number, public y: number) {}
  },
}));
vi.mock("./platform-capabilities", async (importOriginal) => ({ ...(await importOriginal<typeof import("./platform-capabilities")>()), usePlatformCapabilities: vi.fn() }));

const recording: RecorderStatus = {
  phase: "recording", meeting_id: "m1", elapsed_ms: 65_000,
  notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [],
};
let pushStatus: (status: RecorderStatus) => void = () => {};
const windowApi = { startDragging: vi.fn(), setSize: vi.fn(), setPosition: vi.fn(), innerSize: vi.fn(), outerPosition: vi.fn() };
let resized: () => void = () => {};
// jsdom has no layout, so each test states what the bar measures and how big the window is.
let barSize = { width: 266, height: 58 };
let windowSize = { width: 354, height: 58 };

vi.stubGlobal("ResizeObserver", class {
  constructor(private callback: () => void) {}
  observe() { resized = this.callback; this.callback(); }
  disconnect() {}
});

beforeEach(() => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: true, backgroundTranscription: true, snapshots: true });
  vi.mocked(getCurrentWindow).mockReturnValue(windowApi as unknown as ReturnType<typeof getCurrentWindow>);
  windowApi.startDragging.mockResolvedValue(undefined);
  windowApi.setSize.mockImplementation(async (size: { width: number; height: number }) => { windowSize = { width: size.width, height: size.height }; });
  windowApi.setPosition.mockResolvedValue(undefined);
  windowApi.innerSize.mockImplementation(async () => windowSize);
  windowApi.outerPosition.mockResolvedValue({ x: 1000, y: 24 });
  barSize = { width: 266, height: 58 };
  windowSize = { width: 354, height: 58 };
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    return this.getAttribute("aria-label") === "Capture bar" ? { ...barSize, x: 0, y: 0, top: 0, left: 0, right: barSize.width, bottom: barSize.height, toJSON() {} } : new DOMRect();
  });
  vi.mocked(onRecordingStatus).mockImplementation((handler) => {
    pushStatus = handler;
    return Promise.resolve(() => {});
  });
  vi.mocked(onSnapshotError).mockResolvedValue(() => {});
  vi.mocked(recordingStatus).mockResolvedValue(recording);
});

afterEach(() => { cleanup(); vi.resetAllMocks(); vi.restoreAllMocks(); vi.stubGlobal("innerWidth", 1024); });

it("follows the Recording and uses its pause, resume, stop and repeat Snapshot commands", async () => {
  vi.mocked(pauseRecording).mockResolvedValue({ ...recording, phase: "paused" });
  vi.mocked(resumeRecording).mockResolvedValue(recording);
  vi.mocked(stopRecording).mockResolvedValue({ ...recording, phase: "idle", meeting_id: null });
  vi.mocked(takeSnapshot).mockResolvedValue(undefined);
  render(<CaptureBarWindow />);

  expect((await screen.findByRole("status")).textContent).toContain("00:01:05");
  fireEvent.click(screen.getByRole("button", { name: "Take Snapshot" }));
  await waitFor(() => expect(takeSnapshot).toHaveBeenCalledWith(true));
  fireEvent.click(screen.getByRole("button", { name: "Pause" }));
  expect(await screen.findByRole("button", { name: "Resume" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Resume" }));
  expect(await screen.findByRole("button", { name: "Pause" })).toBeTruthy();
  act(() => pushStatus({ ...recording, elapsed_ms: 93_000 }));
  expect(screen.getByRole("status").textContent).toContain("00:01:33");
  fireEvent.click(screen.getByRole("button", { name: "Stop" }));
  await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));
  expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
});

it("hides the Snapshot control when unavailable", async () => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
  render(<CaptureBarWindow />);
  await screen.findByRole("status");
  expect(screen.queryByRole("button", { name: "Take Snapshot" })).toBeNull();
  expect(screen.getByRole("button", { name: "Pause" })).toBeTruthy();
  expect(onSnapshotError).not.toHaveBeenCalled();
});

it("collapses to a draggable timer and expands back to the controls", async () => {
  render(<CaptureBarWindow />);
  fireEvent.pointerDown(await screen.findByRole("button", { name: "Drag Capture bar" }));
  expect(windowApi.startDragging).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "Collapse Capture bar" }));
  expect(screen.queryByRole("button", { name: "Take Snapshot" })).toBeNull();
  expect(screen.getByRole("status").textContent).toContain("00:01:05");

  fireEvent.click(screen.getByRole("button", { name: "Expand Capture bar" }));
  expect(screen.getByRole("button", { name: "Take Snapshot" })).toBeTruthy();
});

it("sizes its window to the bar so display scaling cannot cut off the controls", async () => {
  // A 354 px wide window on a 200% display shows only 177 CSS px, less than the bar's 266.
  vi.stubGlobal("innerWidth", 177);
  windowSize = { width: 354, height: 116 };
  render(<CaptureBarWindow />);
  await waitFor(() => expect(windowApi.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 532, height: 116 })));
  // The right edge stays where it was.
  await waitFor(() => expect(windowApi.setPosition).toHaveBeenCalledWith(expect.objectContaining({ x: 1000 + 354 - 532, y: 24 })));
});

it("refits the window when the bar changes size, and leaves a fitting window alone", async () => {
  vi.stubGlobal("innerWidth", 354);
  render(<CaptureBarWindow />);
  await waitFor(() => expect(windowApi.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 266, height: 58 })));
  vi.stubGlobal("innerWidth", 266);
  windowApi.setSize.mockClear();

  fireEvent.click(screen.getByRole("button", { name: "Collapse Capture bar" }));
  barSize = { width: 150, height: 58 };
  resized();
  await waitFor(() => expect(windowApi.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 150, height: 58 })));
  vi.stubGlobal("innerWidth", 150);
  windowApi.setSize.mockClear();

  resized();
  await waitFor(() => expect(windowApi.innerSize).toHaveBeenCalled());
  expect(windowApi.setSize).not.toHaveBeenCalled();
});

it("shows the Microphone no signal warning and offers sound settings and Dismiss", async () => {
  const warning = {
    ...recording,
    source_devices: [{ source: "microphone" as const, device: "USB Microphone" }],
    source_health: [{ source: "microphone" as const, warning: "no_signal" as const }],
  };
  vi.mocked(recordingStatus).mockResolvedValue(warning);
  vi.mocked(openSoundSettings).mockResolvedValue(undefined);
  vi.mocked(dismissSourceWarning).mockResolvedValue({ ...warning, source_health: [{ source: "microphone", warning: null }] });
  render(<CaptureBarWindow />);

  expect((await screen.findByRole("alert")).textContent).toContain("Microphone has no signal");
  expect(screen.getByRole("alert").textContent).toContain("USB Microphone");
  fireEvent.click(screen.getByRole("button", { name: "Open sound settings" }));
  await waitFor(() => expect(openSoundSettings).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  await waitFor(() => expect(dismissSourceWarning).toHaveBeenCalledWith("microphone"));
  await waitFor(() => expect(screen.queryByText(/Microphone has no signal/)).toBeNull());
  expect(screen.getByRole("button", { name: "Stop" })).toBeTruthy();
});

it("explains when System audio may be missing and handles its warning actions", async () => {
  const warning = {
    ...recording,
    source_devices: [{ source: "system_audio" as const, device: "USB Headset" }],
    source_health: [{ source: "system_audio" as const, warning: "remote_silent" as const }],
  };
  vi.mocked(recordingStatus).mockResolvedValue(warning);
  vi.mocked(openSoundSettings).mockResolvedValue(undefined);
  vi.mocked(dismissSourceWarning).mockResolvedValue({ ...warning, source_health: [{ source: "system_audio", warning: null }] });
  render(<CaptureBarWindow />);

  expect((await screen.findByRole("alert")).textContent).toContain("the other side may not be captured");
  expect(screen.getByRole("alert").textContent).toContain("USB Headset");
  fireEvent.click(screen.getByRole("button", { name: "Open sound settings" }));
  await waitFor(() => expect(openSoundSettings).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
  await waitFor(() => expect(dismissSourceWarning).toHaveBeenCalledWith("system_audio"));
  await waitFor(() => expect(screen.queryByText(/the other side may not be captured/)).toBeNull());
});

it("widens for Source health warnings and shows each with its actions", async () => {
  const microphone = { source: "microphone" as const, warning: "no_signal" as const };
  const systemAudio = { source: "system_audio" as const, warning: "remote_silent" as const };
  const oneWarning = { ...recording, source_health: [microphone] };
  vi.mocked(recordingStatus).mockResolvedValue(oneWarning);
  render(<CaptureBarWindow />);
  expect((await screen.findByLabelText("Capture bar")).style.width).toBe("354px");

  act(() => pushStatus({ ...recording, source_health: [microphone, systemAudio] }));
  await waitFor(() => expect(screen.getAllByRole("alert")).toHaveLength(2));
  expect(screen.getAllByRole("button", { name: "Open sound settings" })).toHaveLength(2);
  expect(screen.getAllByRole("button", { name: "Dismiss" })).toHaveLength(2);

  act(() => pushStatus(recording));
  await waitFor(() => expect(screen.getByLabelText("Capture bar").style.width).toBe("max-content"));
});

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import RecordPromptWindow from "./RecordPromptWindow";
import { dismissRecordPrompt, getRecordPrompt, showRecordingNotice, silenceRecordPromptApp } from "./record-prompt";
import { recordingStatus, startRecording, type RecorderStatus } from "./recording";
import { usePlatformCapabilities } from "./platform-capabilities";

vi.mock("./record-prompt", () => ({
  dismissRecordPrompt: vi.fn(), getRecordPrompt: vi.fn(), showRecordingNotice: vi.fn(), silenceRecordPromptApp: vi.fn(),
}));
vi.mock("./recording", async (original) => ({
  ...await original<typeof import("./recording")>(), recordingStatus: vi.fn(), startRecording: vi.fn(),
}));
vi.mock("./platform-capabilities", () => ({ usePlatformCapabilities: vi.fn() }));

const idle: RecorderStatus = {
  phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true,
  saves: [], continue_offers: [], device_switches: [],
};

beforeEach(() => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: true, backgroundTranscription: true, snapshots: true });
  vi.mocked(getRecordPrompt).mockResolvedValue({ id: "teams", display_name: "Teams" });
  vi.mocked(recordingStatus).mockResolvedValue(idle);
  vi.mocked(startRecording).mockResolvedValue({ ...idle, phase: "recording", meeting_id: "m1" });
  vi.mocked(dismissRecordPrompt).mockResolvedValue(undefined);
  vi.mocked(showRecordingNotice).mockResolvedValue(undefined);
  vi.mocked(silenceRecordPromptApp).mockResolvedValue(undefined);
});

afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });

it("names the app and offers Record and Not now", async () => {
  render(<RecordPromptWindow />);
  expect((await screen.findByText(/Teams/)).textContent).toContain("Teams");
  expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Not now" }));
  await waitFor(() => expect(dismissRecordPrompt).toHaveBeenCalledOnce());
  expect(startRecording).not.toHaveBeenCalled();
});

it("does not query or show Record prompt on an unsupported platform", () => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
  render(<RecordPromptWindow />);
  expect(screen.queryByRole("region", { name: "Record prompt" })).toBeNull();
  expect(getRecordPrompt).not.toHaveBeenCalled();
});

it("Record uses the usual start command", async () => {
  render(<RecordPromptWindow />);
  fireEvent.click(await screen.findByRole("button", { name: "Record" }));
  await waitFor(() => expect(startRecording).toHaveBeenCalledOnce());
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
});

it("Never for this app silences the displayed app and closes the prompt", async () => {
  render(<RecordPromptWindow />);
  fireEvent.click(await screen.findByRole("button", { name: "Never for this app" }));
  await waitFor(() => expect(silenceRecordPromptApp).toHaveBeenCalledOnce());
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
  expect(startRecording).not.toHaveBeenCalled();
});

it("Record opens the main window's Recording notice when needed", async () => {
  vi.mocked(recordingStatus).mockResolvedValue({ ...idle, notice_acknowledged: false });
  render(<RecordPromptWindow />);
  fireEvent.click(await screen.findByRole("button", { name: "Record" }));
  await waitFor(() => expect(showRecordingNotice).toHaveBeenCalledOnce());
  expect(startRecording).not.toHaveBeenCalled();
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
});

it("closes after about twenty seconds without starting a Recording", async () => {
  vi.useFakeTimers();
  render(<RecordPromptWindow />);
  await act(async () => {});
  expect(screen.getByText(/Teams/)).toBeTruthy();
  await act(async () => { await vi.advanceTimersByTimeAsync(20_000); });
  expect(dismissRecordPrompt).toHaveBeenCalledOnce();
  expect(startRecording).not.toHaveBeenCalled();
});

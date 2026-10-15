import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FirstRun from "./FirstRun";
import { acknowledgeRecordingNotice } from "./recording";
import { modelDownloadStatus, onModelDownloadStatus, startModelDownload, updateSettings, type Settings } from "./processing";

vi.mock("./recording", async (original) => ({ ...(await original<typeof import("./recording")>()), acknowledgeRecordingNotice: vi.fn() }));
vi.mock("./processing", async (original) => ({ ...(await original<typeof import("./processing")>()), modelDownloadStatus: vi.fn(), onModelDownloadStatus: vi.fn(), startModelDownload: vi.fn(), updateSettings: vi.fn() }));

const settings: Settings = { first_run_completed: false, recording_notice_acknowledged: false, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } };
let emitDownload: (status: { model_id: string; state: "downloading"; progress: number; error: null }) => void;

beforeEach(() => {
  vi.mocked(modelDownloadStatus).mockResolvedValue([]);
  vi.mocked(onModelDownloadStatus).mockImplementation(async (handler) => { emitDownload = handler; return () => {}; });
  vi.mocked(startModelDownload).mockResolvedValue([{ model_id: "small", state: "downloading", progress: 0, error: null }]);
  vi.mocked(updateSettings).mockResolvedValue(settings);
  vi.mocked(acknowledgeRecordingNotice).mockResolvedValue({ phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("lets every step be skipped without acknowledging the Recording notice", async () => {
  const finish = vi.fn();
  render(<FirstRun settings={settings} discreteGpu={false} onFinish={finish} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByRole("heading", { name: "Set up a local model" })).toBeTruthy();
  expect(screen.getByText(/Small/)).toBeTruthy();
  expect(screen.getByText(/488 MB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByRole("heading", { name: "Library location" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  await waitFor(() => expect(finish).toHaveBeenCalledOnce());
  expect(acknowledgeRecordingNotice).not.toHaveBeenCalled();
});

it("recommends turbo for a discrete GPU or non-English language and shows download progress", async () => {
  render(<FirstRun settings={{ ...settings, language: "vi" }} discreteGpu={false} onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByText(/Large v3 turbo/)).toBeTruthy();
  expect(screen.getByText(/574 MB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download now" }));
  await waitFor(() => expect(startModelDownload).toHaveBeenCalledWith("large-v3-turbo-q5_0"));
  expect(updateSettings).toHaveBeenCalledWith(true, "large-v3-turbo-q5_0", [], "vi", null, undefined);
  emitDownload({ model_id: "large-v3-turbo-q5_0", state: "downloading", progress: 37, error: null });
  expect(await screen.findByText(/37%/)).toBeTruthy();
  expect(screen.getByText(/record while it downloads/i)).toBeTruthy();
});

it("acknowledges the Recording notice without starting a Recording", async () => {
  render(<FirstRun settings={settings} discreteGpu={false} onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "I understand" }));
  await waitFor(() => expect(acknowledgeRecordingNotice).toHaveBeenCalledOnce());
  expect(screen.getByRole("heading", { name: "Set up a local model" })).toBeTruthy();
});

it("opens Keys when the user chooses their own key", async () => {
  const useKey = vi.fn();
  render(<FirstRun settings={settings} discreteGpu={true} onFinish={vi.fn()} onUseKey={useKey} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByText(/Large v3 turbo/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "I'll use my own key" }));
  await waitFor(() => expect(useKey).toHaveBeenCalledOnce());
});

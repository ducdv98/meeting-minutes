import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FirstRun from "./FirstRun";
import { acknowledgeRecordingNotice } from "./recording";
import { modelDownloadStatus, onModelDownloadStatus, openLibraryFolder, startModelDownload, updateSettings, type Settings } from "./processing";
import { usePlatformCapabilities } from "./platform-capabilities";

vi.mock("./recording", async (original) => ({ ...(await original<typeof import("./recording")>()), acknowledgeRecordingNotice: vi.fn() }));
vi.mock("./processing", async (original) => ({ ...(await original<typeof import("./processing")>()), modelDownloadStatus: vi.fn(), onModelDownloadStatus: vi.fn(), openLibraryFolder: vi.fn(), startModelDownload: vi.fn(), updateSettings: vi.fn() }));
vi.mock("./platform-capabilities", async (importOriginal) => ({ ...(await importOriginal<typeof import("./platform-capabilities")>()), usePlatformCapabilities: vi.fn() }));

const settings: Settings = { first_run_completed: false, recording_notice_acknowledged: false, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } };
let emitDownload: (status: { model_id: string; state: "downloading"; progress: number; error: null }) => void;

beforeEach(() => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: true, backgroundTranscription: true, snapshots: true });
  vi.mocked(modelDownloadStatus).mockResolvedValue([]);
  vi.mocked(onModelDownloadStatus).mockImplementation(async (handler) => { emitDownload = handler; return () => {}; });
  vi.mocked(startModelDownload).mockResolvedValue([{ model_id: "small", state: "downloading", progress: 0, error: null }]);
  vi.mocked(updateSettings).mockResolvedValue(settings);
  vi.mocked(openLibraryFolder).mockResolvedValue();
  vi.mocked(acknowledgeRecordingNotice).mockResolvedValue({ phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] });
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("lets every step be skipped without acknowledging the Recording notice", async () => {
  const finish = vi.fn();
  render(<FirstRun settings={settings} discreteGpu={false} libraryLocation="D:\\Meetings" onFinish={finish} onUseKey={vi.fn()} />);
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

it("shows only the supported Recording notice on macOS", () => {
  vi.mocked(usePlatformCapabilities).mockReturnValue({ recordPrompt: false, backgroundTranscription: false, snapshots: false });
  render(<FirstRun settings={settings} discreteGpu={false} libraryLocation="/Users/test/Documents/Meeting Minutes" onFinish={vi.fn()} onUseKey={vi.fn()} />);
  const notice = screen.getByText(/Other people on the call/).textContent;
  expect(notice).not.toMatch(/Snapshots|offer a Recording|checks locally/);
});

it("recommends turbo for a discrete GPU or non-English language and shows download progress", async () => {
  render(<FirstRun settings={{ ...settings, language: "vi" }} discreteGpu={false} libraryLocation="D:\\Meetings" onFinish={vi.fn()} onUseKey={vi.fn()} />);
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

it("saves the selected default Meeting language and updates the CPU recommendation", async () => {
  render(<FirstRun settings={settings} discreteGpu={false} libraryLocation={"D:\\Meetings"} onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  const language = screen.getByRole("combobox", { name: "Meetings are mostly in" }) as HTMLSelectElement;
  expect(language.value).toBe("");
  fireEvent.change(language, { target: { value: "vi" } });
  await waitFor(() => expect(updateSettings).toHaveBeenCalledWith(true, "small", [], "vi", null, undefined));
  expect(language.value).toBe("vi");
  expect(screen.getByText(/Large v3 turbo/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Download now" }));
  await waitFor(() => expect(startModelDownload).toHaveBeenCalledWith("large-v3-turbo-q5_0"));
  expect(updateSettings).toHaveBeenLastCalledWith(true, "large-v3-turbo-q5_0", [], "vi", null, undefined);
});

it("opens the resolved Library folder from setup", async () => {
  render(<FirstRun settings={settings} discreteGpu={false} libraryLocation={"D:\\Meetings"} onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByText("D:\\Meetings")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
  await waitFor(() => expect(openLibraryFolder).toHaveBeenCalledOnce());
});

it("keeps the downloaded model selected when the Meeting language changes again", async () => {
  render(<FirstRun settings={{ ...settings, language: "vi" }} discreteGpu={false} libraryLocation={"D:\\Meetings"} onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  fireEvent.click(screen.getByRole("button", { name: "Download now" }));
  await waitFor(() => expect(startModelDownload).toHaveBeenCalledWith("large-v3-turbo-q5_0"));
  fireEvent.change(screen.getByRole("combobox", { name: "Meetings are mostly in" }), { target: { value: "en" } });
  await waitFor(() => expect(updateSettings).toHaveBeenLastCalledWith(true, "large-v3-turbo-q5_0", [], "en", null, undefined));
});

it("acknowledges the Recording notice without starting a Recording", async () => {
  render(<FirstRun settings={settings} discreteGpu={false} libraryLocation="D:\\Meetings" onFinish={vi.fn()} onUseKey={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "I understand" }));
  await waitFor(() => expect(acknowledgeRecordingNotice).toHaveBeenCalledOnce());
  expect(screen.getByRole("heading", { name: "Set up a local model" })).toBeTruthy();
});

it("opens Keys when the user chooses their own key", async () => {
  const useKey = vi.fn();
  render(<FirstRun settings={settings} discreteGpu={true} libraryLocation="D:\\Meetings" onFinish={vi.fn()} onUseKey={useKey} />);
  fireEvent.click(screen.getByRole("button", { name: "Skip" }));
  expect(screen.getByText(/Large v3 turbo/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "I'll use a cloud key" }));
  await waitFor(() => expect(useKey).toHaveBeenCalledOnce());
});

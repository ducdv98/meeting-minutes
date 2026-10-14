import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import MeetingView from "./MeetingView";
import { deleteMeeting, meetingDetail, openMeetingFolder, renameMeeting, setParticipantName, participantNames, renameParticipant, removeParticipant, assignSegments, type MeetingDetail } from "./library";
import { convertFileSrc } from "@tauri-apps/api/core";
import { onLibraryChanged } from "./recording";
import { processingStatus, meetingTranscript, enqueueTranscription, cancelProcessing, onProcessingStatus, onTranscriptDraft, transcriptDraft, setMeetingLanguage, getSettings, type ProcessingStatus } from "./processing";
import { editSegment } from "./transcript";
import { aiStatus } from "./ai";
import { meetingSummary, summaryStatus } from "./summary";
import { meetingAsks } from "./ask";

vi.mock("./ai", async (original) => ({ ...(await original<typeof import("./ai")>()), aiStatus: vi.fn() }));
vi.mock("./summary", () => ({ meetingSummary: vi.fn(), summaryParticipantsChanged: vi.fn(() => Promise.resolve(false)), summaryStatus: vi.fn(), onSummaryStatus: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./ask", () => ({ meetingAsks: vi.fn(), onAskDelta: vi.fn(() => Promise.resolve(() => {})), onAsksChanged: vi.fn(() => Promise.resolve(() => {})), answerLabel: () => "" }));
vi.mock("./library", () => ({ meetingDetail: vi.fn(), openMeetingFolder: vi.fn(), renameMeeting: vi.fn(), deleteMeeting: vi.fn(), setParticipantName: vi.fn(), participantNames: vi.fn(), renameParticipant: vi.fn(), removeParticipant: vi.fn(), assignSegments: vi.fn() }));
vi.mock("./processing", async (importOriginal) => ({ ...(await importOriginal<typeof import("./processing")>()), processingStatus: vi.fn(), meetingTranscript: vi.fn(), enqueueTranscription: vi.fn(), cancelProcessing: vi.fn(), dismissProcessing: vi.fn(), setMeetingLanguage: vi.fn(), getSettings: vi.fn(), transcriptDraft: vi.fn(), onProcessingStatus: vi.fn(() => Promise.resolve(() => {})), onTranscriptDraft: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./recording", async (importOriginal) => ({ ...(await importOriginal<typeof import("./recording")>()), onLibraryChanged: vi.fn(() => Promise.resolve(() => {})) }));
vi.mock("./transcript", async (importOriginal) => ({ ...(await importOriginal<typeof import("./transcript")>()), editSegment: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: vi.fn((path: string) => `asset:${path}`) }));

const detail: MeetingDetail = {
  id: "old", title: "Planning", started_at: "2026-10-12T09:30:00Z",
  duration_ms: 125_000, language: "vi", audio_path: "C:\\Meetings\\meeting.mp3",
  imported: false,
  voice_note: false,
  participants: {},
  markers: [
    { kind: "pause", at_ms: 10_000, paused_ms: 125_000 },
    { kind: "gap", at_ms: 20_000 },
    { kind: "device_switch", at_ms: 30_000, source: "microphone", device: "Headset" },
  ],
};

beforeEach(() => {
  vi.mocked(meetingSummary).mockResolvedValue(null);
  vi.mocked(summaryStatus).mockResolvedValue({ jobs: [] });
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [], active: null, retryable_id: null });
  vi.mocked(aiStatus).mockResolvedValue({ provider: null, model: null, configured: false, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false });
  vi.mocked(meetingDetail).mockResolvedValue(detail);
  vi.mocked(participantNames).mockResolvedValue(["Anna"]);
  vi.mocked(setParticipantName).mockResolvedValue();
  vi.mocked(renameParticipant).mockResolvedValue();
  vi.mocked(removeParticipant).mockResolvedValue();
  vi.mocked(assignSegments).mockResolvedValue();
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: [], language: null, your_name: "Duc", ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } });
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [] });
  vi.mocked(transcriptDraft).mockResolvedValue([]);
  vi.mocked(meetingTranscript).mockResolvedValue(null);
  vi.mocked(enqueueTranscription).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "queued", progress: 0, error: null }] });
  vi.mocked(cancelProcessing).mockResolvedValue({ jobs: [] });
  vi.mocked(setMeetingLanguage).mockResolvedValue();
  vi.mocked(deleteMeeting).mockResolvedValue();
  vi.mocked(openMeetingFolder).mockResolvedValue();
  vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(async () => {});
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.resetAllMocks(); });

it("shows why part of the Meeting used local transcription", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["Groq", "small"], fallback_reason: "Groq daily limit reached", language: "en", language_detected: false, duration_ms: 1000, segments: [] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Part of this Meeting was transcribed locally: Groq daily limit reached.")).toBeTruthy();
});

it("says local when a cached local span precedes a Gemini fallback", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["small", "Groq", "Gemini"], fallback_reason: "Groq daily limit reached", language: "en", language_detected: false, duration_ms: 1000, segments: [] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Part of this Meeting was transcribed locally: Groq daily limit reached.")).toBeTruthy();
});

it("names the cloud fallback engine in the Meeting notice", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["Groq", "Gemini"], fallback_reason: "Groq daily limit reached", language: "en", language_detected: false, duration_ms: 1000, segments: [] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Part of this Meeting was transcribed with Gemini: Groq daily limit reached.")).toBeTruthy();
});

it("shows a Managed fallback notice when Groq finishes the Meeting", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["Managed transcription", "Groq"], fallback_reason: "Managed transcription was disconnected", language: "en", language_detected: false, duration_ms: 1000, segments: [] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Part of this Meeting was transcribed with Groq: Managed transcription was disconnected.")).toBeTruthy();
});

it("shows the Correction failure notice for the published Revision", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, correction_notice: "Correction was unavailable; this Revision has uncorrected text." });
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 1000, segments: [{ id: "s00001", start_ms: 0, end_ms: 800, text: "Uncorrected", speaker: "me" }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Correction was unavailable; this Revision has uncorrected text.")).toBeTruthy();
});

it("names Participants for present Speaker labels while Processing is busy", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.5, error: null }] });
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 2000, segments: [
    { id: "s00001", start_ms: 0, end_ms: 500, text: "Hello", speaker: "me" },
    { id: "s00002", start_ms: 600, end_ms: 1000, text: "Hi", speaker: "remote" },
  ] });
  vi.mocked(meetingDetail).mockResolvedValueOnce(detail).mockResolvedValue({ ...detail, participants: { remote: "Anna" } });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Participants");
  const remote = await screen.findByRole("combobox", { name: "Remote Participant" });
  expect((remote as HTMLInputElement).disabled).toBe(false);
  fireEvent.change(remote, { target: { value: "Anna" } });
  fireEvent.blur(remote);
  await waitFor(() => expect(renameParticipant).toHaveBeenCalledWith("old", "Remote", "Anna"));
  expect(await screen.findByText("Anna")).toBeTruthy();
  expect(screen.queryByRole("combobox", { name: "Unknown Participant" })).toBeNull();
});

it("names only the user in a Voice note with Unknown Segments", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, sources: ["microphone"], voice_note: true });
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, cloud_notice_acknowledged: false, auto_process: true, model: "small", vocabulary: [], language: null, your_name: "Duc", ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } });
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 1000, segments: [{ id: "s00001", start_ms: 0, end_ms: 500, text: "Note to self", speaker: "unknown" }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  const me = await screen.findByRole("combobox", { name: "Me Participant" });
  await waitFor(() => expect((me as HTMLInputElement).value).toBe("Duc"));
  expect(screen.queryByRole("checkbox", { name: "Select Segment s00001" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "Unknown Participant" })).toBeNull();
  fireEvent.change(me, { target: { value: "Duc D" } });
  fireEvent.blur(me);
  await waitFor(() => expect(renameParticipant).toHaveBeenCalledWith("old", "Duc", "Duc D"));
});

it("renames a Segment Participant onto an existing name and removes it", async () => {
  vi.mocked(meetingDetail).mockResolvedValueOnce({ ...detail, participants: { remote: "Anna" }, segment_participants: { revision: 1, segments: { s00002: "Bao" } } }).mockResolvedValue({ ...detail, participants: { remote: "Anna" }, segment_participants: null });
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 2000, segments: [
    { id: "s00001", start_ms: 0, end_ms: 800, text: "One", speaker: "remote" },
    { id: "s00002", start_ms: 900, end_ms: 1700, text: "Two", speaker: "remote" },
  ] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  const bao = await screen.findByRole("combobox", { name: "Bao Participant" });
  fireEvent.change(bao, { target: { value: "Anna" } });
  fireEvent.blur(bao);
  await waitFor(() => expect(renameParticipant).toHaveBeenCalledWith("old", "Bao", "Anna"));
  expect(screen.queryByRole("combobox", { name: "Bao Participant" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Remove Anna" }));
  await waitFor(() => expect(removeParticipant).toHaveBeenCalledWith("old", "Anna"));
});

it("moves selected Segments through the Meeting command", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 3, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 1000, segments: [{ id: "s00001", start_ms: 0, end_ms: 800, text: "One", speaker: "unknown" }] });
  vi.mocked(meetingDetail).mockResolvedValueOnce(detail).mockResolvedValue({ ...detail, segment_participants: { revision: 3, segments: { s00001: "Bao" } } });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  fireEvent.click(await screen.findByRole("checkbox", { name: "Select Segment s00001" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Move to Participant" }), { target: { value: "__new__" } });
  fireEvent.change(screen.getByRole("textbox", { name: "New Participant name" }), { target: { value: "Bao" } });
  fireEvent.click(screen.getByRole("button", { name: "Move" }));
  await waitFor(() => expect(assignSegments).toHaveBeenCalledWith("old", 3, ["s00001"], "Bao"));
  expect(await screen.findByRole("combobox", { name: "Bao Participant" })).toBeTruthy();
});

it("plays and highlights a search result once the Transcript and player load", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 1000, end_ms: 2000, text: "Budget", speaker: "me" }] });
  const onOpened = vi.fn();
  const { container } = render(<MeetingView id="old" openAt={{ segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000 }} onOpened={onOpened} onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await waitFor(() => expect(onOpened).toHaveBeenCalledTimes(1));
  expect(container.querySelector("audio")?.currentTime).toBe(0.5);
  await waitFor(() => expect(container.querySelector("li[aria-current='true']")?.textContent).toContain("Budget"));
  expect(HTMLMediaElement.prototype.play).toHaveBeenCalledTimes(1);
});

it("does not play a search hit from an older Revision", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 8000, end_ms: 9000, text: "Different span", speaker: "me" }] });
  const onOpened = vi.fn();
  render(<MeetingView id="old" openAt={{ segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000 }} onOpened={onOpened} onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await waitFor(() => expect(onOpened).toHaveBeenCalledTimes(1));
  expect(screen.getByText("This search result is from an older transcript revision.")).toBeTruthy();
  expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();
  expect(screen.queryByRole("alert")).toBeNull();
});

it("does not play a search hit whose Segment is absent", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 125000, segments: [{ id: "s00002", start_ms: 8000, end_ms: 9000, text: "Another span", speaker: "me" }] });
  const onOpened = vi.fn();
  render(<MeetingView id="old" openAt={{ segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000 }} onOpened={onOpened} onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await waitFor(() => expect(onOpened).toHaveBeenCalledTimes(1));
  expect(screen.getByText("This search result is from an older transcript revision.")).toBeTruthy();
  expect(HTMLMediaElement.prototype.play).not.toHaveBeenCalled();
});

it("confirms Meeting deletion after releasing the player and reports success", async () => {
  const onDeleted = vi.fn();
  vi.mocked(deleteMeeting).mockImplementation(async () => { expect(document.querySelector("audio")).toBeNull(); });
  render(<MeetingView id="old" onDeleted={onDeleted} onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.pointerDown(screen.getByRole("button", { name: "More actions" }), { button: 0, ctrlKey: false });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Delete" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" }));
  await waitFor(() => expect(onDeleted).toHaveBeenCalledTimes(1));
  expect(deleteMeeting).toHaveBeenCalledWith("old");
});

it("shows the Meeting fields and Timeline markers", async () => {
  const { container } = render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByRole("heading", { name: "Planning" })).toBeTruthy();
  expect(container.querySelector("time")?.getAttribute("datetime")).toBe(detail.started_at);
  expect(screen.getByText(/2 min/)).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement).value).toBe("vi");
  expect(convertFileSrc).toHaveBeenCalledWith(detail.audio_path);
  expect(container.querySelector("audio")?.getAttribute("src")).toBe(`asset:${detail.audio_path}`);
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).max).toBe("125000");
  expect(screen.getByRole("button", { name: "Paused for 2m 05s at 00:00:10" }).getAttribute("data-kind")).toBe("pause");
  expect(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Microphone switched to Headset at 00:00:30" }).getAttribute("style")).toContain("24%");
  fireEvent.click(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" }));
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).value).toBe("20000");
});

it("shows recorded Sources and opens the Meeting folder", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, sources: ["microphone", "system_audio"] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Microphone + System audio")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Open folder" }));
  await waitFor(() => expect(openMeetingFolder).toHaveBeenCalledWith("old"));
});

it("highlights the cited Timeline span when a Summary Citation plays", async () => {
  vi.mocked(aiStatus).mockResolvedValue({ provider: "ollama", model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false });
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", model: ["small"], language: "en", language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 1000, end_ms: 2500, text: "Budget approved", speaker: "me" }] });
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", provider: "ollama", model: "llama", overview: [{ text: "Approved", citations: [{ segment_id: "s00001", start_ms: 1000, end_ms: 2500 }] }], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  fireEvent.click(await screen.findByRole("button", { name: /Play from 00:01/ }));
  expect(screen.getByTestId("cited-span").getAttribute("style")).toContain("left:");
  expect(HTMLMediaElement.prototype.play).toHaveBeenCalled();
});

it("renames the Meeting while keeping audio mounted and its id unchanged", async () => {
  const onRenamed = vi.fn();
  vi.mocked(renameMeeting).mockImplementation(async () => {
    expect(document.querySelector("audio")).toBeTruthy();
    return { id: "old", title: "Review", started_at: detail.started_at, duration_ms: detail.duration_ms };
  });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={onRenamed} />);
  await screen.findByRole("heading", { name: "Planning" });
  const audio = document.querySelector("audio");
  fireEvent.click(screen.getByRole("button", { name: "Rename Planning" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: " Review " } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("heading", { name: "Review" })).toBeTruthy();
  expect(renameMeeting).toHaveBeenCalledWith("old", "Review");
  expect(onRenamed).toHaveBeenCalledTimes(1);
  expect(document.querySelector("audio")).toBe(audio);
  expect(meetingDetail).toHaveBeenCalledTimes(1);
});

it("keeps the rename input open and shows an error on failure", async () => {
  vi.mocked(renameMeeting).mockRejectedValue("Could not save Title");
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename Planning" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not save Title");
  expect(screen.getByRole("textbox", { name: "Title" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
});

it("shows no audio, disables empty saves, and handles Escape", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, language: null, audio_path: null });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("No audio yet")).toBeTruthy();
  expect((screen.getByRole("combobox", { name: "Language" }) as HTMLSelectElement).value).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "Rename Planning" }));
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: "   " } });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.keyDown(title, { key: "Escape" });
  await waitFor(() => expect(screen.getByRole("heading", { name: "Planning" })).toBeTruthy());
});

it("enqueues transcription and can cancel a queued job", async () => {
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  fireEvent.click(await screen.findByRole("button", { name: "Transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old");
  expect(await screen.findByText("Waiting to transcribe")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel processing" }));
  await waitFor(() => expect(cancelProcessing).toHaveBeenCalledWith("old"));
});

it("shows download progress, failure actions, and a ready revision", async () => {
  const downloading: ProcessingStatus = { jobs: [{ meeting_id: "old", model: null, stage: "downloading", progress: 0.34, error: null }] };
  vi.mocked(processingStatus).mockResolvedValue(downloading);
  const { rerender } = render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Downloading model 34%")).toBeTruthy();
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "failed", progress: 0.34, error: "Model unavailable" }] });
  rerender(<MeetingView id="new" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  rerender(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect((await screen.findByRole("alert")).textContent).toContain("Model unavailable");
  expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Dismiss" })).toBeTruthy();
});

it("shows transcript metadata and changes the Meeting language", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 2, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "vi", language_detected: true, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Transcript revision 2 · model small · 1 segments · Vietnamese")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Re-transcribe" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old");
  fireEvent.change(screen.getByRole("combobox", { name: "Language" }), { target: { value: "en" } });
  await waitFor(() => expect(setMeetingLanguage).toHaveBeenCalledWith("old", "en"));
});

it("re-transcribes with the chosen model and edits the current Revision", async () => {
  const original = { schema_version: 1, revision: 2, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en" as const, language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" as const }] };
  vi.mocked(meetingTranscript).mockResolvedValue(original);
  vi.mocked(editSegment).mockResolvedValue({ ...original, segments: [{ ...original.segments[0], text: "Updated" }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Segment text" }), { target: { value: "Updated" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByText("Updated")).toBeTruthy();
  expect(editSegment).toHaveBeenCalledWith("old", 2, "s00001", "Updated");
  fireEvent.change(screen.getByRole("combobox", { name: "Model" }), { target: { value: "large-v3-turbo-q5_0" } });
  fireEvent.click(screen.getByRole("button", { name: "Re-transcribe" }));
  expect(enqueueTranscription).toHaveBeenCalledWith("old", "large-v3-turbo-q5_0");
});

it("closes an open Segment editor and keeps the newer Revision when one arrives", async () => {
  const r1 = { schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en" as const, language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 0, end_ms: 1000, text: "Hello", speaker: "me" as const }] };
  const r2 = { ...r1, revision: 2, segments: [{ ...r1.segments[0], text: "Hello again" }] };
  vi.mocked(meetingTranscript).mockResolvedValue(r1);
  let resolveEdit!: (transcript: typeof r1) => void;
  vi.mocked(editSegment).mockReturnValue(new Promise((resolve) => { resolveEdit = resolve; }));
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Hello");
  fireEvent.click(screen.getByRole("button", { name: "Edit" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Segment text" }), { target: { value: "Edited" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  vi.mocked(meetingTranscript).mockResolvedValue(r2);
  await act(async () => { vi.mocked(onLibraryChanged).mock.calls[0][0](); });
  expect(await screen.findByText("Hello again")).toBeTruthy();
  expect(screen.queryByRole("textbox", { name: "Segment text" })).toBeNull();
  await act(async () => resolveEdit({ ...r1, segments: [{ ...r1.segments[0], text: "Edited" }] }));
  expect(screen.getByText("Hello again")).toBeTruthy();
  expect(screen.queryByText("Edited")).toBeNull();
});

it("shows transcription progress and reloads a completed transcript", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "transcribing", progress: 0.61, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Transcribing 61%")).toBeTruthy();
  vi.mocked(meetingTranscript).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en", language_detected: false, duration_ms: 125000, segments: [] });
  await act(async () => { vi.mocked(onLibraryChanged).mock.calls[0][0](); });
  expect(await screen.findByText("Transcript revision 1 · model small · 0 segments · English")).toBeTruthy();
});

it("keeps a newer processing event when the initial snapshot arrives late", async () => {
  let resolveSnapshot!: (status: ProcessingStatus) => void;
  vi.mocked(processingStatus).mockReturnValue(new Promise((resolve) => { resolveSnapshot = resolve; }));
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  const event = vi.mocked(onProcessingStatus).mock.calls[0][0];
  await act(async () => event({ jobs: [{ meeting_id: "old", model: null, stage: "transcribing", progress: 0.61, error: null }] }));
  expect(screen.getByText("Transcribing 61%")).toBeTruthy();
  await act(async () => resolveSnapshot({ jobs: [{ meeting_id: "old", model: null, stage: "queued", progress: 0, error: null }] }));
  expect(screen.getByText("Transcribing 61%")).toBeTruthy();
  expect(screen.queryByText("Waiting to transcribe")).toBeNull();
});

it("keeps playback available when rename fails", async () => {
  vi.mocked(renameMeeting).mockRejectedValue("Could not write meeting.json");
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  const audio = document.querySelector("audio");
  fireEvent.click(screen.getByRole("button", { name: "Rename Planning" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not write meeting.json");
  expect(document.querySelector("audio")).toBe(audio);
});

it("labels an imported Meeting below its date", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, imported: true });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Imported audio")).toBeTruthy();
});

it("guides setup when no AI provider is configured", async () => {
  const onOpenSettings = vi.fn();
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} onOpenSettings={onOpenSettings} />);
  expect(await screen.findByRole("heading", { name: "Summary and Ask" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Open AI settings" }));
  expect(onOpenSettings).toHaveBeenCalledOnce();
});
it("keeps saved Summary and Ask history visible without a configured provider", async () => {
  vi.mocked(meetingSummary).mockResolvedValue({ schema_version: 1, revision: 1, created_at: "now", provider: "ollama", model: "llama", overview: [{ text: "Budget approved", citations: [{ segment_id: "s00001", start_ms: 1000, end_ms: 2000 }] }], key_points: [], decisions: [], action_items: [], dropped_items: 0 });
  vi.mocked(meetingAsks).mockResolvedValue({ asks: [{ id: "a0001", question: "What happened?", asked_at: "now", revision: 1, provider: "ollama", model: "llama", state: "answered", answer: "Budget approved", citations: [], invalid_citations: [], error: null }], active: null, retryable_id: null });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} onOpenSettings={() => {}} />);
  expect(await screen.findByRole("heading", { name: "Summary and Ask" })).toBeTruthy();
  expect(await screen.findByText("Budget approved")).toBeTruthy();
  fireEvent.click(screen.getByRole("tab", { name: "Ask" }));
  expect(await screen.findByText("Budget approved")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Generate Summary" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Ask" })).toBeNull();
});

it("hides setup guidance when a provider is configured", async () => {
  vi.mocked(aiStatus).mockResolvedValue({ provider: "ollama", model: "llama3.2", configured: true, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} onOpenSettings={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  await waitFor(() => expect(aiStatus).toHaveBeenCalled());
  expect(screen.queryByRole("button", { name: "Open AI settings" })).toBeNull();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

const transcriptWith = (revision: number, text: string) => ({ schema_version: 1, revision, created_at: "2026-10-12T10:00:00Z", model: ["small"], language: "en" as const, language_detected: false, duration_ms: 125000, segments: [{ id: "s00001", start_ms: 1000, end_ms: 2000, text, speaker: "me" as const }] });

it("shows arriving Draft Segments instead of the old Revision and restores it on cancel", async () => {
  vi.mocked(meetingTranscript).mockResolvedValue(transcriptWith(1, "Old Revision words"));
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, participants: { remote: "Anna" } });
  vi.mocked(aiStatus).mockResolvedValue({ provider: "ollama", model: "llama", configured: true, has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false });
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.2, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Old Revision words");
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [{ start_ms: 1000, end_ms: 2000, text: "Draft words", speaker: "remote" }] }));
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "other", segments: [{ start_ms: 0, end_ms: 500, text: "Other Meeting", speaker: "me" }] }));
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [{ start_ms: 500, end_ms: 900, text: "Earlier words", speaker: "me" }] }));
  expect(within(screen.getByRole("list", { name: "Draft Segments" })).getAllByRole("listitem").map((row) => row.textContent)).toEqual(["00:00DucEarlier words", "00:01AnnaDraft words"]);
  expect(screen.queryByText("Other Meeting")).toBeNull();
  expect(screen.getByText("Draft words")).toBeTruthy();
  expect(screen.getByText("Anna")).toBeTruthy();
  expect(screen.queryByText("Old Revision words")).toBeNull();
  expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();
  expect(screen.queryByRole("checkbox", { name: /Select Segment/ })).toBeNull();
  expect(screen.getByText("Draft · Transcribing…")).toBeTruthy();
  expect(screen.queryByRole("tab", { name: "Ask" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Generate Summary" })).toBeNull();
  expect(screen.queryByRole("button", { name: /Citation/ })).toBeNull();
  await act(async () => vi.mocked(onLibraryChanged).mock.calls[0][0]());
  expect(screen.getByText("Draft words")).toBeTruthy();
  await act(async () => vi.mocked(onProcessingStatus).mock.calls[0][0]({ jobs: [] }));
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [{ start_ms: 3000, end_ms: 4000, text: "Late Draft words", speaker: "me" }] }));
  expect(screen.queryByText("Draft words")).toBeNull();
  expect(screen.queryByText("Earlier words")).toBeNull();
  expect(screen.queryByText("Late Draft words")).toBeNull();
  expect(screen.getByText("Old Revision words")).toBeTruthy();
});

it("shows the stored Draft when a Meeting opens mid-Processing and appends later Segments once", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.4, error: null }] });
  vi.mocked(transcriptDraft).mockResolvedValue([
    { start_ms: 0, end_ms: 900, text: "Stored first", speaker: "me" },
    { start_ms: 1000, end_ms: 2000, text: "Stored second", speaker: "remote" },
  ]);
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("Stored first")).toBeTruthy();
  expect(transcriptDraft).toHaveBeenCalledWith("old");
  expect(screen.getByText("Draft · Transcribing…")).toBeTruthy();
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [
    { start_ms: 1000, end_ms: 2000, text: "Stored second", speaker: "remote" },
    { start_ms: 2500, end_ms: 3000, text: "Arrived later", speaker: "me" },
  ] }));
  expect(within(screen.getByRole("list", { name: "Draft Segments" })).getAllByRole("listitem").map((row) => row.textContent)).toEqual(["00:00DucStored first", "00:01RemoteStored second", "00:02DucArrived later"]);
});

it("does not fetch a stored Draft when the Meeting is not transcribing", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "queued", progress: 0, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  await waitFor(() => expect(processingStatus).toHaveBeenCalled());
  expect(transcriptDraft).not.toHaveBeenCalled();
});

it("replaces the Draft with the published Revision and clears it on failure", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.2, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [{ start_ms: 1000, end_ms: 2000, text: "Draft words", speaker: "me" }] }));
  expect(screen.getByText("Draft words")).toBeTruthy();
  await act(async () => vi.mocked(onProcessingStatus).mock.calls[0][0]({ jobs: [{ meeting_id: "old", model: "small", stage: "failed", progress: 0.2, error: "Failed" }] }));
  expect(screen.queryByText("Draft words")).toBeNull();
  await act(async () => vi.mocked(onProcessingStatus).mock.calls[0][0]({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.3, error: null }] }));
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [{ start_ms: 2000, end_ms: 3000, text: "New Draft words", speaker: "me" }] }));
  vi.mocked(meetingTranscript).mockResolvedValue(transcriptWith(2, "Published words"));
  await act(async () => vi.mocked(onLibraryChanged).mock.calls[0][0]());
  expect(await screen.findByText("Published words")).toBeTruthy();
  expect(screen.queryByText("New Draft words")).toBeNull();
});

it("aligns the published Revision with the nearest visible Draft Segment", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: "small", stage: "transcribing", progress: 0.5, error: null }] });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const start = Number(this.dataset.segmentStart);
    const top = this.getAttribute("aria-label") === "Transcript" ? 100 : start === 1000 ? 120 : start === 2000 ? 160 : start === 2100 ? 200 : 0;
    return { top, bottom: top + 20 } as DOMRect;
  });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  await act(async () => vi.mocked(onTranscriptDraft).mock.calls[0][0]({ meeting_id: "old", segments: [
    { start_ms: 1000, end_ms: 1800, text: "First Draft", speaker: "me" },
    { start_ms: 2000, end_ms: 2800, text: "Second Draft", speaker: "me" },
  ] }));
  const scroll = screen.getByRole("region", { name: "Transcript" });
  scroll.scrollTop = 100;
  vi.mocked(meetingTranscript).mockResolvedValue({ ...transcriptWith(2, "First Revision"), segments: [
    { id: "s00001", start_ms: 1000, end_ms: 1800, text: "First Revision", speaker: "me" },
    { id: "s00002", start_ms: 2100, end_ms: 2900, text: "Second Revision", speaker: "me" },
  ] });
  await act(async () => vi.mocked(onLibraryChanged).mock.calls[0][0]());
  expect(await screen.findByText("Second Revision")).toBeTruthy();
  expect(scroll.scrollTop).toBe(140);
  expect(screen.queryByText("Second Draft")).toBeNull();
  expect(screen.getByRole("list", { name: "Segments" }).parentElement?.classList.contains("motion-content-enter")).toBe(true);
});

it("never shows the previous Meeting after switching, even when its data arrives last", async () => {
  const oldDetail = deferred<MeetingDetail>();
  const oldTranscript = deferred<ReturnType<typeof transcriptWith> | null>();
  vi.mocked(meetingDetail).mockImplementation((id) => id === "old" ? oldDetail.promise : Promise.resolve({ ...detail, id: "new", title: "Retro" }));
  vi.mocked(meetingTranscript).mockImplementation((id) => id === "old" ? oldTranscript.promise : Promise.resolve(transcriptWith(1, "New Meeting words")));
  const props = { onBack: () => {}, onOpenSettings: () => {}, onRenamed: () => {} };
  const { rerender } = render(<MeetingView id="old" {...props} />);
  rerender(<MeetingView id="new" {...props} />);
  await screen.findByText("Retro");
  await screen.findByText("New Meeting words");
  await act(async () => { oldDetail.resolve(detail); oldTranscript.resolve(transcriptWith(1, "Old Meeting words")); });
  expect(screen.queryByText("Planning")).toBeNull();
  expect(screen.queryByText("Old Meeting words")).toBeNull();
  expect(screen.getByText("Retro")).toBeTruthy();
});

it("keeps the newest Transcript when overlapping reloads finish in reverse order", async () => {
  const handlers: Array<() => void> = [];
  const libraryChanged = () => handlers.forEach((handler) => handler());
  vi.mocked(onLibraryChanged).mockImplementation((handler) => { handlers.push(handler as () => void); return Promise.resolve(() => {}); });
  const first = deferred<ReturnType<typeof transcriptWith> | null>();
  const second = deferred<ReturnType<typeof transcriptWith> | null>();
  vi.mocked(meetingTranscript).mockResolvedValueOnce(transcriptWith(1, "Initial words")).mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Initial words");
  act(() => { libraryChanged(); libraryChanged(); });
  await act(async () => { second.resolve(transcriptWith(3, "Newest words")); });
  await screen.findByText("Newest words");
  await act(async () => { first.resolve(transcriptWith(2, "Stale words")); });
  expect(screen.queryByText("Stale words")).toBeNull();
  expect(screen.getByText("Newest words")).toBeTruthy();
});

it("shows Loading, not an empty Transcript, while the Transcript is still loading", async () => {
  const pending = deferred<ReturnType<typeof transcriptWith> | null>();
  vi.mocked(meetingTranscript).mockReturnValue(pending.promise);
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Planning");
  expect(screen.queryByText("No Transcript yet")).toBeNull();
  expect(screen.queryByRole("button", { name: "Transcribe" })).toBeNull();
  await act(async () => { pending.resolve(null); });
  expect(await screen.findByText("No Transcript yet")).toBeTruthy();
});

it("shows the processing label once, also after the loading delay", async () => {
  vi.mocked(processingStatus).mockResolvedValue({ jobs: [{ meeting_id: "old", model: null, stage: "queued", progress: 0, error: null }] });
  render(<MeetingView id="old" onBack={() => {}} onOpenSettings={() => {}} onRenamed={() => {}} />);
  await screen.findByText("Waiting to transcribe");
  await act(() => new Promise((resolve) => setTimeout(resolve, 1100)));
  expect(screen.getAllByText("Waiting to transcribe")).toHaveLength(1);
});

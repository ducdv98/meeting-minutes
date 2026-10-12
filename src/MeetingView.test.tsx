import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import MeetingView from "./MeetingView";
import { meetingDetail, renameMeeting, type MeetingDetail } from "./library";
import { convertFileSrc } from "@tauri-apps/api/core";

vi.mock("./library", () => ({ meetingDetail: vi.fn(), renameMeeting: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: vi.fn((path: string) => `asset:${path}`) }));

const detail: MeetingDetail = {
  id: "old", title: "Planning", started_at: "2026-10-12T09:30:00Z",
  duration_ms: 125_000, language: "vi", audio_path: "C:\\Meetings\\meeting.mp3",
  markers: [
    { kind: "pause", at_ms: 10_000, paused_ms: 125_000 },
    { kind: "gap", at_ms: 20_000 },
    { kind: "device_switch", at_ms: 30_000, source: "microphone", device: "Headset" },
  ],
};

beforeEach(() => {
  vi.mocked(meetingDetail).mockResolvedValue(detail);
  vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
  vi.spyOn(HTMLMediaElement.prototype, "load").mockImplementation(() => {});
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.resetAllMocks(); });

it("shows the Meeting fields and Timeline markers", async () => {
  const { container } = render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByRole("heading", { name: "Planning" })).toBeTruthy();
  expect(screen.getByText("2026-10-12 09:30").getAttribute("datetime")).toBe(detail.started_at);
  expect(screen.getByText("Duration: 00:02:05")).toBeTruthy();
  expect(screen.getByText("Language: Vietnamese")).toBeTruthy();
  expect(convertFileSrc).toHaveBeenCalledWith(detail.audio_path);
  expect(container.querySelector("audio")?.getAttribute("src")).toBe(`asset:${detail.audio_path}`);
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).max).toBe("125000");
  expect(screen.getByRole("button", { name: "Paused for 2m 05s at 00:00:10" }).getAttribute("data-kind")).toBe("pause");
  expect(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Microphone switched to Headset at 00:00:30" }).getAttribute("style")).toContain("24%");
  fireEvent.click(screen.getByRole("button", { name: "Gap between Recordings at 00:00:20" }));
  expect((screen.getByRole("slider", { name: "Position" }) as HTMLInputElement).value).toBe("20000");
});

it("renames the Meeting after releasing audio and follows the new id", async () => {
  const renamed: MeetingDetail = { ...detail, id: "new", title: "Review", audio_path: null, markers: [] };
  vi.mocked(meetingDetail).mockImplementation(async (id) => id === "new" ? renamed : detail);
  const onRenamed = vi.fn();
  vi.mocked(renameMeeting).mockImplementation(async () => {
    expect(document.querySelector("audio")).toBeNull();
    return { id: "new", title: "Review", started_at: detail.started_at, duration_ms: detail.duration_ms };
  });
  function Host() {
    const [id, setId] = useState("old");
    return <MeetingView id={id} onBack={() => {}} onRenamed={(next) => { onRenamed(next); setId(next); }} />;
  }
  render(<Host />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: " Review " } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("heading", { name: "Review" })).toBeTruthy();
  expect(renameMeeting).toHaveBeenCalledWith("old", "Review");
  expect(onRenamed).toHaveBeenCalledWith("new");
  expect(meetingDetail).toHaveBeenCalledWith("new");
});

it("keeps the rename input open and shows an error on failure", async () => {
  vi.mocked(renameMeeting).mockRejectedValue("Folder is busy");
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Folder is busy");
  expect(screen.getByRole("textbox", { name: "Title" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
});

it("shows no audio, disables empty saves, and handles Escape", async () => {
  vi.mocked(meetingDetail).mockResolvedValue({ ...detail, language: null, audio_path: null });
  render(<MeetingView id="old" onBack={() => {}} onRenamed={() => {}} />);
  expect(await screen.findByText("No audio yet")).toBeTruthy();
  expect(screen.getByText("Language: Not set")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  const title = screen.getByRole("textbox", { name: "Title" });
  fireEvent.change(title, { target: { value: "   " } });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.keyDown(title, { key: "Escape" });
  await waitFor(() => expect(screen.getByRole("heading", { name: "Planning" })).toBeTruthy());
});

it("follows the new id even when reloading the renamed Meeting fails", async () => {
  vi.mocked(meetingDetail).mockImplementation(async (id) => {
    if (id === "new") throw "Could not read meeting.json";
    return detail;
  });
  vi.mocked(renameMeeting).mockResolvedValue({ id: "new", title: "Review", started_at: detail.started_at, duration_ms: detail.duration_ms });
  const onRenamed = vi.fn();
  function Host() {
    const [id, setId] = useState("old");
    return <MeetingView id={id} onBack={() => {}} onRenamed={(next) => { onRenamed(next); setId(next); }} />;
  }
  render(<Host />);
  await screen.findByRole("heading", { name: "Planning" });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Review" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Could not read meeting.json");
  expect(onRenamed).toHaveBeenCalledWith("new");
  expect(screen.queryByRole("textbox", { name: "Title" })).toBeNull();
});

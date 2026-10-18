import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import TranscriptView from "./TranscriptView";
import type { Transcript } from "./processing";
import type { TimelinePlayer } from "./timeline";

const transcript: Transcript = {
  schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: ["small"],
  language: "en", language_detected: false, duration_ms: 3000,
  segments: [
    { id: "s00001", start_ms: 100, end_ms: 900, text: "Hello", speaker: "me" },
    { id: "s00002", start_ms: 1000, end_ms: 2000, text: "World", speaker: "remote" },
  ],
};
const player: TimelinePlayer = { seek: vi.fn(), play: vi.fn(), pause: vi.fn(), playSpan: vi.fn() };
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("plays the chosen Segment and highlights the current one", () => {
  render(<TranscriptView transcript={transcript} positionMs={1500} player={player} editable onEdit={vi.fn()} />);
  expect(screen.getByRole("list", { name: "Segments" }).children).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "Play Segment at 00:01, Remote" }));
  expect(player.playSpan).toHaveBeenCalledWith(1000, 2000);
  expect(screen.getByText("World").closest("li")?.getAttribute("aria-current")).toBe("true");
  expect(screen.getByText("World").closest("li")?.className).toContain("current");
});

it("shows a Snapshot between the surrounding Segments and opens it", () => {
  const onOpenSnapshot = vi.fn();
  render(<TranscriptView transcript={transcript} positionMs={0} player={null} editable onEdit={vi.fn()} snapshots={[{ order: 1, timeline_ms: 950, caption: "Slide one", image_url: "data:image/png;base64,AAAA" }]} onOpenSnapshot={onOpenSnapshot} />);
  const rows = within(screen.getByRole("list", { name: "Segments" })).getAllByRole("listitem");
  expect(rows.map((row) => row.textContent)).toEqual([expect.stringContaining("Hello"), expect.stringContaining("Slide one"), expect.stringContaining("World")]);
  const thumbnail = screen.getByRole("img", { name: "Slide one" });
  expect(thumbnail.getAttribute("src")).toBe("data:image/png;base64,AAAA");
  fireEvent.click(screen.getByRole("button", { name: "Open Snapshot: Slide one" }));
  expect(onOpenSnapshot).toHaveBeenCalledWith(1);
});

it("shows Snapshots taken during one pause in capture order at the pause position", () => {
  render(<TranscriptView transcript={transcript} positionMs={0} player={null} editable onEdit={vi.fn()} snapshots={[
    { order: 4, timeline_ms: 950, caption: "Second slide", image_url: "data:image/png;base64,BBBB" },
    { order: 3, timeline_ms: 950, caption: "First slide", image_url: "data:image/png;base64,AAAA" },
  ]} />);
  const rows = within(screen.getByRole("list", { name: "Segments" })).getAllByRole("listitem");
  expect(rows.map((row) => row.textContent)).toEqual([
    expect.stringContaining("Hello"), expect.stringContaining("First slide"),
    expect.stringContaining("Second slide"), expect.stringContaining("World"),
  ]);
});

it("shows Participant names, keeps the detected Speaker label, and clears named Unknown styling", () => {
  const named = { ...transcript, segments: [...transcript.segments, { id: "s00003", start_ms: 2100, end_ms: 2900, text: "Maybe", speaker: "unknown" as const }] };
  const { rerender } = render(<TranscriptView transcript={named} positionMs={0} player={null} editable onEdit={vi.fn()} participants={{ remote: "Anna", unknown: "Anna" }} yourName="Duc" />);
  expect(screen.getByText("Duc").getAttribute("title")).toBe("Speaker label: Me");
  expect(screen.getAllByText("Anna")).toHaveLength(2);
  expect(screen.getByText("Maybe").className).not.toContain("text-ink-muted");
  rerender(<TranscriptView transcript={named} positionMs={0} player={null} editable onEdit={vi.fn()} participants={{}} yourName="Duc" isVoiceNote />);
  expect(screen.getAllByText("Duc")).toHaveLength(3);
  expect(screen.getByText("Maybe").className).not.toContain("text-ink-muted");
});

it("saves edited text and keeps the editor open on error", async () => {
  const onEdit = vi.fn().mockRejectedValueOnce("Save failed").mockResolvedValueOnce(undefined);
  render(<TranscriptView transcript={transcript} positionMs={0} player={player} editable onEdit={onEdit} />);
  fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
  const input = screen.getByRole("textbox", { name: "Segment text" });
  fireEvent.change(input, { target: { value: "Updated" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Save failed");
  expect(screen.getByRole("textbox", { name: "Segment text" })).toBeTruthy();
  fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
  await waitFor(() => expect(onEdit).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.queryByRole("textbox", { name: "Segment text" })).toBeNull());
  expect(onEdit).toHaveBeenCalledWith("s00001", "Updated");
});

it("cancels with Escape and disables editing when the Meeting is busy", () => {
  const { rerender } = render(<TranscriptView transcript={transcript} positionMs={0} player={null} editable onEdit={vi.fn()} />);
  fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Segment text" }), { key: "Escape" });
  expect(screen.queryByRole("textbox", { name: "Segment text" })).toBeNull();
  rerender(<TranscriptView transcript={transcript} positionMs={0} player={null} editable={false} onEdit={vi.fn()} />);
  expect((screen.getAllByRole("button", { name: "Edit" })[0] as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Play Segment at 00:00, Me" }) as HTMLButtonElement).disabled).toBe(true);
});

it("stops an open editor from saving when Processing begins", () => {
  const onEdit = vi.fn();
  const { rerender } = render(<TranscriptView transcript={transcript} positionMs={0} player={player} editable onEdit={onEdit} />);
  fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
  rerender(<TranscriptView transcript={transcript} positionMs={0} player={player} editable={false} onEdit={onEdit} />);
  const input = screen.getByRole("textbox", { name: "Segment text" });
  expect((screen.getByRole("button", { name: "Save" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
  expect(onEdit).not.toHaveBeenCalled();
});

it("moves selected Segments together and clears the selection", async () => {
  const onMove = vi.fn().mockResolvedValue(undefined);
  render(<TranscriptView transcript={transcript} positionMs={0} player={null} editable onEdit={vi.fn()} onMove={onMove} />);
  fireEvent.click(screen.getByRole("checkbox", { name: "Select Segment s00001" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Select Segment s00002" }));
  expect(screen.getByText("2 selected")).toBeTruthy();
  fireEvent.change(screen.getByRole("combobox", { name: "Move to Participant" }), { target: { value: "Remote" } });
  await waitFor(() => expect(onMove).toHaveBeenCalledWith(["s00001", "s00002"], "Remote"));
  expect(screen.queryByText("2 selected")).toBeNull();
});

it("moves one Segment to a new Participant entered inline", async () => {
  const onMove = vi.fn().mockResolvedValue(undefined);
  render(<TranscriptView transcript={transcript} positionMs={0} player={null} editable onEdit={vi.fn()} onMove={onMove} />);
  fireEvent.click(screen.getByRole("checkbox", { name: "Select Segment s00002" }));
  fireEvent.change(screen.getByRole("combobox", { name: "Move to Participant" }), { target: { value: "__new__" } });
  fireEvent.change(screen.getByRole("textbox", { name: "New Participant name" }), { target: { value: " Bao " } });
  fireEvent.click(screen.getByRole("button", { name: "Move" }));
  await waitFor(() => expect(onMove).toHaveBeenCalledWith(["s00002"], "Bao"));
});

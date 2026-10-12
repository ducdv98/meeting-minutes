import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import TranscriptView from "./TranscriptView";
import type { Transcript } from "./processing";
import type { TimelinePlayer } from "./player";

const transcript: Transcript = {
  schema_version: 1, revision: 1, created_at: "2026-10-12T10:00:00Z", model: "small",
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
  fireEvent.click(screen.getByRole("button", { name: "[00:00:01] Remote" }));
  expect(player.playSpan).toHaveBeenCalledWith(1000, 2000);
  expect(screen.getByText("World").closest("li")?.getAttribute("aria-current")).toBe("true");
  expect(screen.getByText("World").closest("li")?.className).toContain("current");
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
  expect((screen.getByRole("button", { name: "[00:00:00] Me" }) as HTMLButtonElement).disabled).toBe(true);
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

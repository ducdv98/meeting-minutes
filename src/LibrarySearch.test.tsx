import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import LibrarySearch from "./LibrarySearch";
import { searchLibrary, snippetParts } from "./library";
import { onLibraryChanged } from "./recording";

vi.mock("./library", async (original) => ({ ...(await original<typeof import("./library")>()), searchLibrary: vi.fn() }));
vi.mock("./recording", async (original) => ({ ...(await original<typeof import("./recording")>()), onLibraryChanged: vi.fn(() => Promise.resolve(() => {})) }));
afterEach(() => { cleanup(); vi.useRealTimers(); vi.resetAllMocks(); });

it("splits FTS snippets into safe text and matches", () => {
  expect(snippetParts("A \u0002budget\u0003 for \u0002người\u0003")).toEqual([
    { text: "A ", match: false }, { text: "budget", match: true },
    { text: " for ", match: false }, { text: "người", match: true },
  ]);
});

it("debounces search, renders matches, and opens a Segment", async () => {
  vi.useFakeTimers();
  const hit = { meeting_id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z", segment_id: "s00001", revision: 1, start_ms: 1000, end_ms: 2000, snippet: "The \u0002budget\u0003" };
  vi.mocked(searchLibrary).mockResolvedValue([hit]);
  const onOpen = vi.fn();
  render(<LibrarySearch onOpen={onOpen} />);
  fireEvent.change(screen.getByRole("searchbox", { name: "Search transcripts" }), { target: { value: "budg" } });
  expect(searchLibrary).not.toHaveBeenCalled();
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(searchLibrary).toHaveBeenCalledWith("budg");
  expect(screen.getByRole("mark").textContent).toBe("budget");
  fireEvent.click(screen.getByRole("button", { name: /Planning/ }));
  expect(onOpen).toHaveBeenCalledWith(hit);
});

it("refreshes a non-empty query when the Library changes", async () => {
  vi.useFakeTimers();
  vi.mocked(searchLibrary).mockResolvedValueOnce([]).mockResolvedValueOnce([]);
  render(<LibrarySearch onOpen={() => {}} />);
  fireEvent.change(screen.getByRole("searchbox", { name: "Search transcripts" }), { target: { value: "budget" } });
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(searchLibrary).toHaveBeenCalledTimes(1);
  await act(async () => { vi.mocked(onLibraryChanged).mock.lastCall![0](); });
  expect(searchLibrary).toHaveBeenCalledTimes(2);
  expect(searchLibrary).toHaveBeenLastCalledWith("budget");
});

it("shows No matches and an error for a failed search", async () => {
  vi.useFakeTimers();
  vi.mocked(searchLibrary).mockResolvedValueOnce([]).mockRejectedValueOnce("Index unavailable");
  render(<LibrarySearch onOpen={() => {}} />);
  const input = screen.getByRole("searchbox", { name: "Search transcripts" });
  fireEvent.change(input, { target: { value: "absent" } });
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(screen.getByText("No results")).toBeTruthy();
  fireEvent.change(input, { target: { value: "broken" } });
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(screen.getByRole("alert").textContent).toContain("Index unavailable");
});

it("keeps searching on Enter and opens Library Ask with the box text", async () => {
  vi.mocked(searchLibrary).mockResolvedValue([]);
  const onAsk = vi.fn();
  render(<LibrarySearch onOpen={() => {}} onAsk={onAsk} />);
  const input = screen.getByRole("searchbox", { name: "Search transcripts" });
  const button = screen.getByRole("button", { name: "Ask" });
  expect(button.hasAttribute("disabled")).toBe(true);
  fireEvent.change(input, { target: { value: "What was decided?" } });
  expect(button.hasAttribute("disabled")).toBe(false);
  fireEvent.keyDown(input, { key: "Enter" });
  expect(searchLibrary).toHaveBeenCalledWith("What was decided?");
  expect(onAsk).not.toHaveBeenCalled();
  fireEvent.click(button);
  expect(onAsk).toHaveBeenCalledWith("What was decided?");
});

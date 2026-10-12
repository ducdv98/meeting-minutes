import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import App from "./App";
import { listMeetings } from "./library";

vi.mock("./library", () => ({ listMeetings: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

it("shows an empty Library when there are no Meetings", async () => {
  vi.mocked(listMeetings).mockResolvedValue([]);

  render(<App />);

  expect(screen.getByRole("heading", { name: "Library" })).toBeTruthy();
  expect(await screen.findByText("No meetings yet")).toBeTruthy();
});

it("lists Meeting titles in the Library", async () => {
  vi.mocked(listMeetings).mockResolvedValue([
    { id: "one", title: "Planning", started_at: "2026-10-12T09:00:00Z" },
    { id: "two", title: "Review", started_at: "2026-10-12T10:00:00Z" },
  ]);

  render(<App />);

  expect(await screen.findByText("Planning")).toBeTruthy();
  expect(screen.getByText("Review")).toBeTruthy();
});

it("shows an error when the Library cannot be loaded", async () => {
  vi.mocked(listMeetings).mockRejectedValue(new Error("boom"));

  render(<App />);

  expect(await screen.findByRole("alert")).toBeTruthy();
});

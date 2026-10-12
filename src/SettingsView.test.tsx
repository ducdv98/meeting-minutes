import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import SettingsView from "./SettingsView";
import { getSettings, updateSettings } from "./processing";

vi.mock("./processing", () => ({ getSettings: vi.fn(), updateSettings: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("loads and saves transcription settings", async () => {
  vi.mocked(getSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: ["Nguyễn"] });
  vi.mocked(updateSettings).mockResolvedValue({ recording_notice_acknowledged: true, auto_process: false, model: "medium", vocabulary: ["Alice", "Project X"] });
  render(<SettingsView onBack={() => {}} />);
  const auto = await screen.findByRole("checkbox", { name: "Transcribe automatically after recording" });
  expect((auto as HTMLInputElement).checked).toBe(true);
  fireEvent.click(auto);
  fireEvent.change(screen.getByRole("combobox", { name: "Transcription model" }), { target: { value: "medium" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Vocabulary" }), { target: { value: " Alice \n\nProject X" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(updateSettings).toHaveBeenCalledWith(false, "medium", ["Alice", "Project X"]));
});

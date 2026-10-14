import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { SpeechToTextTab } from "./SpeechToTextTab";
import { managedConnect, managedStatus } from "../ai";
import { MANAGED_PRIVACY_STATEMENT, MANAGED_PRIVACY_SUMMARY } from "../privacy-text";
import type { Settings } from "../processing";

vi.mock("../ai", async (importOriginal) => ({ ...(await importOriginal<typeof import("../ai")>()), managedStatus: vi.fn(), managedConnect: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });

const settings: Settings = { recording_notice_acknowledged: true, cloud_notice_acknowledged: true, auto_process: true, model: "small", vocabulary: [], language: null, your_name: null, ai: { provider: null, ollama_model: null, openai_model: null, anthropic_model: null, gemini_model: null } };

function renderTab() {
  return render(<SpeechToTextTab settings={settings} setSettings={vi.fn()} save={vi.fn()} installed={[]} vocabulary="" setVocabulary={vi.fn()} saveVocabulary={vi.fn()} providerStatus={null} setProviderStatus={vi.fn()} />);
}

it("shows the privacy statement before Connect, then connects after the invite link opens", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: false });
  vi.mocked(managedConnect).mockResolvedValue({ connected: true, revoked: false });
  renderTab();
  const section = screen.getByRole("region", { name: "Managed transcription (experimental)" });
  await waitFor(() => expect(managedStatus).toHaveBeenCalledOnce());
  expect(within(section).queryByLabelText("Invite code")).toBeNull();
  expect(within(section).getByText(MANAGED_PRIVACY_SUMMARY)).toBeTruthy();
  const details = within(section).getByRole("button", { name: "Details" });
  fireEvent.click(details);
  expect(details.getAttribute("aria-expanded")).toBe("true");
  expect(within(section).getByText(MANAGED_PRIVACY_STATEMENT)).toBeTruthy();
  fireEvent.click(within(section).getByRole("button", { name: "Have an invite code?" }));
  fireEvent.change(within(section).getByLabelText("Invite code"), { target: { value: " code " } });
  fireEvent.click(within(section).getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(managedConnect).toHaveBeenCalledWith(" code "));
  expect(await within(section).findByText("Connected")).toBeTruthy();
});

it("opens the Invite code field when Managed transcription was revoked", async () => {
  vi.mocked(managedStatus).mockResolvedValue({ connected: false, revoked: true });
  renderTab();
  const section = screen.getByRole("region", { name: "Managed transcription (experimental)" });
  expect(await within(section).findByText("Managed transcription was disconnected.")).toBeTruthy();
  expect(within(section).getByLabelText("Invite code")).toBeTruthy();
});

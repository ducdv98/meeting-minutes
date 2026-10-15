import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import HelpView from "./HelpView";
import { appInfo, openLink } from "./app-info";
import { getDiagnostics } from "./diagnostics";

vi.mock("./app-info", () => ({ appInfo: vi.fn(), openLink: vi.fn() }));
vi.mock("./diagnostics", () => ({ getDiagnostics: vi.fn() }));

const writeText = vi.fn();
beforeEach(() => {
  vi.mocked(appInfo).mockResolvedValue({ version: "0.1.7" });
  vi.mocked(getDiagnostics).mockResolvedValue("Meeting Minutes 0.1.7\n--- recent log ---");
  vi.mocked(openLink).mockResolvedValue();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });

it("shows the app version without a release badge", async () => {
  render(<HelpView />);
  const about = screen.getByRole("region", { name: "About" });
  expect(await within(about).findByText("0.1.7")).toBeTruthy();
  expect(within(about).queryByText("Beta")).toBeNull();
  expect(within(about).getByText("Duc Dao")).toBeTruthy();
});

it("copies fresh diagnostics before opening a prefilled GitHub issue", async () => {
  render(<HelpView />);
  await screen.findByLabelText("Diagnostics preview");
  vi.mocked(getDiagnostics).mockResolvedValue("fresh diagnostics");
  fireEvent.click(screen.getByRole("button", { name: "Report a problem" }));
  await waitFor(() => expect(openLink).toHaveBeenCalledWith("report_problem"));
  expect(writeText).toHaveBeenCalledWith("fresh diagnostics");
  expect(writeText.mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(openLink).mock.invocationCallOrder[0]);
  expect((await screen.findByRole("status")).textContent).toContain("Paste them into the GitHub issue");
});

it("copies the diagnostics text and confirms", async () => {
  render(<HelpView />);
  expect((await screen.findByLabelText("Diagnostics preview")).textContent).toContain("recent log");
  vi.mocked(getDiagnostics).mockResolvedValue("fresh diagnostics");
  fireEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith("fresh diagnostics"));
  expect((await screen.findByRole("status")).textContent).toContain("Diagnostics copied");
  expect(screen.getByRole("button", { name: "Copied" })).toBeTruthy();
});

it("says so when the clipboard refuses", async () => {
  writeText.mockRejectedValue(new Error("denied"));
  render(<HelpView />);
  fireEvent.click(screen.getByRole("button", { name: "Copy diagnostics" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Couldn't copy diagnostics");
});

it("opens the support address and privacy page", async () => {
  render(<HelpView />);
  fireEvent.click(screen.getByRole("button", { name: "admin@meeting-minutes.tech" }));
  fireEvent.click(screen.getByRole("button", { name: "meeting-minutes.tech/privacy" }));
  await waitFor(() => expect(openLink).toHaveBeenCalledTimes(2));
  expect(vi.mocked(openLink).mock.calls).toEqual([["support"], ["privacy"]]);
});

it("lists the real keyboard shortcuts", () => {
  render(<HelpView />);
  const shortcuts = screen.getByRole("region", { name: "Keyboard shortcuts" }).textContent;
  expect(shortcuts).toContain("CtrlAltShiftR");
  expect(shortcuts).toContain("CtrlAltShiftP");
  expect(shortcuts).toContain("CtrlK");
});

it("offers the setup guide only when it can open it", () => {
  const openGuide = vi.fn();
  const { rerender } = render(<HelpView />);
  expect(screen.queryByRole("button", { name: "Open the setup guide" })).toBeNull();
  rerender(<HelpView onOpenGuide={openGuide} />);
  fireEvent.click(screen.getByRole("button", { name: "Open the setup guide" }));
  expect(openGuide).toHaveBeenCalledOnce();
});

it("retries reading diagnostics after a failure", async () => {
  vi.mocked(getDiagnostics).mockRejectedValueOnce("log locked").mockResolvedValue("Meeting Minutes 0.1.7");
  render(<HelpView />);
  expect((await screen.findByRole("alert")).textContent).toContain("log locked");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect((await screen.findByLabelText("Diagnostics preview")).textContent).toContain("0.1.7");
});

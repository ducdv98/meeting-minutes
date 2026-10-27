import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import LibraryAskConfirmation from "./LibraryAskConfirmation";

afterEach(cleanup);

it("lists evidence, keeps every Meeting ticked, and sends only selected Meetings", () => {
  const onSend = vi.fn();
  render(<LibraryAskConfirmation provider="openai" meetings={[{ meeting_id: "m1", title: "Planning", started_at: "2026-10-12T09:00:00Z", excerpts: 2 }, { meeting_id: "m2", title: "Review", started_at: "2026-10-13T09:00:00Z", excerpts: 3 }]} onSend={onSend} onCancel={() => {}} />);
  expect(screen.getByRole("dialog", { name: "Send to OpenAI?" }).textContent).toContain("Your question was sent to OpenAI to find relevant Meetings. Excerpts from the ticked Meetings will be sent next.");
  fireEvent.click(screen.getByRole("checkbox", { name: /Review/ }));
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(onSend).toHaveBeenCalledWith(["m2"]);
});

it("disables Send with no ticked Meetings and Cancel sends nothing", () => {
  const onSend = vi.fn();
  const onCancel = vi.fn();
  render(<LibraryAskConfirmation provider="openai" meetings={[{ meeting_id: "m1", title: "Planning", started_at: "2026-10-12T09:00:00Z", excerpts: 1 }]} onSend={onSend} onCancel={onCancel} />);
  fireEvent.click(screen.getByRole("checkbox", { name: /Planning/ }));
  expect(screen.getByRole("button", { name: "Send" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onCancel).toHaveBeenCalled();
  expect(onSend).not.toHaveBeenCalled();
});

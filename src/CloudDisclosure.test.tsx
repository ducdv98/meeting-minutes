import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import CloudDisclosure from "./CloudDisclosure";

afterEach(cleanup);
it("shows exactly what will be sent and provides Send and Cancel", () => {
  const onSend = vi.fn(); const onCancel = vi.fn();
  render(<CloudDisclosure disclosure={{ provider: "openai", model: "gpt-4o", endpoint: "https://api.openai.com/v1/chat/completions", system: "System instructions", messages: [{ role: "user", content: "Meeting text" }], characters: 31 }} onSend={onSend} onCancel={onCancel} />);
  expect(screen.getByRole("dialog", { name: "Send to OpenAI?" })).toBeTruthy();
  expect(screen.getByText("31 characters")).toBeTruthy();
  expect(screen.getByText("System instructions")).toBeTruthy();
  expect(screen.getByText("Meeting text")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onSend).toHaveBeenCalledOnce(); expect(onCancel).toHaveBeenCalledOnce();
});

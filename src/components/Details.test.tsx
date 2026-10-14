import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { Details } from "./Details";

afterEach(cleanup);

it("reveals details with a keyboard-operable button and announces its state", () => {
  render(<Details><a href="#privacy">Full privacy statement</a></Details>);
  const button = screen.getByRole("button", { name: "Details" });
  const panel = document.getElementById(button.getAttribute("aria-controls")!);
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(panel?.getAttribute("aria-hidden")).toBe("true");
  expect(panel?.hasAttribute("inert")).toBe(true);
  button.focus();
  fireEvent.keyDown(button, { key: "Enter" });
  fireEvent.click(button);
  expect(button.getAttribute("aria-expanded")).toBe("true");
  expect(panel?.getAttribute("aria-hidden")).toBe("false");
  expect(panel?.hasAttribute("inert")).toBe(false);
  expect(screen.getByRole("link", { name: "Full privacy statement" })).toBeTruthy();
  fireEvent.keyDown(button, { key: " " });
  fireEvent.keyUp(button, { key: " " });
  fireEvent.click(button);
  expect(button.getAttribute("aria-expanded")).toBe("false");
  expect(panel?.getAttribute("data-expanded")).toBe("false");
});

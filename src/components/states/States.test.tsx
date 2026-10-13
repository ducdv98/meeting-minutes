import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Loading } from "./Loading";
import { EmptyState } from "./EmptyState";
import { ErrorState } from "./ErrorState";


afterEach(() => { cleanup(); vi.useRealTimers(); });

it("shows loading skeletons, then explanatory text after one second", () => {
  vi.useFakeTimers();
  const { container } = render(<Loading message="Reading Library…" rows={2} />);
  expect(container.querySelectorAll('[aria-hidden="true"]')).toHaveLength(2);
  expect(screen.queryByText("Reading Library…")).toBeNull();
  act(() => vi.advanceTimersByTime(1000));
  expect(screen.getByText("Reading Library…")).toBeTruthy();
});

it("shows an empty state with its optional action", () => {
  render(<EmptyState title="No meetings yet" description="Your meetings stay on this PC." action={<button>Record</button>} />);
  expect(screen.getByRole("heading", { name: "No meetings yet" })).toBeTruthy();
  expect(screen.getByText("Your meetings stay on this PC.")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Record" })).toBeTruthy();
});

it("explains an error and retries when possible", () => {
  const retry = vi.fn();
  render(<ErrorState title="Couldn't open the Library" guidance="Check the folder and try again." onRetry={retry} />);
  expect(screen.getByRole("alert").textContent).toContain("Check the folder");
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(retry).toHaveBeenCalledOnce();
});

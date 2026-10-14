import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { Button } from "./Button";
import { Switch } from "./Switch";
import { Citation } from "./Citation";
import { RadioGroup, RadioGroupItem } from "./RadioGroup";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./Tabs";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./Dialog";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "./Tooltip";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "./DropdownMenu";

afterEach(cleanup);

function renderWithTooltips(children: ReactNode) {
  return render(<TooltipProvider delayDuration={0}>{children}</TooltipProvider>);
}

it("renders button variants and blocks a disabled action", () => {
  render(<>
    <Button variant="primary">Primary</Button>
    <Button variant="secondary">Secondary</Button>
    <Button variant="ghost">Ghost</Button>
    <Button variant="accent">Accent</Button>
    <Button variant="record">Record</Button>
  </>);
  for (const name of ["Primary", "Secondary", "Ghost", "Accent", "Record"]) {
    expect(screen.getByRole("button", { name })).toBeTruthy();
  }
  const click = vi.fn();
  render(<Button disabled onClick={click}>Save</Button>);
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(click).not.toHaveBeenCalled();
});

it("toggles the switch with the keyboard", () => {
  render(<Switch aria-label="Auto process" />);
  const control = screen.getByRole("switch", { name: "Auto process" });
  expect(control.getAttribute("aria-checked")).toBe("false");
  control.focus();
  fireEvent.keyDown(control, { key: " " });
  fireEvent.keyUp(control, { key: " " });
  // jsdom does not synthesize a native button click after Space.
  fireEvent.click(control);
  expect(control.getAttribute("aria-checked")).toBe("true");
});

it("moves radio selection with arrow keys and separates labels from descriptions", async () => {
  render(<RadioGroup defaultValue="local" aria-label="Processing">
    <RadioGroupItem value="local" description="Stays on this PC" sideNote="Recommended">Local</RadioGroupItem>
    <RadioGroupItem value="cloud" description="Sent to a provider">Cloud</RadioGroupItem>
  </RadioGroup>);
  const local = screen.getByRole("radio", { name: "Local" });
  const cloud = screen.getByRole("radio", { name: "Cloud" });
  const descriptionIds = local.getAttribute("aria-describedby")?.split(" ") ?? [];
  expect(descriptionIds.map((id) => document.getElementById(id)?.textContent)).toEqual(["Stays on this PC", "Recommended"]);
  expect(local.getAttribute("aria-checked")).toBe("true");
  local.focus();
  fireEvent.keyDown(local, { key: "ArrowDown" });
  await waitFor(() => expect(cloud.getAttribute("aria-checked")).toBe("true"));
});

it("switches tab panels", () => {
  render(<Tabs defaultValue="transcript">
    <TabsList><TabsTrigger value="transcript">Transcript</TabsTrigger><TabsTrigger value="summary">Summary</TabsTrigger></TabsList>
    <TabsContent value="transcript">Transcript content</TabsContent>
    <TabsContent value="summary">Summary content</TabsContent>
  </Tabs>);
  expect(screen.getByText("Transcript content")).toBeTruthy();
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Summary" }), { button: 0, ctrlKey: false });
  expect(screen.getByText("Summary content")).toBeTruthy();
  expect(screen.queryByText("Transcript content")).toBeNull();
});

it("gives a dialog an accessible title", () => {
  render(<Dialog><DialogTrigger>Open</DialogTrigger><DialogContent><DialogTitle>Delete meeting?</DialogTitle></DialogContent></Dialog>);
  fireEvent.click(screen.getByRole("button", { name: "Open" }));
  expect(screen.getByRole("dialog", { name: "Delete meeting?" }).classList.contains("motion-dialog-content")).toBe(true);
  expect(document.querySelector(".motion-dialog-overlay")?.getAttribute("data-state")).toBe("open");
});

it("opens a dropdown with motion from its trigger side", async () => {
  render(<DropdownMenu><DropdownMenuTrigger>More</DropdownMenuTrigger><DropdownMenuContent side="bottom"><DropdownMenuItem>Rename</DropdownMenuItem></DropdownMenuContent></DropdownMenu>);
  fireEvent.pointerDown(screen.getByRole("button", { name: "More" }), { button: 0, ctrlKey: false });
  const menu = await screen.findByRole("menu");
  expect(menu.classList.contains("motion-popover")).toBe(true);
  expect(menu.getAttribute("data-side")).toBe("bottom");
  expect(menu.getAttribute("data-state")).toBe("open");
});

it("shows tooltip content when its trigger receives focus", async () => {
  renderWithTooltips(<Tooltip><TooltipTrigger>Help</TooltipTrigger><TooltipContent>Saved on this PC</TooltipContent></Tooltip>);
  fireEvent.focus(screen.getByRole("button", { name: "Help" }));
  await waitFor(() => expect(screen.getByRole("tooltip").textContent).toContain("Saved on this PC"));
  expect(screen.getByRole("tooltip").classList.contains("motion-popover")).toBe(true);
});

it("plays a citation when clicked", () => {
  const click = vi.fn();
  render(<Citation time="12:34" segmentText="Decision recorded" onClick={click} />);
  fireEvent.click(screen.getByRole("button", { name: /12:34/ }));
  expect(click).toHaveBeenCalledOnce();
});

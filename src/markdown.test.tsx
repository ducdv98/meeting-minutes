import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import Markdown from "./markdown";

afterEach(cleanup);

it("renders paragraphs, line breaks, emphasis, lists, headings, and inline code", () => {
  render(<Markdown text={"First line\nSecond *line* with `code`.\n\n# Next steps\n1. **Find information…** 2. **Clarify…**\n\n- One\n- Two"} />);
  expect(screen.getByText(/First line/).querySelector("br")).toBeTruthy();
  expect(screen.getByText("line").tagName).toBe("EM");
  expect(screen.getByText("code").tagName).toBe("CODE");
  expect(screen.getByText("Next steps").tagName).toBe("STRONG");
  const steps = screen.getAllByRole("list")[0];
  expect(steps.tagName).toBe("OL");
  expect(within(steps).getAllByRole("listitem")).toHaveLength(2);
  expect(screen.getByText("Find information…").tagName).toBe("STRONG");
  expect(screen.getByText("Clarify…").tagName).toBe("STRONG");
  expect(screen.getAllByRole("list")[1].tagName).toBe("UL");
});

it("shows unsupported links, images, tables, code blocks, and HTML as text", () => {
  const { container } = render(<Markdown text={"<script>**alert(1)**</script> [x](javascript:alert) ![img](photo.png)\n\n| A | B |\n|---|---|\n\n```html\n<strong>literal</strong>\n```"} />);
  expect(container.textContent).toContain("<script>**alert(1)**</script>");
  expect(container.textContent).toContain("[x](javascript:alert)");
  expect(container.textContent).toContain("![img](photo.png)");
  expect(container.textContent).toContain("| A | B |");
  expect(container.textContent).toContain("```html");
  expect(container.querySelector("script, a, img, table, pre, strong")).toBeNull();
});

it("keeps incomplete emphasis literal until a stream closes it", () => {
  const { rerender } = render(<Markdown text="The **decision" />);
  expect(screen.getByText("The **decision")).toBeTruthy();
  rerender(<Markdown text="The **decision**" />);
  expect(screen.getByText("decision").tagName).toBe("STRONG");
  expect(screen.queryByText(/\*\*/)).toBeNull();
});

it("places trailing Citations after the last block", () => {
  const { container } = render(<Markdown text={"First paragraph\n\nSecond paragraph"} trailing={<button type="button">Citation</button>} />);
  const paragraphs = container.querySelectorAll("p");
  expect(paragraphs).toHaveLength(2);
  expect(paragraphs[0].querySelector("button")).toBeNull();
  expect(paragraphs[1].querySelector("button")?.textContent).toBe("Citation");
});

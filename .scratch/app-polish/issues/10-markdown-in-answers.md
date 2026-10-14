# Markdown in Answers and Summary items

Status: ready-for-agent
Spec: ../spec.md

## What

Models write Markdown inside the Answer string (e.g. `1. **Find information…** 2. **Clarify…**`), and `AskPanel` shows it raw in a single `<p>`, which also joins the numbered list into one line.

- Render a safe subset in Answers (final and streaming) and Summary items: paragraphs, line breaks, bold, italic, bulleted and numbered lists, inline code. Headings render as bold paragraphs.
- Links, images, tables, code blocks and raw HTML are shown as plain text. Never use `dangerouslySetInnerHTML`.
- Citation chips stay after the rendered text (after the last block for multi-block Answers).
- A small renderer in `src/` is fine; if a library is used, it must allow restricting to this subset.
- Leave `summary.md` in the Library unchanged.

## Acceptance criteria

- [ ] The sample above renders as a two-item numbered list with bold lead-ins.
- [ ] `<script>`, `[x](javascript:…)` and `![img](…)` render as text (tests).
- [ ] A partial stream with an unclosed `**` renders without errors or flicker into raw markers once closed.
- [ ] Insufficient evidence and Off-topic Answers keep their current styling.

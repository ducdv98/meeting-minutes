# Visual identity

One identity for the Windows app and the website `meeting-minutes.tech`. The app is the product and sets the tone; the website is the same identity at a slightly larger scale, not a louder one. Both build from the tokens in this file. The mark and wordmark come from `assets/brand/` (ADR-0004).

## Character

Calm, precise, quiet. In the spirit of Linear and Notion: lots of whitespace, a clear type hierarchy, ink on paper, one restrained accent. The user's words (the Transcript) are the content; the interface stays out of the way.

- **Paper and ink.** Warm off-white surfaces and charcoal text, taken from the app icon's plate and mark.
- **Layered pages.** The mark is a charcoal page over a grey one. Layers of surface (canvas, panel, raised) carry structure instead of borders and shadows.
- **One accent.** Ink blue marks what the user can act on or follow: the primary action, selection, links and Citations. Never decoration.
- **Red means recording.** Red is reserved for the recording state and destructive actions. Nothing else is red.
- **Honest.** Show the real product and real states. No fake metrics, logos, testimonials or screenshots of features that don't exist.

## Colour

All colours are tokens. Light and dark follow Windows (`prefers-color-scheme`). Contrast ratios are against the surface they sit on; text pairs meet WCAG AA (4.5:1).

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--canvas` | `#FAF9F7` | `#1A1A1B` | Window and page background |
| `--panel` | `#F3F2EF` | `#202022` | Sidebar, secondary panels, website alternate sections |
| `--raised` | `#FFFFFF` | `#26262A` | Cards, popovers, dialogs, inputs |
| `--hover` | `#ECEBE7` | `#2C2C30` | Row and control hover |
| `--line` | `#E4E3DF` | `#2E2E32` | Dividers, panel edges |
| `--line-strong` | `#CFCECA` | `#3E3E43` | Input borders, focused panels |
| `--ink` | `#2A2A2B` | `#EDEDEE` | Primary text, primary button (13.6:1 / 14.9:1) |
| `--ink-muted` | `#6B6B6F` | `#9E9EA3` | Secondary text, metadata (5.0:1 / 6.5:1) |
| `--ink-faint` | `#A3A3A7` | `#6A6A70` | Placeholders, disabled; never body text |
| `--page-shadow` | `#E2E3E6` | `#2A2B2F` | The mark's second page; subtle fills, skeletons |
| `--accent` | `#2F5AD6` | `#7C9DF2` | Links, Citations, selection, focus ring, accent button (5.6:1 / 6.6:1) |
| `--accent-soft` | `#EEF2FC` | `#1E2640` | Selected row, Citation chip, highlighted Segment |
| `--accent-ink` | `#FFFFFF` | `#111318` | Text on an `--accent` fill |
| `--record` | `#D3363A` | `#F26B6B` | Recording indicator, Stop, destructive actions |
| `--record-soft` | `#FBECEC` | `#3A1F21` | Recording bar background, destructive confirm |
| `--ok` | `#1A7F45` | `#4ABF7A` | Done states (Transcript ready, test prompt succeeded) |
| `--warn` | `#A15C00` | `#E0A43C` | Incomplete, Unverified, silent Source, model still downloading |

Rules:
- The primary button is ink on canvas (`--ink` fill, `--canvas` text), not accent. The accent button is used once per screen at most, for the main forward action (Record is the exception: it uses `--record`).
- No gradients, glass, glows or coloured section backgrounds. Alternate website sections use `--panel`.
- Speaker labels are not colour-coded with the accent: **Me** uses `--ink`, **Remote** `--ink-muted`, **Unknown** `--ink-faint` with an icon, so colour is never the only signal.

## Typography

Two families, both open source, both with full Vietnamese support (Meeting language is English or Vietnamese). Fonts are bundled with the app and self-hosted on the website: the app works offline and the website loads nothing from third parties.

- **Be Vietnam Pro** (600, 700) for headings and display. A geometric sans that echoes the wordmark.
- **Inter** (400, 500, 600) for everything else: UI text, Transcript, body copy. Use `font-variant-numeric: tabular-nums` for timestamps, durations and sizes.
- No monospace except diagnostics output.

| Token | Size / line height | Weight | Tracking | Use |
| --- | --- | --- | --- | --- |
| `--text-xs` | 12 / 16 | 500 | 0 | Metadata, badges, timestamps |
| `--text-sm` | 13 / 20 | 400 | 0 | Secondary UI, sidebar rows |
| `--text-base` | 14 / 22 | 400 | 0 | Default app UI text |
| `--text-read` | 15 / 26 | 400 | 0 | Transcript, Summary, Answers (long reading) |
| `--text-lg` | 17 / 26 | 400 | -0.005em | Website body |
| `--heading-sm` | 16 / 24 | 600 | -0.01em | Panel titles |
| `--heading-md` | 20 / 28 | 600 | -0.015em | Meeting title, dialog titles |
| `--heading-lg` | 28 / 36 | 600 | -0.02em | First-run step titles, website section titles |
| `--display` | clamp(40px, 5.5vw, 64px) / 1.05 | 700 | -0.03em | Website hero only |

Sentence case everywhere: headings, buttons, menus. No all-caps except `--text-xs` labels, and no lowercase-only headlines.

## Space, shape and depth

- **Spacing:** 4px base. Scale `4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 96, 128`. App panels use 16–24px padding; website sections use 96–128px vertical space on desktop and 64px on mobile.
- **Radius:** `--radius-sm` 6px (buttons, inputs, chips), `--radius-md` 8px (cards, popovers, menus), `--radius-lg` 12px (dialogs, product screenshots on the website). Nothing pill-shaped except toggles and the Beta badge.
- **Borders:** 1px `--line`. Structure comes first from surface layers, then from dividers; avoid boxing every element.
- **Shadows:** none on resting surfaces. Popovers and menus: `0 4px 16px rgb(0 0 0 / 0.08)`. Dialogs: `0 12px 40px rgb(0 0 0 / 0.14)`. In dark mode use a `--line-strong` edge instead of a stronger shadow.
- **Focus:** `2px solid var(--accent)` outline, 2px offset, on every interactive element. Never removed.

## Motion

Fast and functional: 120ms for hover and press, 180ms for popovers and panels, 240ms for dialogs, easing `cubic-bezier(.2, .8, .2, 1)`. Nothing animates on its own except progress, live audio levels and the recording pulse (a 2s opacity pulse on the red dot). Website sections may fade in once on scroll (opacity and 12px rise, 400ms). `prefers-reduced-motion` turns off all non-essential motion, including the recording pulse.

## Iconography

Lucide icons at 16px (app) or 20px (website), 1.5px stroke, `currentColor`. Icons sit next to a label or carry a tooltip; no decorative icon bubbles.

## Brand marks

- Light theme: `wordmark-dark` / `mark-dark`. Dark theme: `wordmark-white` / `mark-white`. Derived for the build by `scripts/build-brand-assets.py` (ADR-0004).
- Empty states and watermarks use the theme's mark (`mark-dark` or `mark-white`) at about 22% opacity, as in the approved mockup.
- Clear space around the mark is at least half its height. Never recolour, stretch or put the mark on the accent.

## Recurring elements

- **Citation:** an inline chip with the Segment's start time, `--accent` text on `--accent-soft`, `--radius-sm`, tabular numbers (`12:34`). Hover shows the Segment text; click plays the span. It looks the same in Summary, Action items and Answers.
- **Recording indicator:** a red dot plus elapsed time, always visible while a Recording runs, in the window and the tray. The recording bar uses `--record-soft`.
- **Beta badge:** `--text-xs`, `--ink-muted` text, `--line-strong` 1px border, pill radius. Next to the wordmark in About and on the website header.
- **States:** loading uses `--page-shadow` skeletons (no spinners longer than 1s without text); empty states use the theme's mark at low opacity, one sentence and one action; errors say what happened and what to do, with Retry where possible.

## Voice

Short, literal, calm. Say what the app does, in the glossary's words (Meeting, Transcript, Summary, Ask, Citation; see `CONTEXT.md`).

- Yes: "Your meetings stay on this PC." "Every answer points to the moment it came from."
- No: "AI-powered", "revolutionize", "seamlessly", "supercharge", exclamation marks.
- Errors blame nothing and suggest a next step: "Couldn't reach Ollama. Is it running?"
- Never overstate availability, privacy or accuracy.

# App polish spec

Status: ready-for-agent

Make the app feel finished: consistent structure, functional motion, a clearer Settings page, rendered Answers, and a choice of model for every AI step. Vocabulary: `CONTEXT.md` (**Provider**, **Speech-to-text model**, **Language model**, **Transcription order**, **Correction**, **Summary**, **Ask**, **Answer**, **Citation**, **Draft**, **Revision**, **Managed transcription**, **Invite code**). Decisions: ADR 0004 (branding), ADR 0011 and 0012 (cloud keys and Managed first). Design rules: `docs/design/visual-identity.md`.

Pricing is out of scope.

## Goal

A user opens Settings and sees, on separate tabs, which models they have, which one transcribes a new Meeting first, and what leaves their PC. On Ask, Summary and Transcribe they can see and change the model that will run. Answers show lists and emphasis instead of raw Markdown. Moving around the app feels smooth and every view has the same structure.

## Requirements

### Motion (ticket 01)
- Follow the motion table in `visual-identity.md` → Motion. Use the existing duration tokens and `ease-functional`. No new animation library.
- `prefers-reduced-motion` turns all of it off (already global in `tokens.css`).

### Page structure (tickets 08, 09)
- Every view (Library home, Meeting, Search, Settings, Help) has the same page header: title, one line of context, actions on the right.
- Import progress, import errors and unreadable folders go to one notification area under the header instead of stacked banners.
- With no Meeting selected, the main pane shows a Library table: Title · Date · Duration · Language · Status (processing %, failed, Summary made). Sortable by Title, Date and Duration. Clicking a row opens the Meeting. The sidebar list stays.
- After 08 and 09, do an audit pass of every screen against `visual-identity.md` and file what's left as new tickets.

### Settings (tickets 02–07)
- Tabs (existing `components/ui/Tabs`): **General** · **Speech-to-text** · **Language models** · **Keys** · **Privacy**.
- **General:** Your name, Default language, Library folder, Process after I stop.
- **Speech-to-text:**
  - Transcription order summary at the top (read-only). It lists Managed transcription → each cloud Provider's automatic Speech-to-text model → local Whisper, each with a status (Ready, Not connected, No key, Downloads when first used), and says which one a new Meeting uses first.
  - List of Speech-to-text models. Local Whisper models are built in. Managed transcription appears when connected. Groq and Gemini models are added by the user (Add → Provider → model → Test). Each Provider has one model marked "Used automatically".
  - Managed transcription collapsed behind "Have an invite code?" unless connected.
  - Vocabulary.
- **Language models:**
  - List of Language models added by the user (Add → Provider → model from the Provider's list or typed ID → Test). Providers: Ollama, OpenAI, Anthropic, Gemini.
  - **Use for** defaults: Correction (or Off), Summary, Ask.
  - Choosing a cloud model for Correction shows a one-time confirmation ("Transcripts will be sent to X after every transcription") before it's saved. Ollama or Off needs none.
- **Keys:** the only place a key is entered, saved, tested or removed: Groq, Gemini, OpenAI, Anthropic, and the Ollama address. Adding a model whose Provider has no key links here. The cloud notice ("Got it") shows here right after a key is saved.
- **Privacy:** one row per place data can go (Managed, Groq, Gemini, OpenAI, Anthropic, and the Correction model): what is sent, when, and whether it is on. The cloud notice text stays here after it is dismissed.
- Long privacy text next to a control becomes one line plus a "Details" disclosure. The disclosure and the Privacy tab use the same text source.
- Never call Transcription, Correction, Summary and Ask "tasks" in the UI. Use "Use for".

### Model choice at the point of use (tickets 11, 12)
- One shared picker component, grouped **On this PC** (Ollama, local Whisper) and **Cloud** (each option marked "sends to X"). It lists only models of the right kind that are ready.
- **Ask:** a "Provider · model" chip under the input. Each Answer records and shows the model it used.
- **Summary:** a split button "Generate with X ▾" (and on Redo). The Summary footer already shows the model.
- **Transcribe / Transcribe again with…:** in the Meeting's ⋯ menu, a dialog listing Speech-to-text models with status. A chosen model runs that transcription only, with no fallback, and makes a new Revision. The default is "Automatic (Transcription order)". The automatic run after Stop always uses the order.
- A choice at the point of use never changes Settings. On Ask it stays for follow-up Asks until changed or the Meeting is closed; on Summary and Transcribe it applies to that run only.
- Nothing configured: the Ask and Summary panels show an info state ("No Language model is set up for Summary. Set one up →") that opens the Language models tab. Transcription always has local Whisper, so it never reaches this state.
- Cloud sends still go through the existing disclosure dialog with the chosen Provider and model.

### Rendered Answers (ticket 10)
- Answers and Summary items render a safe Markdown subset: paragraphs, line breaks, bold, italic, bulleted and numbered lists, inline code. Headings render as bold text. Links, images, tables and raw HTML are shown as plain text, never rendered.
- Citation chips stay after the text. The streaming Answer renders the same way.
- Library files (`summary.md`) are unchanged.

## Out of scope
- Pricing, payments, accounts.
- A user-defined Transcription order (would need an ADR amending 0011/0012).
- New Speech-to-text Providers (e.g. OpenAI transcription).
- A visual refresh of colour, type or brand (ADR 0004).

## Tickets
| # | Ticket | Blocked by |
|---|---|---|
| 01 | Motion coverage | — |
| 02 | Settings tabs and regrouping | — |
| 03 | Keys tab | 02 |
| 04 | Speech-to-text models and Transcription order summary | 02, 03 |
| 05 | Language models, Use for defaults and Correction consent | 02, 03 |
| 06 | Managed collapse and Details disclosures | 02 |
| 07 | Privacy tab | 03, 05, 06 |
| 08 | Page header and notification area | — |
| 09 | Library table home | 08 |
| 10 | Markdown in Answers and Summary items | — |
| 11 | Model picker for Ask and Summary | 05 |
| 12 | Transcribe again with a chosen model | 04 |

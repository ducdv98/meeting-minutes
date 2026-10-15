# Website design

Design spec for `meeting-minutes.tech`. The website uses the app's identity: every colour, font, radius, motion value and voice rule comes from `docs/design/visual-identity.md`, and this file only adds what is specific to the website. If the two disagree, the identity file wins.

Status copy must be plain about what exists and never overstate availability. Do not publish a roadmap or target dates.

## Intent

Someone landing on the site should understand in five seconds what the app does, that it runs on their PC, and whether they can have it today. It should feel like the app it sells: calm, precise, ink on paper, with the product itself as the main image. The same identity at a slightly larger scale, not a louder one.

Avoid: gradient blobs, glassmorphism, AI sparkle imagery, floating abstract dashboards, repeated three-card feature grids, oversized lowercase poster headlines, marquees, fake proof.

## Layout

- Container: `min(100% - 48px, 1120px)`, centred. Outer margin 24px desktop, 16px mobile.
- Reading width for paragraphs: 36rem.
- 12-column grid on desktop, 4 on mobile. Text-and-image sections split 5/7, alternating sides; everything else left-aligned in the reading width. Centre only the hero and the closing call to action.
- Section spacing: 128px desktop, 96px tablet, 64px mobile. Alternate sections may sit on `--panel`.
- Header: 64px, `--canvas` background, 1px `--line` bottom edge once scrolled. Wordmark left; Pricing, Privacy, Support, Changelog and the download button right. Collapses to wordmark + menu under 768px.

## Pages

### Home

In this order:

1. **Hero.** Headline (`--display`, sentence case, at most two lines), one sentence beneath it (`--text-lg`, `--ink-muted`), the primary action, and a status line under the button.
   - Headline direction: "Your meetings, written down and on your PC." Final copy follows the voice rules.
   - Primary action, when a Release is published: "Download for Windows" (ink primary button) with the version and size under it, plus "Windows 11 · Free · unsigned, see why" linking to the SmartScreen note.
   - With no published Release: no download button. Say only that the Windows app is coming.
   - Below: a large real screenshot of the Meeting view (Transcript, Summary with Citations), `--radius-lg`, 1px `--line` edge, no device frame, light screenshot in light mode and dark in dark.
2. **What it does.** Three short blocks in a row (stacked on mobile), each a `--heading-sm` title and one or two sentences, no icons in bubbles: Record any call (Microphone and System audio, any meeting app). Transcription your way (the eleven Meeting languages with Auto-detect; local Whisper by default, or the user's own Groq or Gemini key). Answers with Citations (every Summary item and Answer points to the moment it came from).
3. **Product sections.** Two or three 5/7 text-and-screenshot rows, each showing one real feature: Ask with cited Answers; the Transcript with Speaker labels and playback; Library search. Screenshots of real UI only.
4. **Privacy, plainly.** A `--panel` section: what stays on the PC (the Library folder), what goes to a provider under the user's own key (audio to Groq or Gemini for transcription and Transcript text to Gemini for Correction once a key is added; Transcript text to Gemini, OpenAI or Anthropic for Summary and Ask only when the user clicks), that Gemini's free tier may use content for training, no telemetry, no account. Link to the privacy policy.
5. **Works with any call app.** One sentence: it records what the PC plays and what the microphone hears, so Zoom, Teams, Google Meet and others work without an integration. Text only, no third-party logos.
6. **FAQ.** Full-width accordion rows with 1px `--line` dividers and a `+` that rotates to `×`. Questions: Is it free? Where are my meetings stored? Does anything get uploaded? Why does Windows warn me when I install it? Which languages does it understand? Do I need a powerful PC? How do I uninstall it, and do I keep my meetings?
7. **Closing call to action.** Centred: one line in `--heading-lg` and the same primary action as the hero (or the Windows app is coming status when nothing is published).
8. **Footer.** Wordmark, one sentence, links (Pricing, Privacy, Support, Changelog, GitHub), "Made by Duc Dao", `admin@meeting-minutes.tech`, and the year.

Do not add customer logos, metrics, testimonials or use-case pages.

### Pricing

Two plan cards: Free plan and Pro plan. Free shows $0 forever and “Available now” for local Processing and the user's own provider keys. An optional Account and its limited Managed transcription allowance are marked “Planned” within Free; Pro is marked “Coming soon” and has no price. A reading-width section explains Processed audio minutes, the monthly reset and transcription fallback. Cards use the site's surface, border and spacing tokens, stack at mobile widths, and use text for every availability state. No purchase or registration action appears until those flows exist.

### Privacy

A plain reading page in the reading width: `--heading-lg` title, last-updated date, then sections with `--heading-sm` headings. The content is written from the app code.

### Support

How to get help: email, GitHub issues, how to use Help → Copy diagnostics in the app (with a screenshot), the SmartScreen walkthrough, and how to uninstall while keeping the Library.

### Changelog

One entry per published Release, newest first: version, date and notes, generated from GitHub Releases at build time.

### `llms.txt`

A plain-text summary of the product and its availability, the maker and the contact, with links to each page.

## Components

Built from the identity's tokens:

- **Buttons:** primary (ink fill, canvas text), secondary (transparent, `--line-strong` border, ink text), and text link with an arrow that moves 4px on hover. 40px tall (44px on touch), `--radius-sm`, weight 500.
- **Screenshot frame:** image, `--radius-lg`, 1px `--line` edge, no shadow. Alt text describes what the screenshot shows.
- **Accordion row:** button with `aria-expanded`, the whole row clickable, a 180ms height transition.

## Motion

Hero heading, description and action fly in with short staggered delays. The main product screenshot rises into place, then floats by 18px. Feature screenshots fly in from alternating sides once they enter the viewport; feature text blocks and support screenshots rise in once. Sections still fade and rise 12px on scroll. No parallax, pinning, marquees or scroll-linked animation. `prefers-reduced-motion` disables all of it.

## Responsive

Check at 1440, 1024, 768, 390 and 360px wide.
- Under 768px: single column, `--display` drops to 40px, the primary button spans the full width, and screenshots crop to the most relevant region rather than shrinking text to illegible sizes.
- No horizontal page scroll at any width.

## Accessibility

WCAG AA contrast (guaranteed by the identity tokens), semantic headings in order, keyboard access to every control with the identity's focus ring, 44px touch targets, alt text on every screenshot, a skip link, and a `lang` attribute on the page.

## Implementation

- A static site in `website/`, built in CI and deployed to `meeting-minutes.tech`. No client-side framework is needed; keep JavaScript to the accordion, theme handling and nothing else.
- Fonts self-hosted (Be Vietnam Pro, Inter). No third-party scripts, trackers, analytics or font CDNs, matching the app's privacy promise.
- The latest published Release (version, date, size, installer URL) and the changelog are fetched from the GitHub API at build time, not in the browser.
- Light and dark follow the system; no theme toggle needed.

## Acceptance criteria

- [ ] Every colour, font, radius and motion value comes from the identity tokens.
- [ ] Side by side with the app, the site reads as the same product.
- [ ] Nothing on the site claims a download, user, metric or feature that does not exist.
- [ ] The download area matches the latest published Release, or says only that the Windows app is coming when there is none.
- [ ] No roadmap or target date appears in public copy.
- [ ] Works at all listed widths, in light and dark, with reduced motion, by keyboard.
- [ ] No third-party requests in the browser's network panel.

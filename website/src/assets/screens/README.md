# Real app screenshots needed before publishing

Capture each state in the finished Windows app at a 1200 × 800 px window, with no personal information or invented Meeting content. Save PNGs at their native resolution. Both light and dark variants are required; the website switches with `prefers-color-scheme`.

- `meeting-light.png`, `meeting-dark.png`: An open Meeting after Processing, with the Transcript and a Summary containing visible Citations. Include the full Meeting view.
- `ask-light.png`, `ask-dark.png`: The Ask panel for one Meeting, with a real Answer and clickable Citations visible. Include the question and its source context.
- `transcript-light.png`, `transcript-dark.png`: The Transcript tab, with several real Segments, Me and Remote Speaker labels, timestamps and playback controls visible.
- `search-light.png`, `search-dark.png`: The Library search view with a real keyword query, matching Transcript snippets and a result that opens its Segment.
- `diagnostics-light.png`, `diagnostics-dark.png`: The Help menu open over the app with Copy diagnostics visible. No copied diagnostic output or private device details should appear.
- `firstrun-model-light.png`, `firstrun-model-dark.png`: The first-run setup guide showing its hardware-based transcription model recommendation before download.

The non-draft build fails until every file exists. A draft build shows a neutral “Screenshot” frame in missing slots.

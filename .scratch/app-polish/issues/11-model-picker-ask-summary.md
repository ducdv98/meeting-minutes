# Model picker for Ask and Summary

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 05

## What

- A shared model picker component grouped **On this PC** and **Cloud**; cloud options marked "sends to X". It lists only Language models that are ready; others appear disabled with "Set up in Settings →".
- **Ask:** a "Provider · model" chip under the input, defaulting to the Ask default. The choice stays for follow-up Asks until changed or the Meeting is closed. It never changes Settings.
- **Summary:** a split button "Generate with X ▾" (also on Redo), defaulting to the Summary default.
- Each Answer records the Provider and model it used (in `asks.json`) and shows it in small text. Summaries already record theirs.
- The cloud disclosure dialog shows the chosen Provider and model.
- Nothing set up for that step: the panel shows an info state ("No Language model is set up for Ask. Set one up →") that opens Settings on the Language models tab, instead of the input or Generate button.

## Acceptance criteria

- [ ] Asking with a non-default model sends to that model and records it on the Answer (tests).
- [ ] Settings defaults are unchanged after using the picker.
- [ ] Old `asks.json` without a model field still loads.
- [ ] The info state appears with no Language models and opens the right tab.

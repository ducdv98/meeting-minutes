# Keys tab

Status: ready-for-agent
Spec: ../spec.md
Blocked by: 02

## What

- The Keys tab is the only place to enter, save, test and remove a key: Groq, Gemini, OpenAI, Anthropic, plus the Ollama address if configurable today (otherwise show the default address read-only).
- One row per Provider: name, what it's used for (Speech-to-text, Language models or both), status (Key saved / No key), Save, Test, Remove, and the "Get a free key" link where one exists.
- Remove the key fields from the Speech-to-text and Language models tabs. They show a status line and "Manage keys" link instead.
- The cloud notice ("With a key saved, Meeting audio and text are sent…", "Got it") shows on the Keys tab right after a cloud key is saved, until acknowledged.

## Acceptance criteria

- [ ] Each key can be entered in exactly one place in the app.
- [ ] Saving, testing and removing each key works as before (keys stay in Windows Credential Manager).
- [ ] The Gemini key appears once, marked as used by both kinds.
- [ ] UI tests cover save, test, remove and the cloud notice for at least Groq and OpenAI.

# Gemini text provider for Summary, Action items and Ask

Status: ready-for-human
Spec: ../spec.md
Blocked by: 01

## What

- Implement `TextProvider` for Gemini (the Generative Language API `generateContent` / `streamGenerateContent`) through `HttpClient`.
- Support:
  - streaming deltas;
  - the JSON-schema requests `generate_json` needs, via `responseMimeType=application/json` with `responseSchema`;
  - cancel.
- Default to the current free-tier Flash model, overridable in Settings like the other providers' models (`Settings::model_for`).
- Gemini appears in the Summary and Ask provider choice. The existing per-Meeting cloud disclosure applies unchanged.
- Map 429 and quota errors to a clear message: "Gemini free limit reached, try later or pick another provider".

## Acceptance criteria

- [ ] Fake-HTTP tests: streaming, JSON-schema output parsed by `generate_json`, error mapping, cancel.
- [ ] Summary and Ask tests pass with a fake Gemini provider, including Citation checks.
- [ ] An ignored live test makes one small `generate_json` call with `GEMINI_API_KEY`.

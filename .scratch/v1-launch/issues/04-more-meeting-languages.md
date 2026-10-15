# 04: Support more Meeting languages

**What to build:** Meeting language grows from English and Vietnamese to English, Vietnamese, Spanish, French, German, Portuguese, Italian, Japanese, Korean, Chinese and Hindi, with Auto-detect as the default for new Meetings. This covers the core `Language` type, local Whisper, Groq, Gemini and Managed language codes, Correction, Summary and Ask prompts (written in the Meeting language), the Meeting header picker, the Library table and sidebar labels, and Library search, which folds accents for every language instead of only Vietnamese. The interface stays in English. The website FAQ lists the languages. See `.scratch/v1-launch/spec.md`.

**Blocked by:** None

Status: ready-for-agent

- [ ] Every listed language can be chosen per Meeting, and Auto-detect is the default
- [ ] Each Provider receives the right language code
- [ ] Summary and Answers come back in the Meeting language
- [ ] Search finds accented and unaccented spellings in every language
- [ ] Existing en/vi Meetings and settings load unchanged
- [ ] The website FAQ lists the supported languages

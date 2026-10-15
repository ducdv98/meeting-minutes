export const MEETING_LANGUAGES = [
  { code: "en", name: "English", label: "English" },
  { code: "vi", name: "Vietnamese", label: "Tiếng Việt" },
  { code: "es", name: "Spanish", label: "Spanish" },
  { code: "fr", name: "French", label: "French" },
  { code: "de", name: "German", label: "German" },
  { code: "pt", name: "Portuguese", label: "Portuguese" },
  { code: "it", name: "Italian", label: "Italian" },
  { code: "ja", name: "Japanese", label: "Japanese" },
  { code: "ko", name: "Korean", label: "Korean" },
  { code: "zh", name: "Chinese", label: "Chinese" },
  { code: "hi", name: "Hindi", label: "Hindi" },
] as const;

export type Language = typeof MEETING_LANGUAGES[number]["code"];

export function meetingLanguageName(code: string | null): string {
  return MEETING_LANGUAGES.find((language) => language.code === code)?.name ?? (code || "Auto-detect");
}

export function meetingLanguageLabel(code: string | null): string {
  return MEETING_LANGUAGES.find((language) => language.code === code)?.label ?? (code || "Auto-detect");
}

import type { ProviderKind } from "./ai";

export const MANAGED_PRIVACY_SUMMARY = "Sends Meeting audio to the Meeting Minutes service; nothing is stored.";
export const MANAGED_PRIVACY_STATEMENT = "Managed transcription sends your meeting audio over HTTPS to the Meeting Minutes transcription service, which forwards it to Groq for speech-to-text. The service does not store audio: it is held in memory and a temporary file only while each part is checked, then deleted. The transcript of each part is kept for up to 5 minutes so a retry does not reprocess it, then discarded. Your Library stays on this PC. Service logs record request metadata (request ID, timing, outcome) and never audio, transcript text or access tokens.";
export const CLOUD_NOTICE_TEXT = "With a key saved, Meeting audio and text are sent to Groq or Gemini for Processing. Gemini's free tier may use content for training.";

export const PROVIDER_NOTES: Record<ProviderKind, string> = {
  ollama: "On this PC",
  openai: "Sends the Transcript to OpenAI when you click. Key kept in Windows Credential Manager.",
  anthropic: "Sends the Transcript to Anthropic when you click. Key kept in Windows Credential Manager.",
  groq: "Transcription key",
  gemini: "Sends the Transcript to Gemini when you click. Also used for Correction. Key kept in Windows Credential Manager.",
};

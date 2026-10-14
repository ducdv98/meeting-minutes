export const MANAGED_PRIVACY_SUMMARY = "Sends Meeting audio to the Meeting Minutes service; nothing is stored.";
const LIBRARY_LOCAL_TEXT = "Your Library stays on this PC.";
const GEMINI_TRAINING_NOTICE = "Gemini's free tier may use content for training.";
export const MANAGED_PRIVACY_STATEMENT = `Managed transcription sends your meeting audio over HTTPS to the Meeting Minutes transcription service, which forwards it to Groq for speech-to-text. The service does not store audio: it is held in memory and a temporary file only while each part is checked, then deleted. The transcript of each part is kept for up to 5 minutes so a retry does not reprocess it, then discarded. ${LIBRARY_LOCAL_TEXT} Service logs record request metadata (request ID, timing, outcome) and never audio, transcript text or access tokens.`;
export const CLOUD_NOTICE_TEXT = `With a key saved, Meeting audio can be sent to Groq or Gemini for Processing, and Transcript text can be sent to Gemini, OpenAI or Anthropic when you use their Language models. ${GEMINI_TRAINING_NOTICE}`;

export const PRIVACY_INTRO = `${LIBRARY_LOCAL_TEXT} Nothing is sent unless one of these is on.`;
export const PRIVACY_DESTINATIONS = {
  managed: {
    sent: "Meeting audio",
    when: "When transcription uses the Transcription order: after Stop if automatic Processing is on, or when you start transcription.",
    details: MANAGED_PRIVACY_STATEMENT,
  },
  groq: {
    sent: "Meeting audio",
    when: "After Stop if automatic Processing is on and a Groq model is set, or when you choose a Groq Speech-to-text model.",
    details: "Groq receives Meeting audio for Speech-to-text when Processing reaches Groq in the Transcription order, or when you choose a Groq model for one transcription. The key stays in Windows Credential Manager.",
  },
  gemini: {
    sent: "Meeting audio for Speech-to-text; Transcript text for Correction, Summary and Ask",
    when: "After Stop for automatic transcription, after transcription if Gemini is the Correction default, or when you choose a Gemini model for a run.",
    details: `Gemini receives Meeting audio for Speech-to-text when Processing reaches Gemini, or when you choose a Gemini Speech-to-text model. It receives Transcript text when its Language model runs Correction, Summary or Ask. The key stays in Windows Credential Manager. ${GEMINI_TRAINING_NOTICE}`,
  },
  openai: {
    sent: "Transcript text for Correction, Summary and Ask",
    when: "After transcription if OpenAI is the Correction default, or when you choose an OpenAI Language model for Summary or Ask.",
    details: "OpenAI receives Transcript text when its Language model runs Correction, Summary or Ask. Correction runs after transcription when selected as the default; Summary and Ask run when you choose them. The key stays in Windows Credential Manager.",
  },
  anthropic: {
    sent: "Transcript text for Correction, Summary and Ask",
    when: "After transcription if Anthropic is the Correction default, or when you choose an Anthropic Language model for Summary or Ask.",
    details: "Anthropic receives Transcript text when its Language model runs Correction, Summary or Ask. Correction runs after transcription when selected as the default; Summary and Ask run when you choose them. The key stays in Windows Credential Manager.",
  },
  correction: {
    sent: "Transcript text when a cloud Language model is selected",
    when: "After transcription, before the Revision is published.",
    details: "Correction sends Transcript text to the chosen cloud Provider after transcription. Ollama runs on this PC, and Off sends nothing. Correction does not change Segment times or Sources.",
  },
} as const;

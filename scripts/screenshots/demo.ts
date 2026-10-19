// Runs the real UI against a fake backend serving the demo Library.
import { mockIPC, mockConvertFileSrc } from "@tauri-apps/api/mocks";
import * as demo from "./demo-data";

const settings = {
  first_run_completed: true,
  recording_notice_acknowledged: true,
  cloud_notice_acknowledged: true,
  auto_process: true,
  model: "large-v3-turbo-q5_0",
  vocabulary: ["Priya", "Sam"],
  language: null,
  your_name: "Alex",
  ai: { provider: "ollama", ollama_model: "qwen3:8b", openai_model: null, anthropic_model: null, gemini_model: null },
  speech: { models: [], automatic_groq: null, automatic_gemini: null, automatic_whisper: "large-v3-turbo-q5_0" },
  language_models: {
    models: [{ provider: "ollama", model: "qwen3:8b" }],
    correction: { provider: "ollama", model: "qwen3:8b" },
    summary: { provider: "ollama", model: "qwen3:8b" },
    ask: { provider: "ollama", model: "qwen3:8b" },
    confirmed_correction_providers: ["ollama"],
    legacy_correction_pending: false,
  },
};

const aiStatus = {
  provider: "ollama", model: "qwen3:8b", configured: true,
  has_openai_key: false, has_anthropic_key: false, has_groq_key: false, has_gemini_key: false,
  summary_provider: "ollama", summary_model: "qwen3:8b", summary_configured: true,
  ask_provider: "ollama", ask_model: "qwen3:8b", ask_configured: true,
};

const idle = { phase: "idle", meeting_id: null, elapsed_ms: 0, notice_acknowledged: true, saves: [], continue_offers: [], device_switches: [] };

mockConvertFileSrc("windows");
mockIPC((cmd, args) => {
  const a = (args ?? {}) as Record<string, unknown>;
  const meetingId = a.meetingId as string;
  switch (cmd) {
    case "rescan_library": return { meetings: demo.meetings, malformed: [] };
    case "list_meetings": return demo.meetings;
    case "meeting_detail": return demo.detail(meetingId);
    case "meeting_transcript": return demo.transcript(meetingId);
    case "meeting_summary": return meetingId === demo.DEMO_ID ? demo.summary : null;
    case "meeting_asks": return meetingId === demo.DEMO_ID ? demo.asks : { asks: [], active: null, retryable_id: null };
    case "summary_participants_changed": return false;
    case "summary_status": return { jobs: [] };
    case "processing_status": return { jobs: [] };
    case "transcript_draft": return [];
    case "recording_status": return idle;
    case "import_status": return { current: null, queued: 0, errors: [] };
    case "get_settings": return settings;
    case "ai_status": return aiStatus;
    case "managed_status": return { connected: false, revoked: false, email: null, enabled: false };
    case "managed_account": return { email: "alex@example.com", state: "admitted", minutes_used: 0, minutes_remaining: 120, allowance_minutes: 120, resets_at: "2026-11-01T00:00:00.000Z" };
    case "installed_models": return ["large-v3-turbo-q5_0"];
    case "list_participant_names": return ["Alex", "Priya", "Sam"];
    case "list_ai_models": return ["qwen3:8b"];
    case "search_library": return demo.search(a.query as string);
    case "app_info": return { version: "1.1.0" };
    case "first_run_info": return { discrete_gpu: true, library_location: "C:\\Users\\Alex\\Documents\\Meeting Minutes" };
    case "model_download_status": return [];
    case "acknowledge_recording_notice": return idle;
    case "get_diagnostics": return demo.diagnostics;
    case "plugin:event|listen": return 0;
    case "plugin:event|unlisten": return null;
    default:
      console.warn("demo: unhandled command", cmd, a);
      return null;
  }
});

await import("../../src/main");

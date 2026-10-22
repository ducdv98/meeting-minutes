// Fictional demo Library for website screenshots. No real people or calls.
import type { MeetingDetail, MeetingSummary, SearchHit } from "../../src/library";
import type { Segment, Transcript } from "../../src/processing";
import type { Summary } from "../../src/summary";
import type { AskView } from "../../src/ask";

const DAY = 86_400_000;
function at(daysAgo: number, hour: number, minute: number): string {
  const date = new Date(Date.now() - daysAgo * DAY);
  date.setHours(hour, minute, 0, 0);
  return date.toISOString();
}

export const DEMO_ID = "2026-10-15T10-02-11";

export const meetings: MeetingSummary[] = [
  { id: DEMO_ID, title: "Website launch sync", started_at: at(0, 10, 2), duration_ms: 31 * 60_000 + 40_000 },
  { id: "m2", title: "Weekly product review", started_at: at(0, 8, 30), duration_ms: 46 * 60_000 },
  { id: "m3", title: "Customer interview — Northwind", started_at: at(1, 15, 0), duration_ms: 38 * 60_000 },
  { id: "m4", title: "Hiring debrief", started_at: at(1, 11, 15), duration_ms: 22 * 60_000 },
  { id: "m5", title: "Họp kế hoạch quý 4", started_at: at(3, 9, 0), duration_ms: 54 * 60_000 },
  { id: "m6", title: "Design critique: onboarding", started_at: at(4, 14, 30), duration_ms: 41 * 60_000 },
  { id: "m7", title: "Voice note — release checklist", started_at: at(6, 18, 10), duration_ms: 4 * 60_000 },
  { id: "m8", title: "Investor update call", started_at: at(19, 16, 0), duration_ms: 63 * 60_000 },
  { id: "m9", title: "Partner kickoff — Contoso", started_at: at(24, 10, 0), duration_ms: 49 * 60_000 },
];

const languages: Record<string, string> = { m5: "vi" };

export function detail(id: string): MeetingDetail {
  const meeting = meetings.find((item) => item.id === id) ?? meetings[0];
  return {
    ...meeting,
    language: languages[id] ?? "en",
    audio_path: `C:\\Users\\demo\\Documents\\Meeting Minutes\\${id}\\audio.ogg`,
    markers: id === DEMO_ID ? [{ kind: "pause", at_ms: 14 * 60_000 + 5_000, paused_ms: 90_000 }] : [],
    imported: false,
    sources: id === "m7" ? ["microphone"] : ["microphone", "system_audio"],
    voice_note: id === "m7",
    participants: id === DEMO_ID ? { remote: "Priya" } : {},
    segment_participants: id === DEMO_ID ? { revision: 2, segments: { s00006: "Sam", s00009: "Sam", s00014: "Sam", s00020: "Sam", s00023: "Sam" } } : null,
    correction_notice: null,
  };
}

// [speaker, start seconds, end seconds, text]
const lines: [Segment["speaker"], number, number, string][] = [
  ["me", 4, 11, "Okay, I think everyone's here. Let's go through the launch checklist for the website."],
  ["remote", 12, 21, "Sounds good. I pushed the new home page last night, so the hero and the feature sections are live on staging."],
  ["me", 22, 29, "Great. The big open question from last week was the pricing page. Where did we land on that?"],
  ["remote", 30, 42, "I drafted two plans, Free and Pro. Free covers everything local and your own provider keys, with no usage limit."],
  ["remote", 43, 52, "Pro is the cloud transcription allowance. We said we wouldn't show a price until we've measured the cost."],
  ["remote", 53, 61, "I'd still like a way for people to tell us they want Pro, though, without building a signup form."],
  ["me", 62, 70, "What if the Pro card links to a GitHub Discussion? People can upvote it and we don't store any emails."],
  ["remote", 71, 76, "That works for me. It keeps the privacy page simple too."],
  ["remote", 77, 88, "Agreed. Let's do the Discussion link, and keep Pro marked as planned until checkout actually exists."],
  ["me", 89, 95, "Decided, then. Next: screenshots. The site still has placeholder frames."],
  ["remote", 96, 108, "I can script them. Run the app with demo data, capture light and dark at twelve hundred by eight hundred."],
  ["me", 109, 116, "Can you have them ready by Thursday? I want the launch post drafted by Friday."],
  ["remote", 117, 121, "Thursday is fine. I'll post them in the channel when they're done."],
  ["remote", 122, 133, "One more thing from support: two people asked how to verify the installer, since it isn't signed."],
  ["me", 134, 145, "Let's publish a SHA-256 checksum with every release and explain the SmartScreen warning on the support page."],
  ["remote", 146, 155, "I'll add attestation to the release workflow as well, so anyone can check the build came from our CI."],
  ["me", 156, 160, "Perfect. Priya, can you own the support page copy?"],
  ["remote", 161, 166, "Yes, I'll take it. I'll have a draft by Wednesday."],
  ["me", 167, 176, "Last item is languages. Users keep asking for Spanish and Japanese."],
  ["remote", 177, 189, "The large turbo model handles both well in our tests. The small model struggles with Japanese names, though."],
  ["remote", 190, 199, "Then setup should recommend turbo for anyone who picks a language other than English."],
  ["me", 200, 207, "Makes sense. Let's add that to the first-run guide."],
  ["remote", 208, 214, "I'll write the ticket today."],
  ["remote", 215, 226, "And I'll check that search handles accents in every language, not just Vietnamese."],
  ["me", 227, 236, "Great. So: Discussion link for Pro, screenshots by Thursday, checksums and attestation, and turbo for non-English."],
  ["remote", 237, 241, "That's everything on my list."],
  ["me", 242, 247, "Thanks, everyone. Same time next week."],
];

export const segments: Segment[] = lines.map(([speaker, start, end, text], index) => ({
  id: `s${String(index + 1).padStart(5, "0")}`,
  speaker,
  start_ms: start * 1000,
  end_ms: end * 1000,
  text,
}));

// Fictional shared slides, drawn at run time so no image files are needed.
export const demoSnapshots = [
  { order: 1, timeline_ms: 42_500, caption: "Pricing slide: Free and Pro", title: "Plans", lines: ["Free — local Processing + your own keys", "Pro — cloud allowance (planned)", "No price until costs are measured"] },
  { order: 2, timeline_ms: 134_500, caption: "Release checklist", title: "Release checklist", lines: ["Publish SHA-256 with every release", "Explain the SmartScreen warning", "Add build attestation to the workflow"] },
];

export async function snapshotPng(order: number): Promise<number[]> {
  const slide = demoSnapshots.find((item) => item.order === order)!;
  const canvas = document.createElement("canvas");
  canvas.width = 1280; canvas.height = 720;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#f4f6fb"; ctx.fillRect(0, 0, 1280, 720);
  ctx.fillStyle = "#2f4ea2"; ctx.fillRect(0, 0, 1280, 14);
  ctx.fillStyle = "#14213d"; ctx.font = "700 72px Inter, system-ui, sans-serif"; ctx.fillText(slide.title, 80, 190);
  ctx.font = "400 44px Inter, system-ui, sans-serif";
  slide.lines.forEach((line, index) => { ctx.fillStyle = "#2f4ea2"; ctx.fillRect(80, 290 + index * 110, 12, 12); ctx.fillStyle = "#14213d"; ctx.fillText(line, 120, 305 + index * 110); });
  const blob: Blob = await new Promise((resolve) => canvas.toBlob((value) => resolve(value!), "image/png"));
  return Array.from(new Uint8Array(await blob.arrayBuffer()));
}

export function transcript(id: string): Transcript | null {
  if (id !== DEMO_ID) return { ...demoTranscript, segments: segments.slice(0, 8) };
  return demoTranscript;
}

const demoTranscript: Transcript = {
  schema_version: 1,
  revision: 2,
  created_at: at(0, 10, 36),
  model: ["large-v3-turbo-q5_0"],
  fallback_reason: null,
  language: "en",
  language_detected: true,
  duration_ms: meetings[0].duration_ms,
  segments,
};

const cite = (...ids: string[]) => ids.map((id) => {
  const segment = segments.find((item) => item.id === id)!;
  return { segment_id: id, start_ms: segment.start_ms, end_ms: segment.end_ms };
});

export const summary: Summary = {
  schema_version: 1,
  revision: 2,
  created_at: at(0, 10, 38),
  provider: "ollama",
  model: "qwen3:8b",
  overview: [
    { text: "The team reviewed the website launch checklist: the pricing page, screenshots, installer verification and language support.", citations: cite("s00001", "s00003") },
  ],
  key_points: [
    { text: "The pricing page shows Free and Pro; Pro has no price until cloud transcription cost is measured.", citations: cite("s00004", "s00005") },
    { text: "Two users asked how to verify the unsigned installer.", citations: cite("s00014") },
    { text: "The large turbo model handles Spanish and Japanese well; the small model struggles with Japanese names.", citations: cite("s00020") },
  ],
  decisions: [
    { text: "The Pro card links to a GitHub Discussion instead of a signup form, and stays marked as planned.", citations: cite("s00007", "s00009") },
    { text: "Every release gets a SHA-256 checksum and build attestation.", citations: cite("s00015", "s00016") },
    { text: "First-run setup recommends the turbo model for non-English languages.", citations: cite("s00021", "s00022") },
  ],
  action_items: [
    { text: "Script light and dark screenshots with demo data", owner: "Sam", due: "Thursday", citations: cite("s00011", "s00012", "s00013") },
    { text: "Draft the support page copy on installer verification", owner: "Priya", due: "Wednesday", citations: cite("s00017", "s00018") },
    { text: "Write the ticket for the turbo recommendation", owner: "Sam", due: null, citations: cite("s00023") },
  ],
  dropped_items: 0,
  participants_fingerprint: null,
};

export const asks: AskView = {
  asks: [
    {
      id: "a1",
      question: "What did we decide about the Pro plan?",
      asked_at: at(0, 10, 41),
      revision: 2,
      provider: "ollama",
      model: "qwen3:8b",
      state: "answered",
      answer: "Pro stays marked as **planned** with no price until the cost of cloud transcription has been measured. Instead of a signup form, the Pro card will link to a GitHub Discussion so people can upvote it without the app storing any emails.",
      citations: cite("s00005", "s00007", "s00009"),
      invalid_citations: [],
      error: null,
    },
    {
      id: "a2",
      question: "Who is writing the support page, and by when?",
      asked_at: at(0, 10, 43),
      revision: 2,
      provider: "ollama",
      model: "qwen3:8b",
      state: "answered",
      answer: "Priya owns the support page copy and will have a draft by Wednesday.",
      citations: cite("s00017", "s00018"),
      invalid_citations: [],
      error: null,
    },
  ],
  active: null,
  retryable_id: null,
};

const mark = (text: string, word: string) => text.replace(new RegExp(`(${word})`, "i"), "\u0002$1\u0003");

export function search(query: string): SearchHit[] {
  const word = query.trim();
  if (!word) return [];
  const hits: SearchHit[] = segments
    .filter((segment) => segment.text.toLowerCase().includes(word.toLowerCase()))
    .map((segment) => ({ meeting_id: DEMO_ID, revision: 2, title: meetings[0].title, started_at: meetings[0].started_at, segment_id: segment.id, start_ms: segment.start_ms, end_ms: segment.end_ms, snippet: mark(segment.text, word) }));
  const others: [string, number, string][] = [
    ["m2", 754_000, "If the pricing page goes live first, we can link to it from the changelog."],
    ["m3", 1_312_000, "Honestly, the pricing was the reason we picked you over the other tools."],
    ["m8", 2_045_000, "We'll revisit pricing once we have a month of usage data."],
  ];
  for (const [id, start, text] of others) {
    if (!text.toLowerCase().includes(word.toLowerCase())) continue;
    const meeting = meetings.find((item) => item.id === id)!;
    hits.push({ meeting_id: id, revision: 1, title: meeting.title, started_at: meeting.started_at, segment_id: "s00042", start_ms: start, end_ms: start + 7_000, snippet: mark(text, word) });
  }
  return hits;
}

export const diagnostics = `Meeting Minutes 1.2.1
Windows: Windows 11 Pro 24H2
CPU: 8-core x64
RAM: 16384 MB
Audio devices:
- Microphone: Headset Microphone
- System audio: Speakers
Selected model: large-v3-turbo-q5_0
Provider: Ollama
Transcription backend: Vulkan
Last transcription RTF: 0.08
Last processing failure: none`;

# Meeting Minutes

A single-user, local-first Windows app that records calls from any meeting app, turns them into timed transcripts, and answers cited questions about one meeting at a time. A Managed key gives access to experimental Managed transcription; Meetings remain local, without sharing or sync.

## Language

### Capture

**Meeting**:
One conversation the user wants to keep, holding one or more Recordings played back as a single timeline.
_Avoid_: Session, call, note

**Title**:
The user-chosen name of a Meeting. It can be changed at any time, including while the Meeting is recording or being processed, and changing it never moves the Meeting's folder or changes its identity.
_Avoid_: Label, tag, folder name

**Recording**:
One continuous capture run belonging to a Meeting. Pausing continues the same Recording; a restart after a crash adds a new Recording to the same Meeting.
_Avoid_: Take, file, clip

**Source**:
One distinguishable audio input of a Recording: the **Microphone** (the user) or **System audio** (everything the PC plays, including remote participants).
_Avoid_: Channel, track, device

**Timeline**:
The single playback time axis of a Meeting: its Recordings joined in order, with pauses and gaps removed and marked. Segment ranges and Citations are measured on the Timeline.

**Marker**:
A point on the Timeline recording an event: a pause, a gap between Recordings, or an audio device switch.

**Snapshot**:
An image of a region of the user's screen, taken by hand during a Recording and anchored to a position on the Timeline. It is kept as an image file in the Meeting's folder and shown inline in the Transcript at that position, with an optional caption the user adds later. A Snapshot is user content, not an event, so it is not a Marker, and it is never a Citation. Summary and Ask do not read it, and it stays on the user's PC.
_Avoid_: Screenshot, capture, screen grab

**Capture region**:
The rectangle of the screen the user drags out for Snapshots. The Capture bar's Snapshot button asks for a new one each time; the Take Snapshot hotkey reuses the last one in the same Recording, and a second hotkey draws a new one. It lasts for the Recording and is not saved to the Meeting.

**Capture bar**:
The small always-on-top floating bar shown while a Recording runs, with pause, stop, Snapshot and a timer. Starting a Recording stays in the main window. The bar and the region overlay never appear in a Snapshot or in screen sharing.

**Chunk**:
A durably written slice of a Source's audio. Completed Chunks survive a crash and are never discarded by a failed save.

**Import**:
Creating a Meeting from an existing audio file instead of capturing it. An imported Meeting has no Source split.

**Voice note**:
A Meeting recorded with only the Microphone Source.

**Library**:
The user-owned folder holding every Meeting as a readable folder of audio and transcript files. The Library is the source of truth; the app can rebuild everything else from it.
_Avoid_: Database, vault, workspace

**Recording notice**:
The reminder that other participants may need to consent to being recorded, plus the always-visible indicator while a Recording runs. It also says that Snapshots may capture what others share, and each Snapshot is confirmed on screen when taken.

### Access

**Free plan**:
The permanent app offering, with no app-imposed usage limit on local Processing or cloud Processing through the user's own provider keys. It does not require an Account; a Managed key carries its own limited Managed transcription allowance.
_Avoid_: Free tier, Registered tier

**Account**:
The planned identity of one user across installations. Accounts are deferred while Managed transcription is experimental and do not grant Managed access during this period.
_Avoid_: Install, Invite code

**Managed access**:
Permission to use Managed transcription, granted by holding a **Managed key**. During the experimental period the operator issues keys by hand to people who ask; Accounts, automatic admission and the Waitlist are deferred until later.

**Managed transcription allowance**:
The monthly amount of Managed transcription a Managed key may use at no charge, measured in Processed audio minutes and reset on the first day of each month in UTC without rollover. It covers transcription of audio, not Correction, Summary or Ask.
_Avoid_: Gateway quota, cloud credits

**Processed audio minute**:
One minute of audio successfully transcribed by Managed transcription; each Source counts separately, and an Import counts as one stream. Re-transcribing the same audio counts again.

**Pro plan**:
A planned paid offering with a larger Managed transcription allowance, shown as coming soon and not yet available for purchase.
_Avoid_: Pro tier

### Understanding

**Meeting language**:
The spoken language of a Meeting, one of the app's supported languages, chosen by the user or detected. One language per Meeting; its Summary and Answers are written in it.

**Processing**:
Turning a Meeting's audio into a Transcript, then a Summary and Action items. Processing starts automatically after Stop, in the cloud when the user has connected Managed transcription or set up a provider key, and locally otherwise. One Meeting's Transcript may come partly from each.

**Managed transcription**:
Cloud transcription run by the Meeting Minutes service, experimental for now and enabled by a user who holds a Managed key. Saving the key is the user's consent to sending audio to the cloud, as with a provider key. Its one Speech-to-text model is **mm-transcription-internal**.
_Avoid_: Gateway, relay, hosted Groq

**Managed key**:
The key the operator issues, after the user asks for it through the request form on the website, that gives access to Managed transcription. The user pastes it into Settings and it is kept like a provider key. It carries its own monthly Managed transcription allowance.
_Avoid_: Invite code, license key, token

**Provider**:
A cloud service or local runtime that runs a model for the app: Managed transcription, Groq, Gemini, OpenAI, Anthropic, Ollama or local Whisper. A cloud Provider other than Managed transcription is used only with the user's own key; Managed transcription is used only with a Managed key.
_Avoid_: Vendor, backend, engine

**Speech-to-text model**:
A model the user has added (or that is built in) for turning Meeting audio into a Transcript. Each Provider has one Speech-to-text model used automatically; the others are used only when the user chooses them for a run.
_Avoid_: Transcription engine, ASR

**Language model**:
A model the user has added for working with text: Correction, Summary and Ask. Each of these has a default Language model, and the user can choose another for a single run of Summary or Ask.
_Avoid_: General model, AI model, LLM

**Transcription order**:
The fixed order of Providers that Processing tries for transcription, each with its automatic Speech-to-text model: Managed transcription, then the user's own cloud keys, then local Whisper. A Provider that isn't enabled or available is skipped, and one that fails or hits its limit passes the work to the next. The user can't reorder it. Transcribing with one chosen Speech-to-text model skips the order for that run only, with no fallback.
_Avoid_: Engine chain, fallback chain

**Invite code**:
A one-time code from the operator that formerly connected one install of the app to experimental Managed transcription. Retired in 1.0 in favor of Managed keys.
_Avoid_: License key, account, token

**Transcript**:
The ordered list of Segments for a Meeting. Re-transcribing produces a new **Revision**; it never silently rewrites the old one. Editing a Segment's text keeps its ID and Revision.

**Vocabulary**:
The user's list of names and terms given to transcription as a hint. A Meeting's Participant names are added to it for that Meeting only.

**Segment**:
A span of transcribed speech with an app-assigned stable ID and a millisecond range on the Meeting timeline. A Segment is about one sentence from one Source: it never crosses a change of Speaker label, a fragment too short to stand alone joins a neighbour, and an overlong sentence is split at its longest pause. Segments from different Sources may overlap in time when people talk over each other.
_Avoid_: Line, utterance, chunk

**Correction**:
The step of Processing that fixes a Transcript's wording (names, Vocabulary terms, punctuation, diacritics) before its Revision is published. It never changes a Segment's time range or Source, and never merges or splits Segments. It runs on the user's chosen Language model, or not at all when the user has turned it off. If it fails, the uncorrected text is published.
_Avoid_: Cleanup, polish, rewrite

**Draft**:
The Segments shown while a Meeting is still being transcribed, before its Revision is published. A Draft is read-only and cannot be cited, asked about or searched; it is replaced by the Revision when Processing completes, and discarded if Processing fails.
_Avoid_: Partial transcript, live transcript

**Speaker label**:
Which Source a Segment was transcribed from: **Me** (Microphone) or **Remote** (System audio). **Unknown** is used only when the audio has no Source split, as in an Import. Set by the app, never edited by the user.

**Participant**:
A named person in one Meeting, chosen by the user. Each Speaker label starts with a default Participant: Me is the user's name from Settings (overridable per Meeting), Remote is "Remote" until named, and in a Voice note every Segment belongs to the user. The user may move any Segment to another Participant. Names are unique within a Meeting; giving a Participant an existing name merges them. There is no Library-wide person: "Anna" in two Meetings is two Participants with the same name. A new Revision keeps the Participants, and each new Segment goes to whichever Participant covered most of its time in the previous Revision. Participant names appear wherever the Transcript does: the Library files, Summaries, Asks and Library search.
_Avoid_: Speaker name, attendee, person

**Summary**:
A generated overview of a Meeting with fixed sections (Overview, Key points, Decisions, Action items). Every item has a Citation; an item without one is dropped.

**Action item**:
A task stated in a Meeting, derived from its Transcript, citing the Segments it came from.

### Asking

**Ask**:
A question the user poses about exactly one Meeting, answered only from that Meeting's Transcript.
_Avoid_: Chat, query, prompt

**Answer**:
The response to an Ask: either supported by Citations, or an explicit **Insufficient evidence** result. An Answer may also be **Incomplete** (stopped or failed mid-stream) or **Unverified** (contains invalid Citations); neither is used as context for later Asks.

**Library search**:
Keyword search across every Transcript in the Library, landing on a Segment. Not an Ask: it never generates an answer.

**Citation**:
A reference from an Answer, Summary or Action item to a Segment ID in the same Meeting. Model-written timestamps are never Citations.

## Relationships

- A **Meeting** has one or more **Recordings**; an imported **Meeting** has none and holds its audio directly
- A **Recording** has one or two **Sources**, each made of **Chunks**
- A **Meeting** has at most one current **Transcript**, made of **Segments**
- A **Meeting** has any number of **Snapshots**, each at one position on its **Timeline**; one taken while paused sits at the pause **Marker**
- A **Segment** has exactly one **Speaker label** and belongs to one **Revision**
- A **Segment** is attributed to exactly one **Participant**; by default the one for its **Speaker label**
- A **Participant** belongs to exactly one **Meeting**
- A **Citation** points to a Segment in a specific **Revision**
- An **Ask** belongs to exactly one **Meeting**; its **Citations** point only to that Meeting's **Segments**

## Example dialogue

> **Dev:** "The app crashed 20 minutes into the call and I hit Record again — is that a new **Meeting**?"
> **Domain expert:** "No. The surviving **Chunks** become the first **Recording**, the restart is a second **Recording**, and both play back as one **Meeting** timeline."

> **Dev:** "Three people joined from the other side. Are they three **Speaker labels**?"
> **Domain expert:** "No. They are all **Remote**, because they came through System audio. The user names them as three **Participants** and moves Segments between them; the **Speaker label** stays Remote."

## Flagged ambiguities

- "Task" means an Action item. Transcription, Correction, Summary and Ask are not tasks; name them directly.

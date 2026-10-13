# Meeting Minutes

A single-user, local-first Windows app (no accounts, sharing or sync) that records calls from any meeting app, turns them into timed transcripts, and answers cited questions about one meeting at a time.

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
The reminder that other participants may need to consent to being recorded, plus the always-visible indicator while a Recording runs.

### Understanding

**Meeting language**:
The spoken language of a Meeting (English or Vietnamese), chosen by the user or detected. One language per Meeting.

**Processing**:
Turning a Meeting's audio into a Transcript, then a Summary and Action items. Local Processing may start automatically after Stop; cloud Processing only starts on an explicit user action.

**Transcript**:
The ordered list of Segments for a Meeting. Re-transcribing produces a new **Revision**; it never silently rewrites the old one. Editing a Segment's text keeps its ID and Revision.

**Vocabulary**:
The user's list of names and terms given to transcription as a hint. A Meeting's Participant names are added to it for that Meeting only.

**Segment**:
A span of transcribed speech with an app-assigned stable ID and a millisecond range on the Meeting timeline. A Segment is about one sentence from one Source: it never crosses a change of Speaker label, a fragment too short to stand alone joins a neighbour, and an overlong sentence is split at its longest pause. Segments from different Sources may overlap in time when people talk over each other.
_Avoid_: Line, utterance, chunk

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

- "Meetless" is the macOS reference product this app is modelled on, not this product's name.

import { useCallback, useEffect, useRef, useState } from "react";
import { meetingDetail, renameMeeting, type MeetingDetail } from "./library";
import Player, { PlayerContext, type PlayerHandle } from "./Player";
import type { TimelinePlayer } from "./player";
import { formatElapsed } from "./recording";
import { onLibraryChanged } from "./recording";
import { cancelProcessing, dismissProcessing, enqueueTranscription, meetingTranscript, onProcessingStatus, processingStageLabel, processingStatus, setMeetingLanguage, type Language, type ProcessingJob, type Transcript } from "./processing";

interface Props {
  id: string;
  onBack(): void;
  onRenamed(newId: string): void;
}

function languageName(code: string | null): string {
  if (code === "en") return "English";
  if (code === "vi") return "Vietnamese";
  return code ?? "Not set";
}

export default function MeetingView({ id, onBack, onRenamed }: Props) {
  const [detail, setDetail] = useState<MeetingDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [saving, setSaving] = useState(false);
  const [audioOpen, setAudioOpen] = useState(true);
  const [player, setPlayer] = useState<TimelinePlayer | null>(null);
  const [transcript, setTranscript] = useState<Transcript | null>(null);
  const [job, setJob] = useState<ProcessingJob | null>(null);
  const processingEventSeen = useRef(false);
  const playerHandle = useRef<PlayerHandle>(null);
  const onPlayerChange = useCallback((next: TimelinePlayer | null) => setPlayer(next), []);

  useEffect(() => {
    let active = true;
    processingEventSeen.current = false;
    setError(null);
    meetingDetail(id).then(
      (next) => { if (active) { setDetail(next); setAudioOpen(true); } },
      (reason) => { if (active) setError(String(reason)); },
    );
    meetingTranscript(id).then((next) => { if (active) setTranscript(next); }, (reason) => { if (active) setError(String(reason)); });
    processingStatus().then((next) => { if (active && !processingEventSeen.current) setJob(next.jobs.find((item) => item.meeting_id === id) ?? null); }, (reason) => { if (active) setError(String(reason)); });
    return () => { active = false; };
  }, [id]);

  useEffect(() => {
    let active = true;
    const processing = onProcessingStatus((status) => { if (active) { processingEventSeen.current = true; setJob(status.jobs.find((item) => item.meeting_id === id) ?? null); } });
    const library = onLibraryChanged(() => {
      void meetingTranscript(id).then((next) => { if (active) setTranscript(next); }, (reason) => { if (active) setError(String(reason)); });
      void meetingDetail(id).then((next) => { if (active) setDetail(next); }, () => {});
    });
    return () => { active = false; void processing.then((stop) => stop(), () => {}); void library.then((stop) => stop(), () => {}); };
  }, [id]);

  async function runTranscription() {
    setError(null);
    try { const status = await enqueueTranscription(id); setJob(status.jobs.find((item) => item.meeting_id === id) ?? null); }
    catch (reason) { setError(String(reason)); }
  }
  async function cancelJob() {
    try { const status = await cancelProcessing(id); setJob(status.jobs.find((item) => item.meeting_id === id) ?? null); }
    catch (reason) { setError(String(reason)); }
  }
  async function dismissJob() {
    try { const status = await dismissProcessing(id); setJob(status.jobs.find((item) => item.meeting_id === id) ?? null); }
    catch (reason) { setError(String(reason)); }
  }
  async function changeLanguage(value: string) {
    try {
      await setMeetingLanguage(id, value ? value as Language : null);
      setDetail((current) => current ? { ...current, language: value || null } : current);
    } catch (reason) { setError(String(reason)); }
  }

  function cancelRename() {
    setRenaming(false);
    setError(null);
  }

  async function saveRename() {
    if (!detail || !title.trim() || saving) return;
    setSaving(true);
    setError(null);
    playerHandle.current?.release();
    setAudioOpen(false);
    // Let the player unmount first: Windows cannot rename a folder whose audio file is open.
    await new Promise((resolve) => setTimeout(resolve, 0));
    try {
      const summary = await renameMeeting(detail.id, title.trim());
      // The folder has moved: follow the new id; the view reloads by it and reports its own errors.
      setRenaming(false);
      onRenamed(summary.id);
    } catch (reason) {
      setError(String(reason));
      setAudioOpen(true);
    } finally {
      setSaving(false);
    }
  }

  return (
    <PlayerContext.Provider value={player}>
      <section className="meeting-view">
        <button type="button" disabled={saving} onClick={onBack}>Back to Library</button>
        {error && <p role="alert">{error}</p>}
        {detail && (
          <>
            {renaming ? (
              <div>
                <label htmlFor="meeting-title">Title</label>
                <input id="meeting-title" value={title} onChange={(event) => setTitle(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") { event.preventDefault(); void saveRename(); }
                    if (event.key === "Escape") cancelRename();
                  }} />
                <button type="button" disabled={!title.trim() || saving} onClick={() => void saveRename()}>Save</button>
                <button type="button" onClick={cancelRename}>Cancel</button>
              </div>
            ) : (
              <>
                <h1>{detail.title}</h1>
                <button type="button" onClick={() => { setTitle(detail.title); setError(null); setRenaming(true); }}>Rename</button>
              </>
            )}
            <p><time dateTime={detail.started_at}>{detail.started_at.slice(0, 16).replace("T", " ")}</time></p>
            <p>Duration: {formatElapsed(detail.duration_ms)}</p>
            <label htmlFor="meeting-language">Language</label>
            <select id="meeting-language" value={detail.language ?? ""} disabled={!!job && job.stage !== "failed"} onChange={(event) => void changeLanguage(event.target.value)}>
              <option value="">Auto-detect</option><option value="en">English</option><option value="vi">Vietnamese</option>
            </select>
            {detail.audio_path && audioOpen ? (
              <Player ref={playerHandle} audio_path={detail.audio_path} duration_ms={detail.duration_ms}
                markers={detail.markers} onPlayerChange={onPlayerChange} />
            ) : !detail.audio_path ? <p>No audio yet</p> : null}
            <section aria-label="Transcript" className="transcript-section">
              <h2>Transcript</h2>
              {transcript && <p>Transcript revision {transcript.revision} · {transcript.segments.length} segments · {languageName(transcript.language)}</p>}
              {job && job.stage !== "failed" && <p>{processingStageLabel(job)}</p>}
              {job && (job.stage === "queued" || job.stage === "downloading" || job.stage === "transcribing") && <button type="button" onClick={() => void cancelJob()}>Cancel processing</button>}
              {job?.stage === "failed" && <><p role="alert">{job.error}</p><button type="button" onClick={() => void runTranscription()}>Retry</button><button type="button" onClick={() => void dismissJob()}>Dismiss</button></>}
              {!job && <button type="button" onClick={() => void runTranscription()}>{transcript ? "Re-transcribe" : "Transcribe"}</button>}
            </section>
          </>
        )}
      </section>
    </PlayerContext.Provider>
  );
}

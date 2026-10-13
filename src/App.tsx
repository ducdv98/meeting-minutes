import { useEffect, useState } from "react";
import { dismissImportErrors, importStatus, onImportHover, onImportStatus, pickAudioFiles, type ImportStatus } from "./importing";
import { Wordmark } from "./components/Brand";
import { deleteMeeting, rescanLibrary, type MalformedFolder, type MeetingSummary, type SearchHit } from "./library";
import MeetingView from "./MeetingView";
import LibrarySearch from "./LibrarySearch";
import ConfirmDelete from "./ConfirmDelete";
import RecordingBar from "./RecordingBar";
import SettingsView from "./SettingsView";
import { formatElapsed, onLibraryChanged, onRecordingStatus, recordingStatus, type RecorderStatus } from "./recording";
import { cancelProcessing, onProcessingStatus, processingStageLabel, processingStatus, type ProcessingJob } from "./processing";

export default function App() {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [malformed, setMalformed] = useState<MalformedFolder[]>([]);
  const [failed, setFailed] = useState(false);
  const [scan, setScan] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [openAt, setOpenAt] = useState<Pick<SearchHit, "segment_id" | "revision" | "start_ms" | "end_ms"> | null>(null);
  const [deleting, setDeleting] = useState<MeetingSummary | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [jobs, setJobs] = useState<ProcessingJob[]>([]);
  const [recorder, setRecorder] = useState<RecorderStatus | null>(null);
  const [imports, setImports] = useState<ImportStatus>({ current: null, queued: 0, errors: [] });
  const [importHover, setImportHover] = useState(false);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    recordingStatus().then((status) => { if (active && !eventArrived) setRecorder(status); }, () => {});
    const listener = onRecordingStatus((status) => { if (active) { eventArrived = true; setRecorder(status); } });
    return () => { active = false; void listener.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    processingStatus().then((status) => { if (active && !eventArrived) setJobs(status.jobs); }, () => {});
    const listener = onProcessingStatus((status) => {
      if (active) { eventArrived = true; setJobs(status.jobs); }
    });
    return () => { active = false; void listener.then((stop) => stop(), () => {}); };
  }, []);

  useEffect(() => {
    let active = true;
    let eventArrived = false;
    importStatus().then((status) => { if (active && !eventArrived) setImports(status); }, () => {});
    const statusListener = onImportStatus((status) => { if (active) { eventArrived = true; setImports(status); } });
    const hoverListener = onImportHover((hover) => { if (active) setImportHover(hover); });
    return () => { active = false; void statusListener.then((stop) => stop(), () => {}); void hoverListener.then((stop) => stop(), () => {}); };
  }, []);

  async function pickFiles() {
    try { await pickAudioFiles(); }
    catch (reason) { setImports((current) => ({ ...current, errors: [...current.errors, String(reason)] })); }
  }

  async function dismissErrors() {
    try { await dismissImportErrors(); }
    catch (reason) { setImports((current) => ({ ...current, errors: [...current.errors, String(reason)] })); }
  }

  async function cancelJob(meetingId: string) {
    const status = await cancelProcessing(meetingId);
    setJobs(status.jobs);
  }

  async function confirmDelete() {
    if (!deleting || deleteBusy) return;
    setDeleteBusy(true);
    setDeleteError(null);
    try {
      await deleteMeeting(deleting.id);
      setDeleting(null);
      setScan((value) => value + 1);
    } catch (reason) { setDeleteError(String(reason)); }
    finally { setDeleteBusy(false); }
  }

  useEffect(() => {
    let active = true;
    setFailed(false);
    // Rebuild from Meeting folders on startup and whenever Rescan is requested.
    rescanLibrary().then(
      (report) => {
        if (active) {
          setMeetings(report.meetings);
          setMalformed(report.malformed);
        }
      },
      () => {
        if (active) setFailed(true);
      },
    );
    return () => { active = false; };
  }, [scan]);

  useEffect(() => {
    // A saved Recording or a new Meeting changes the Library list.
    const unlisten = onLibraryChanged(() => setScan((value) => value + 1));
    return () => { unlisten.then((stop) => stop(), () => {}); };
  }, []);

  return (
    <main>
      <header>
        <Wordmark className="wordmark" />
        <button type="button" onClick={() => { setSelectedId(null); setShowSettings(true); }}>Settings</button>
      </header>
      <RecordingBar />
      {showSettings ? <SettingsView onBack={() => setShowSettings(false)} /> : selectedId ? (
        <MeetingView id={selectedId} recordingMeetingId={recorder?.phase !== "idle" ? recorder?.meeting_id : null} openAt={openAt} onOpened={() => setOpenAt(null)} onDeleted={() => {
          setSelectedId(null); setOpenAt(null); setScan((value) => value + 1);
        }} onRenamed={setSelectedId} onOpenSettings={() => setShowSettings(true)} onBack={() => {
          setSelectedId(null);
          setOpenAt(null);
          setScan((value) => value + 1);
        }} />
      ) : (
        <>
          <h1>Library</h1>
          <button type="button" onClick={() => void pickFiles()}>Import audio</button>
          <button type="button" onClick={() => setScan((value) => value + 1)}>Rescan</button>
          <LibrarySearch onOpen={(hit) => { setSelectedId(hit.meeting_id); setOpenAt({ segment_id: hit.segment_id, revision: hit.revision, start_ms: hit.start_ms, end_ms: hit.end_ms }); }} />
          {deleteError && <p role="alert">{deleteError}</p>}
          {deleting && <ConfirmDelete title={deleting.title} busy={deleteBusy} onConfirm={() => void confirmDelete()} onCancel={() => setDeleting(null)} />}
          {imports.current && <p role="status">Importing {imports.current}…{imports.queued > 0 ? ` (+${imports.queued} queued)` : ""}</p>}
          {imports.errors.length > 0 && <div role="alert"><ul>{imports.errors.map((error, index) => <li key={`${index}:${error}`}>{error}</li>)}</ul><button type="button" onClick={() => void dismissErrors()}>Dismiss import errors</button></div>}
          {failed && <p role="alert">Could not load the Library</p>}
          {malformed.length > 0 && (
            <section role="status">
              <p>{malformed.length} {malformed.length === 1 ? "folder" : "folders"} could not be read</p>
              <ul>{malformed.map((item) => <li key={item.folder}>{item.folder}: {item.reason}</li>)}</ul>
            </section>
          )}
          {meetings?.length === 0 && <p>No meetings yet</p>}
          {meetings && meetings.length > 0 && (
            <ul>
              {meetings.map((meeting) => {
                const job = jobs.find((item) => item.meeting_id === meeting.id);
                return <li key={meeting.id}>
                  <time dateTime={meeting.started_at}>{meeting.started_at.slice(0, 16).replace("T", " ")}</time>{" "}
                  <button type="button" onClick={() => setSelectedId(meeting.id)}>{meeting.title}</button>
                  {meeting.duration_ms > 0 && <> · {formatElapsed(meeting.duration_ms)}</>}
                  {job && <> · <span>{processingStageLabel(job)}</span></>}
                  {job && job.stage !== "failed" && <> <button type="button" aria-label="Cancel processing" onClick={() => void cancelJob(meeting.id)}>Cancel</button></>}
                  {" "}<button type="button" aria-label={`Delete ${meeting.title}`} disabled={recorder?.phase !== "idle" && recorder?.meeting_id === meeting.id} onClick={() => { setDeleteError(null); setDeleting(meeting); }}>Delete</button>
                </li>;
              })}
            </ul>
          )}
        </>
      )}
      {importHover && <div className="import-overlay">Drop audio to import (mp3, m4a, wav, flac)</div>}
    </main>
  );
}

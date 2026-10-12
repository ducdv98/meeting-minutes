import { useEffect, useState } from "react";
import { dismissImportErrors, importStatus, onImportHover, onImportStatus, pickAudioFiles, type ImportStatus } from "./importing";
import wordmark from "./assets/wordmark-dark.png";
import { rescanLibrary, type MalformedFolder, type MeetingSummary } from "./library";
import MeetingView from "./MeetingView";
import RecordingBar from "./RecordingBar";
import SettingsView from "./SettingsView";
import { formatElapsed, onLibraryChanged } from "./recording";
import { cancelProcessing, onProcessingStatus, processingStageLabel, processingStatus, type ProcessingJob } from "./processing";

export default function App() {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [malformed, setMalformed] = useState<MalformedFolder[]>([]);
  const [failed, setFailed] = useState(false);
  const [scan, setScan] = useState(0);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [jobs, setJobs] = useState<ProcessingJob[]>([]);
  const [imports, setImports] = useState<ImportStatus>({ current: null, queued: 0, errors: [] });
  const [importHover, setImportHover] = useState(false);

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
        <img className="wordmark" src={wordmark} alt="Meeting Minutes" />
        <button type="button" onClick={() => { setSelectedId(null); setShowSettings(true); }}>Settings</button>
      </header>
      <RecordingBar />
      {showSettings ? <SettingsView onBack={() => setShowSettings(false)} /> : selectedId ? (
        <MeetingView id={selectedId} onRenamed={setSelectedId} onBack={() => {
          setSelectedId(null);
          setScan((value) => value + 1);
        }} />
      ) : (
        <>
          <h1>Library</h1>
          <button type="button" onClick={() => void pickFiles()}>Import audio</button>
          <button type="button" onClick={() => setScan((value) => value + 1)}>Rescan</button>
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

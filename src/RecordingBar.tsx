import { useEffect, useState } from "react";
import {
  acknowledgeRecordingNotice,
  formatElapsed,
  onRecordingNoticeRequested,
  onRecordingStatus,
  pauseHotkey,
  pauseRecording,
  recordHotkey,
  recordingStatus,
  resumeRecording,
  retrySave,
  startRecording,
  stopRecording,
  type RecorderStatus,
} from "./recording";

export default function RecordingBar() {
  const [status, setStatus] = useState<RecorderStatus | null>(null);
  const [showNotice, setShowNotice] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const unlisten = [
      onRecordingStatus((next) => { if (active) setStatus(next); }),
      onRecordingNoticeRequested(() => { if (active) setShowNotice(true); }),
    ];
    recordingStatus().then(
      (next) => { if (active) setStatus(next); },
      () => { if (active) setError("Could not read the recording state"); },
    );
    return () => {
      active = false;
      unlisten.forEach((promise) => promise.then((stop) => stop(), () => {}));
    };
  }, []);

  function run(action: () => Promise<RecorderStatus>) {
    setError(null);
    action().then(setStatus, (reason) => setError(String(reason)));
  }

  function record() {
    if (status && !status.notice_acknowledged) {
      setShowNotice(true);
      return;
    }
    run(startRecording);
  }

  function confirmNotice() {
    setShowNotice(false);
    run(async () => {
      await acknowledgeRecordingNotice();
      return startRecording();
    });
  }

  if (!status) {
    return error ? <p role="alert">{error}</p> : null;
  }

  const capturing = status.phase !== "idle";
  return (
    <section className="recording-bar" aria-label="Recording">
      {capturing ? (
        <p className={`recording-indicator ${status.phase}`} role="status" aria-live="polite">
          <span className="dot" aria-hidden="true" />
          {status.phase === "paused" ? "Paused" : "Recording"}{" "}
          <time>{formatElapsed(status.elapsed_ms)}</time>
        </p>
      ) : null}
      <div className="recording-controls">
        {!capturing && (
          <button type="button" onClick={record} title={recordHotkey}>Record</button>
        )}
        {status.phase === "recording" && (
          <button type="button" onClick={() => run(pauseRecording)} title={pauseHotkey}>Pause</button>
        )}
        {status.phase === "paused" && (
          <button type="button" onClick={() => run(resumeRecording)} title={pauseHotkey}>Resume</button>
        )}
        {capturing && (
          <button type="button" onClick={() => run(stopRecording)} title={recordHotkey}>Stop</button>
        )}
      </div>
      {error && <p role="alert">{error}</p>}
      {status.saves.map((save) => (
        <p key={save.meeting_id} className="pending-save">
          {save.state === "saving" ? (
            <>Saving {save.meeting_id}…</>
          ) : (
            <>
              Could not save {save.meeting_id}: {save.error}. The recorded audio is kept.{" "}
              <button type="button" onClick={() => run(() => retrySave(save.meeting_id))}>Retry save</button>
            </>
          )}
        </p>
      ))}
      {showNotice && (
        <div role="dialog" aria-modal="true" aria-labelledby="recording-notice-title" className="notice">
          <h2 id="recording-notice-title">Before you record</h2>
          <p>
            Other people on the call may need to agree to being recorded. Let them know before you start.
            While a Recording runs, the window and the tray icon show it.
          </p>
          <button type="button" onClick={confirmNotice}>Start recording</button>
          <button type="button" onClick={() => setShowNotice(false)}>Cancel</button>
        </div>
      )}
    </section>
  );
}

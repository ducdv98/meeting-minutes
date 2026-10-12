import { useCallback, useEffect, useRef, useState } from "react";
import { meetingDetail, renameMeeting, type MeetingDetail } from "./library";
import Player, { PlayerContext, type PlayerHandle } from "./Player";
import type { TimelinePlayer } from "./player";
import { formatElapsed } from "./recording";

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
  const playerHandle = useRef<PlayerHandle>(null);
  const onPlayerChange = useCallback((next: TimelinePlayer | null) => setPlayer(next), []);

  useEffect(() => {
    let active = true;
    setError(null);
    meetingDetail(id).then(
      (next) => { if (active) { setDetail(next); setAudioOpen(true); } },
      (reason) => { if (active) setError(String(reason)); },
    );
    return () => { active = false; };
  }, [id]);

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
            <p>Language: {languageName(detail.language)}</p>
            {detail.audio_path && audioOpen ? (
              <Player ref={playerHandle} audio_path={detail.audio_path} duration_ms={detail.duration_ms}
                markers={detail.markers} onPlayerChange={onPlayerChange} />
            ) : !detail.audio_path ? <p>No audio yet</p> : null}
          </>
        )}
      </section>
    </PlayerContext.Provider>
  );
}

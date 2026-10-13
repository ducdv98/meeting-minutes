import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play } from "lucide-react";
import { createContext, forwardRef, useContext, useEffect, useImperativeHandle, useRef, useState } from "react";
import type { Marker } from "./library";
import { createTimelinePlayer, markerLabel, markerPosition, type TimelinePlayer } from "./timeline";
import { formatElapsed } from "./recording";

export const PlayerContext = createContext<TimelinePlayer | null>(null);
export function usePlayer(): TimelinePlayer | null { return useContext(PlayerContext); }
export interface PlayerHandle { release(): void }
interface Props {
  audio_path: string;
  duration_ms: number;
  markers: Marker[];
  highlight?: { start_ms: number; end_ms: number } | null;
  onPlayerChange(player: TimelinePlayer | null): void;
  onPosition?(ms: number): void;
}

/** m:ss, or h:mm:ss when the Meeting runs an hour or more. */
function clock(ms: number, duration_ms: number): string {
  const total = Math.floor(ms / 1000);
  const hours = Math.floor(total / 3600), minutes = Math.floor((total % 3600) / 60), seconds = String(total % 60).padStart(2, "0");
  return duration_ms >= 3_600_000 ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}

const Player = forwardRef<PlayerHandle, Props>(function Player({ audio_path, duration_ms, markers, highlight, onPlayerChange, onPosition }, ref) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const playerRef = useRef<(TimelinePlayer & { dispose(): void }) | null>(null);
  const [position, setPosition] = useState(0);
  const [playing, setPlaying] = useState(false);
  useEffect(() => {
    const media = audioRef.current;
    if (!media) return;
    const player = createTimelinePlayer(media);
    playerRef.current = player;
    onPlayerChange(player);
    const updatePosition = () => { const ms = Math.round(media.currentTime * 1000); setPosition(ms); onPosition?.(ms); };
    const onPlay = () => setPlaying(true);
    const onPause = () => setPlaying(false);
    media.addEventListener("timeupdate", updatePosition);
    media.addEventListener("play", onPlay);
    media.addEventListener("pause", onPause);
    media.addEventListener("ended", onPause);
    return () => {
      player.dispose(); playerRef.current = null; onPlayerChange(null);
      media.removeEventListener("timeupdate", updatePosition);
      media.removeEventListener("play", onPlay);
      media.removeEventListener("pause", onPause);
      media.removeEventListener("ended", onPause);
    };
  }, [audio_path, onPlayerChange, onPosition]);
  useImperativeHandle(ref, () => ({ release() {
    const media = audioRef.current;
    if (!media) return;
    playerRef.current?.pause(); media.removeAttribute("src"); media.load(); onPlayerChange(null);
  } }), [onPlayerChange]);
  function seek(ms: number) { playerRef.current?.seek(ms); setPosition(ms); onPosition?.(ms); }
  const progress = markerPosition(position, duration_ms);
  const start = highlight ? markerPosition(highlight.start_ms, duration_ms) : 0;
  const end = highlight ? markerPosition(highlight.end_ms, duration_ms) : 0;
  return <section className="mx-7 mt-4 flex h-11 items-center gap-2.5 rounded-lg bg-panel pl-1.5 pr-4" aria-label="Timeline player">
    <audio ref={audioRef} preload="metadata" src={convertFileSrc(audio_path)} />
    <button type="button" aria-label={playing ? "Pause" : "Play"} className="grid size-8 shrink-0 place-items-center rounded-full text-ink transition-colors hover:bg-hover" onClick={() => playing ? playerRef.current?.pause() : playerRef.current?.play()}>
      {playing ? <Pause size={16} strokeWidth={0} fill="currentColor" /> : <Play size={16} strokeWidth={0} fill="currentColor" className="translate-x-px" />}
    </button>
    <span className="shrink-0 text-xs tabular-nums text-ink">{clock(position, duration_ms)}</span>
    <div className="group relative h-6 min-w-0 flex-1">
      <div className="absolute inset-x-0 top-1/2 h-1 -translate-y-1/2 overflow-hidden rounded-full bg-line-strong transition-[height] group-hover:h-1.5 group-has-[:focus-visible]:h-1.5">
        {highlight && <div data-testid="cited-span" className="absolute inset-y-0 bg-accent/35" style={{ left: `${start}%`, width: `${Math.max(0, end - start)}%` }} />}
        <div className="absolute inset-y-0 left-0 bg-accent" style={{ width: `${progress}%` }} />
      </div>
      <div className="pointer-events-none absolute top-1/2 size-3 -translate-1/2 rounded-full bg-accent opacity-0 shadow-[0_1px_3px_rgb(0_0_0/0.25)] transition-opacity group-hover:opacity-100 group-has-[:focus-visible]:opacity-100 group-has-[:focus-visible]:ring-4 group-has-[:focus-visible]:ring-accent/25" style={{ left: `${progress}%` }} />
      <input type="range" aria-label="Position" min={0} max={duration_ms} step={1000} value={Math.min(position, duration_ms)} onChange={(event) => seek(Number(event.target.value))} className="absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0" />
      {markers.map((marker, index) => {
        const label = `${markerLabel(marker)} at ${formatElapsed(marker.at_ms)}`;
        return <button key={index} type="button" data-kind={marker.kind} aria-label={label} title={label} className="absolute top-1/2 z-20 h-3 w-2 -translate-1/2 after:absolute after:inset-y-0 after:left-[3px] after:w-0.5 after:rounded-full after:bg-ink-faint hover:after:bg-ink" style={{ left: `${markerPosition(marker.at_ms, duration_ms)}%` }} onClick={() => seek(marker.at_ms)} />;
      })}
    </div>
    <span className="shrink-0 text-xs tabular-nums text-ink-muted">{clock(duration_ms, duration_ms)}</span>
  </section>;
});
export default Player;

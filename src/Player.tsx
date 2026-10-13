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
  return <section className="mx-7 mt-4 grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3.5 rounded-md border border-line bg-raised px-3.5 py-2.5" aria-label="Timeline player">
    <audio ref={audioRef} preload="metadata" src={convertFileSrc(audio_path)} />
    <button type="button" aria-label={playing ? "Pause" : "Play"} className="grid size-8 place-items-center rounded-full bg-ink text-canvas hover:opacity-85" onClick={() => playing ? playerRef.current?.pause() : playerRef.current?.play()}>
      {playing ? <Pause size={14} strokeWidth={1.5} fill="currentColor" /> : <Play size={14} strokeWidth={1.5} fill="currentColor" />}
    </button>
    <div className="relative h-7 min-w-0 focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-accent">
      <div className="absolute inset-x-0 top-[13px] h-[3px] rounded-sm bg-page-shadow" />
      {highlight && <div data-testid="cited-span" className="absolute top-[11px] h-[7px] rounded-sm bg-accent/35" style={{ left: `${start}%`, width: `${Math.max(0, end - start)}%` }} />}
      <div className="absolute top-[13px] h-[3px] rounded-sm bg-ink" style={{ width: `${progress}%` }} />
      <div className="pointer-events-none absolute top-2 size-[13px] -translate-x-1/2 rounded-full border-2 border-ink bg-raised" style={{ left: `${progress}%` }} />
      <input type="range" aria-label="Position" min={0} max={duration_ms} step={1000} value={Math.min(position, duration_ms)} onChange={(event) => seek(Number(event.target.value))} className="absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0" />
      {markers.map((marker, index) => {
        const label = `${markerLabel(marker)} at ${formatElapsed(marker.at_ms)}`;
        return <button key={index} type="button" data-kind={marker.kind} aria-label={label} title={label} className="absolute top-1.5 z-20 h-[17px] w-2 -translate-x-1/2 rounded-sm after:absolute after:left-[3px] after:top-0 after:h-full after:w-[2px] after:rounded-sm after:bg-ink-faint" style={{ left: `${markerPosition(marker.at_ms, duration_ms)}%` }} onClick={() => seek(marker.at_ms)} />;
      })}
    </div>
    <span className="whitespace-nowrap text-xs tabular-nums text-ink-muted">{formatElapsed(position)} / {formatElapsed(duration_ms)}</span>
  </section>;
});
export default Player;

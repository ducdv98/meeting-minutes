import { convertFileSrc } from "@tauri-apps/api/core";
import { createContext, forwardRef, useContext, useEffect, useImperativeHandle, useRef, useState } from "react";
import type { Marker } from "./library";
import { createTimelinePlayer, markerLabel, markerPosition, type TimelinePlayer } from "./player";
import { formatElapsed } from "./recording";

export const PlayerContext = createContext<TimelinePlayer | null>(null);
export function usePlayer(): TimelinePlayer | null { return useContext(PlayerContext); }

export interface PlayerHandle { release(): void }

interface Props {
  audio_path: string;
  duration_ms: number;
  markers: Marker[];
  onPlayerChange(player: TimelinePlayer | null): void;
  onPosition?(ms: number): void;
}

const Player = forwardRef<PlayerHandle, Props>(function Player({ audio_path, duration_ms, markers, onPlayerChange, onPosition }, ref) {
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
      player.dispose();
      playerRef.current = null;
      onPlayerChange(null);
      media.removeEventListener("timeupdate", updatePosition);
      media.removeEventListener("play", onPlay);
      media.removeEventListener("pause", onPause);
      media.removeEventListener("ended", onPause);
    };
  }, [audio_path, onPlayerChange, onPosition]);

  useImperativeHandle(ref, () => ({
    release() {
      const media = audioRef.current;
      if (!media) return;
      playerRef.current?.pause();
      media.removeAttribute("src");
      media.load();
      onPlayerChange(null);
    },
  }), [onPlayerChange]);

  function seek(ms: number) {
    playerRef.current?.seek(ms);
    setPosition(ms);
    onPosition?.(ms);
  }

  return (
    <section className="player" aria-label="Timeline player">
      <audio ref={audioRef} preload="metadata" src={convertFileSrc(audio_path)} />
      <button type="button" onClick={() => playing ? playerRef.current?.pause() : playerRef.current?.play()}>
        {playing ? "Pause" : "Play"}
      </button>
      <span>{formatElapsed(position)} / {formatElapsed(duration_ms)}</span>
      <div className="scrubber">
        <input type="range" aria-label="Position" min={0} max={duration_ms} step={1000} value={Math.min(position, duration_ms)} onChange={(event) => seek(Number(event.target.value))} />
        <div className="marker-layer">
          {markers.map((marker, index) => {
            const label = `${markerLabel(marker)} at ${formatElapsed(marker.at_ms)}`;
            return <button key={index} type="button" data-kind={marker.kind} aria-label={label} title={label}
              style={{ left: `${markerPosition(marker.at_ms, duration_ms)}%` }} onClick={() => seek(marker.at_ms)} />;
          })}
        </div>
      </div>
    </section>
  );
});

export default Player;

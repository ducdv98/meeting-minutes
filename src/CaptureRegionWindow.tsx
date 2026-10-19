import { useEffect, useRef, useState, type PointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

type Point = { x: number; y: number };

export function CaptureRegionWindow() {
  const start = useRef<Point | null>(null);
  const [end, setEnd] = useState<Point | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const cancel = (event: KeyboardEvent) => {
      if (event.key === "Escape") void invoke("cancel_capture_region");
    };
    window.addEventListener("keydown", cancel);
    return () => window.removeEventListener("keydown", cancel);
  }, []);

  function pointerDown(event: PointerEvent<HTMLDivElement>) {
    if (busy) return;
    const point = { x: event.clientX, y: event.clientY };
    start.current = point;
    setEnd(point);
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function pointerMove(event: PointerEvent<HTMLDivElement>) {
    if (start.current) setEnd({ x: event.clientX, y: event.clientY });
  }

  async function pointerUp(event: PointerEvent<HTMLDivElement>) {
    const first = start.current;
    start.current = null;
    if (!first || busy) return;
    const x = Math.min(first.x, event.clientX);
    const y = Math.min(first.y, event.clientY);
    const width = Math.abs(event.clientX - first.x);
    const height = Math.abs(event.clientY - first.y);
    if (width < 2 || height < 2) {
      setEnd(null);
      return;
    }
    setBusy(true);
    try {
      await invoke("select_capture_region", { x, y, width, height });
    } catch (reason) {
      setError(String(reason));
      setBusy(false);
    }
  }

  const first = start.current;
  const selection = first && end ? {
    left: Math.min(first.x, end.x),
    top: Math.min(first.y, end.y),
    width: Math.abs(first.x - end.x),
    height: Math.abs(first.y - end.y),
  } : null;

  return <div onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={(event) => void pointerUp(event)}
    style={{ position: "fixed", inset: 0, background: selection ? "transparent" : "rgba(8, 15, 25, 0.12)", cursor: "crosshair", touchAction: "none", userSelect: "none" }}>
    <div style={{ position: "absolute", left: 20, top: 20, padding: "10px 14px", borderRadius: 8, color: "white", background: "rgba(23, 34, 52, 0.92)", font: "14px system-ui" }}>
      Drag to draw a Capture region · Esc to cancel
    </div>
    {selection && <div aria-hidden="true" style={{ position: "absolute", ...selection, border: "2px solid #fff", boxShadow: "0 0 0 1px #20324b, 0 0 0 9999px rgba(8, 15, 25, 0.35)", pointerEvents: "none" }} />}
    {error && <p role="alert" style={{ position: "absolute", left: 20, bottom: 20, padding: 12, background: "#792d2d", color: "white" }}>{error}</p>}
  </div>;
}

export function SnapshotConfirmationWindow() {
  return <div role="status" style={{ display: "grid", placeItems: "center", height: "100vh", background: "#172234", color: "white", borderRadius: 10, font: "600 15px system-ui" }}>Snapshot saved</div>;
}

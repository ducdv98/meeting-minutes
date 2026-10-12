import type { TimelinePlayer } from "./timeline";
import { formatElapsed } from "./recording";
import type { Citation } from "./summary";
export default function CitationLink({ citation, player }: { citation: Citation; player: TimelinePlayer | null }) {
  const elapsed = formatElapsed(citation.start_ms);
  const label = elapsed.startsWith("00:") ? elapsed.slice(3) : elapsed;
  return <button type="button" aria-label={`Play from ${label}`} disabled={!player} onClick={() => player?.playSpan(citation.start_ms, citation.end_ms)}>[{label}]</button>;
}

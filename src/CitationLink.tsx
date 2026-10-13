import type { TimelinePlayer } from "./timeline";
import { formatElapsed } from "./recording";
import type { Citation } from "./summary";
import { Citation as CitationButton } from "./components/ui/Citation";
export default function CitationLink({ citation, player, segmentText, onPlay }: { citation: Citation; player: TimelinePlayer | null; segmentText?: string; onPlay?: (citation: Citation) => void }) {
  const elapsed = formatElapsed(citation.start_ms);
  const label = elapsed.startsWith("00:") ? elapsed.slice(3) : elapsed;
  return <CitationButton time={label} segmentText={segmentText} disabled={!player} onClick={() => { onPlay?.(citation); player?.playSpan(citation.start_ms, citation.end_ms); }} />;
}

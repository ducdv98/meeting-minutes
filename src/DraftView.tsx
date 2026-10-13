import type { DraftSegment, Speaker } from "./processing";
import { formatElapsed } from "./recording";
import { labelParticipantName, speakerName } from "./transcript";

interface Props {
  segments: DraftSegment[];
  participants: Partial<Record<Speaker, string>>;
  yourName: string | null;
  isVoiceNote: boolean;
}

export default function DraftView({ segments, participants, yourName, isVoiceNote }: Props) {
  return <ol aria-label="Draft Segments" className="m-0 list-none p-0">{segments.map((segment, index) => {
    const elapsed = formatElapsed(segment.start_ms);
    const time = elapsed.startsWith("00:") ? elapsed.slice(3) : elapsed;
    const name = labelParticipantName(segment.speaker, participants, yourName, isVoiceNote);
    return <li key={index} className="-mx-2 grid grid-cols-[18px_52px_68px_minmax(0,1fr)] gap-3 rounded-sm px-2 py-2">
      <span aria-hidden="true" />
      <span className="pt-[3px] text-xs tabular-nums text-ink-faint">{time}</span>
      <span title={`Speaker label: ${speakerName(segment.speaker)}`} className="pt-[3px] text-xs font-medium text-ink-muted">{name}</span>
      <p className="m-0 min-w-0 text-read leading-[26px]">{segment.text}</p>
    </li>;
  })}</ol>;
}

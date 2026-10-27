import { useEffect, useRef, useState } from "react";
import { Search } from "lucide-react";
import { searchLibrary, snippetParts, type SearchHit } from "./library";
import { formatElapsed, onLibraryChanged } from "./recording";
import { EmptyState } from "./components/states/EmptyState";
import { ErrorState } from "./components/states/ErrorState";
import { Loading } from "./components/states/Loading";
import { Input } from "./components/ui/Input";
import { Button } from "./components/ui/Button";

interface Props {
  onOpen(hit: SearchHit): void;
  onAsk?(question: string): void;
  query?: string;
  onQueryChange?(query: string): void;
}

function dateLabel(value: string) {
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value));
}

export default function LibrarySearch({ onOpen, onAsk, query: controlledQuery, onQueryChange }: Props) {
  const [localQuery, setLocalQuery] = useState("");
  const query = controlledQuery ?? localQuery;
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  function changeQuery(next: string) {
    if (onQueryChange) onQueryChange(next);
    else setLocalQuery(next);
  }

  function runSearch(value: string) {
    if (!value.trim()) return;
    if (timer.current) clearTimeout(timer.current);
    const token = ++request.current;
    setSearched(false);
    setError(null);
    void searchLibrary(value).then(
      (next) => { if (request.current === token) { setHits(next); setSearched(true); setError(null); } },
      (reason) => { if (request.current === token) { setHits([]); setSearched(true); setError(String(reason)); } },
    );
  }

  useEffect(() => {
    const token = ++request.current;
    if (!query.trim()) {
      setHits([]);
      setSearched(false);
      setError(null);
      return;
    }
    setSearched(false);
    setHits([]);
    setError(null);
    timer.current = setTimeout(() => {
      void searchLibrary(query).then(
        (next) => { if (request.current === token) { setHits(next); setSearched(true); setError(null); } },
        (reason) => { if (request.current === token) { setHits([]); setSearched(true); setError(String(reason)); } },
      );
    }, 250);
    return () => { if (timer.current) clearTimeout(timer.current); };
  }, [query]);

  useEffect(() => {
    const unlisten = onLibraryChanged(() => { if (query.trim()) runSearch(query); });
    return () => { void unlisten.then((stop) => stop(), () => {}); };
    // Refresh whenever the active query changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query]);

  return <section aria-label="Library search" className="flex min-h-0 flex-1 flex-col">
    <div className="mx-7 mt-5 flex h-10 items-center gap-2.5 rounded-md border border-line-strong bg-raised px-3">
      <Search size={16} strokeWidth={1.5} className="shrink-0 text-ink-muted" aria-hidden="true" />
      <Input type="search" aria-label={controlledQuery === undefined ? "Search transcripts" : "Search the Library"} value={query} onChange={(event) => changeQuery(event.target.value)}
        onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); runSearch(query); } }}
        className="h-full border-0 bg-transparent px-0 text-read shadow-none focus:border-0 focus:shadow-none" />
      <Button variant="secondary" aria-label="Library Ask" onClick={() => onAsk?.(query)}>Ask</Button>
    </div>
    {!query.trim() && <div className="grid flex-1 place-items-center p-10"><EmptyState title="Search the Library" description="Find a moment in any Transcript." /></div>}
    {query.trim() && !searched && !error && <div className="px-7"><Loading message="Searching the Library…" rows={2} /></div>}
    {error && <div className="px-7"><ErrorState title="Couldn't search the Library" guidance={`${error}. Try again, or Rescan the Library.`} onRetry={() => runSearch(query)} /></div>}
    {query.trim() && searched && !error && hits.length === 0 && <div className="grid flex-1 place-items-center p-10"><EmptyState title="No results" description="Try a different word from a Transcript." /></div>}
    {query.trim() && hits.length > 0 && <div className="min-h-0 overflow-y-auto px-7 pb-7 pt-2">
      <p className="m-0 py-2 text-xs text-ink-muted" role="status">{hits.length} {hits.length === 1 ? "result" : "results"} in {new Set(hits.map((hit) => hit.meeting_id)).size} {new Set(hits.map((hit) => hit.meeting_id)).size === 1 ? "Meeting" : "Meetings"}</p>
      <ul aria-label="Search results" className="m-0 list-none p-0">{hits.map((hit) => <li key={`${hit.meeting_id}:${hit.segment_id}`}>
        <button type="button" className="grid w-full gap-1 border-b border-line py-3 text-left hover:bg-hover" onClick={() => onOpen(hit)}>
          <span className="flex flex-wrap items-center gap-2 text-sm font-medium">{hit.title}<span className="text-xs font-normal tabular-nums text-ink-faint">{dateLabel(hit.started_at)} · {formatElapsed(hit.start_ms)}</span></span>
          <span className="text-base text-ink-muted">{snippetParts(hit.snippet).map((part, index) => part.match ? <mark key={index} className="rounded-sm bg-accent-soft px-0.5 text-ink">{part.text}</mark> : <span key={index}>{part.text}</span>)}</span>
        </button>
      </li>)}</ul>
    </div>}
  </section>;
}

import { useEffect, useRef, useState } from "react";
import { searchLibrary, snippetParts, type SearchHit } from "./library";
import { formatElapsed, onLibraryChanged } from "./recording";

interface Props { onOpen(hit: SearchHit): void; }

export default function LibrarySearch({ onOpen }: Props) {
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  function searchNow() {
    if (!query.trim()) return;
    if (timer.current) clearTimeout(timer.current);
    const token = ++request.current;
    void searchLibrary(query).then(
      (next) => { if (request.current === token) { setHits(next); setSearched(true); setError(null); } },
      (reason) => { if (request.current === token) { setHits([]); setSearched(true); setError(String(reason)); } },
    );
  }

  useEffect(() => {
    const unlisten = onLibraryChanged(() => { if (query.trim()) searchNow(); });
    return () => { void unlisten.then((stop) => stop(), () => {}); };
  }, [query]);

  return <section aria-label="Library search">
    <input type="search" aria-label="Search transcripts" value={query} onChange={(event) => setQuery(event.target.value)}
      onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); searchNow(); } }} />
    {error && <p role="alert">{error}</p>}
    {query.trim() && searched && !error && hits.length === 0 && <p>No matches</p>}
    {query.trim() && hits.length > 0 && <ul aria-label="Search results">{hits.map((hit) => <li key={`${hit.meeting_id}:${hit.segment_id}`}>
      <button type="button" onClick={() => onOpen(hit)}>
        <strong>{hit.title}</strong>{" "}
        <time dateTime={hit.started_at}>{hit.started_at.slice(0, 16).replace("T", " ")}</time>{" "}
        <span>{formatElapsed(hit.start_ms)}</span>{" "}
        <span>{snippetParts(hit.snippet).map((part, index) => part.match ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>)}</span>
      </button>
    </li>)}</ul>}
  </section>;
}

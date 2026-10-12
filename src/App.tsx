import { useEffect, useState } from "react";
import wordmark from "./assets/wordmark-dark.png";
import { rescanLibrary, type MalformedFolder, type MeetingSummary } from "./library";

export default function App() {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [malformed, setMalformed] = useState<MalformedFolder[]>([]);
  const [failed, setFailed] = useState(false);
  const [scan, setScan] = useState(0);

  useEffect(() => {
    let active = true;
    setFailed(false);
    // Rebuild from Meeting folders on startup and whenever Rescan is requested.
    rescanLibrary().then(
      (report) => {
        if (active) {
          setMeetings(report.meetings);
          setMalformed(report.malformed);
        }
      },
      () => {
        if (active) setFailed(true);
      },
    );
    return () => { active = false; };
  }, [scan]);

  return (
    <main>
      <header>
        <img className="wordmark" src={wordmark} alt="Meeting Minutes" />
      </header>
      <h1>Library</h1>
      <button type="button" onClick={() => setScan((value) => value + 1)}>Rescan</button>
      {failed && <p role="alert">Could not load the Library</p>}
      {malformed.length > 0 && (
        <section role="status">
          <p>{malformed.length} {malformed.length === 1 ? "folder" : "folders"} could not be read</p>
          <ul>{malformed.map((item) => <li key={item.folder}>{item.folder}: {item.reason}</li>)}</ul>
        </section>
      )}
      {meetings?.length === 0 && <p>No meetings yet</p>}
      {meetings && meetings.length > 0 && (
        <ul>
          {meetings.map((meeting) => (
            <li key={meeting.id}>
              <time dateTime={meeting.started_at}>{meeting.started_at.slice(0, 16).replace("T", " ")}</time> {meeting.title}
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

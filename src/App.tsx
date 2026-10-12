import { useEffect, useState } from "react";
import wordmark from "./assets/wordmark-dark.png";
import { listMeetings, type MeetingSummary } from "./library";

export default function App() {
  const [meetings, setMeetings] = useState<MeetingSummary[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    listMeetings().then(
      (items) => {
        if (active) setMeetings(items);
      },
      () => {
        if (active) setFailed(true);
      },
    );
    return () => { active = false; };
  }, []);

  return (
    <main>
      <header>
        <img className="wordmark" src={wordmark} alt="Meeting Minutes" />
      </header>
      <h1>Library</h1>
      {failed && <p role="alert">Could not load the Library</p>}
      {meetings?.length === 0 && <p>No meetings yet</p>}
      {meetings && meetings.length > 0 && (
        <ul>
          {meetings.map((meeting) => <li key={meeting.id}>{meeting.title}</li>)}
        </ul>
      )}
    </main>
  );
}

# Meeting folders are named by start time only; the Title lives in meeting.json

Renaming a Meeting used to move its folder, because the folder name was the start time plus the Title. Windows cannot move a folder while a file inside it is open, so renames were refused during Processing, Summary, Ask, recording and playback. New Meeting folders are now named only by their start time (`2026-10-13 0930`, with ` (2)` added when two Meetings start in the same minute). The Title is stored only in `meeting.json`. A rename is then a metadata write that can happen at any time, and the Meeting ID (the folder name) never changes.

## Considered Options

- **Keep the Title in the folder name and rename the folder once the Meeting is idle**: the folder stays readable, but the folder name lags behind the Title and a pending rename has to survive crashes. Rejected as too complex.
- **Freeze the first Title in the folder name**: recorded Meetings start as "Untitled meeting", so most folders would be named wrongly forever.

## Consequences

- Browsing the Library folder shows dates, not Titles. `transcript.md` and `summary.md` inside each folder still carry the Title.
- Existing folders that already contain a Title are left as they are; the app reads the Title only from `meeting.json`.

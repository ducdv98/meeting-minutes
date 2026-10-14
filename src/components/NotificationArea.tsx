import type { MalformedFolder } from "../library";
import { Button } from "./ui/Button";
import { ErrorState } from "./states/ErrorState";
import { Loading } from "./states/Loading";

interface Props {
  importing: string | null;
  queued: number;
  importErrors: string[];
  deleteError: string | null;
  malformed: MalformedFolder[];
  onDismissImportErrors(): void;
  onDismissDeleteError(): void;
  onDismissMalformed(): void;
}

export function NotificationArea({ importing, queued, importErrors, deleteError, malformed, onDismissImportErrors, onDismissDeleteError, onDismissMalformed }: Props) {
  if (!importing && !importErrors.length && !deleteError && !malformed.length) return null;
  return <div role="region" aria-label="Notifications" className="grid shrink-0 gap-2 px-7 pb-3">
    {importing && <div className="rounded-md bg-panel px-3 py-2 text-sm text-ink-muted"><Loading message={`Importing ${importing}…${queued > 0 ? ` (+${queued} queued)` : ""}`} rows={1} /><p aria-hidden="true" className="m-0">Importing {importing}…{queued > 0 ? ` (+${queued} queued)` : ""}</p></div>}
    {importErrors.length > 0 && <div className="rounded-md bg-panel px-3 pb-3"><ErrorState title="Some files couldn't be imported" guidance={`${importErrors.join(" · ")}. Choose an mp3, m4a, wav or flac file and try again.`} /><Button variant="secondary" onClick={onDismissImportErrors}>Dismiss import errors</Button></div>}
    {deleteError && <div className="rounded-md bg-panel px-3 pb-3"><ErrorState title="Couldn't delete the Meeting" guidance={`${deleteError}. Try again or Rescan the Library.`} /><Button variant="secondary" onClick={onDismissDeleteError}>Dismiss delete error</Button></div>}
    {malformed.length > 0 && <div role="status" className="rounded-md bg-panel px-3 py-2 text-sm text-ink-muted"><p className="m-0">{malformed.length} {malformed.length === 1 ? "folder" : "folders"} could not be read</p><ul className="m-0 pl-5">{malformed.map((item) => <li key={item.folder}>{item.folder}: {item.reason}</li>)}</ul><Button variant="ghost" className="mt-2" onClick={onDismissMalformed}>Dismiss unreadable folders</Button></div>}
  </div>;
}

import { Button } from "./components/ui/Button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "./components/ui/Dialog";

interface Props {
  title: string;
  onConfirm(): void;
  onCancel(): void;
  busy: boolean;
  error?: string | null;
}

export default function ConfirmDelete({ title, onConfirm, onCancel, busy, error }: Props) {
  return <Dialog open onOpenChange={(open) => { if (!open && !busy) onCancel(); }}>
    <DialogContent onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (busy) event.preventDefault(); }}>
      <DialogTitle>Delete {title}?</DialogTitle>
      <DialogDescription>The Meeting folder moves to the Recycle Bin. You can restore it from there and Rescan to bring it back.</DialogDescription>
      {error && <p role="alert" className="mt-3 text-sm text-record">{error}</p>}
      <div className="mt-6 flex justify-end gap-2">
        <Button variant="secondary" disabled={busy} onClick={onCancel}>Cancel</Button>
        <Button variant="record" disabled={busy} onClick={onConfirm}>Delete</Button>
      </div>
    </DialogContent>
  </Dialog>;
}

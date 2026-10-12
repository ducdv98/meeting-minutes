interface Props {
  title: string;
  onConfirm(): void;
  onCancel(): void;
  busy: boolean;
}

export default function ConfirmDelete({ title, onConfirm, onCancel, busy }: Props) {
  return <div role="dialog" aria-label={`Delete ${title}?`}>
    <p>The Meeting folder moves to the Recycle Bin. You can restore it from there and Rescan to bring it back.</p>
    <button type="button" disabled={busy} onClick={onConfirm}>Delete</button>
    <button type="button" disabled={busy} onClick={onCancel}>Cancel</button>
  </div>;
}

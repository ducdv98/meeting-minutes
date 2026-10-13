# Delete a Meeting to the Recycle Bin

Status: resolved
Blocked by: 04
Spec: ../spec.md

## What

Delete a Meeting from its view or the list with a confirm dialog: move its folder to the Windows Recycle Bin and remove it from the index and search. Not allowed while it is recording.

## Acceptance criteria

- [x] Deleted Meeting folder is in the Recycle Bin and restorable; after restoring it and rescanning, it is back in the Library.
- [x] Deleting the Meeting that is currently recording is blocked.

## Comments

### 2026-10-12: implemented; laptop check pending

Design: `../15-core-design.md`. `Library::delete_meeting` refuses unsaved Recordings and takes the trash function as a parameter. In the app that function is `recycle_bin::move_to_recycle_bin` (`SHFileOperationW` with `FOF_ALLOWUNDO`). The Tauri command refuses a Meeting that is recording or saving, being processed, or busy with a Summary or Ask. It can be used from the list or from the Meeting view, after a confirm dialog; the Meeting view releases the audio player first. The restore + Rescan path is tested with a fake bin. The real Recycle Bin is not verified yet.

Manual check on the laptop: delete a Meeting and check it is in the Recycle Bin. Restore it, click Rescan, and it should be back and searchable. Try to delete the Meeting that is recording; it should be refused.

### 2026-10-13: laptop check passed

The owner ran the manual checks on the laptop and all passed. Resolved.

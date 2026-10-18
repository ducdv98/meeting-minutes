# Snapshots are manual, local images on the Timeline

During a Recording the user can take a **Snapshot**: an image of a **Capture region** of their screen, for example a shared slide or demo. Each Snapshot is a PNG file in the Meeting's folder with a Timeline position and an optional caption, shown inline in the Transcript. The Library stays the source of truth: the files and their positions can be read without the app's database, which is only a rebuildable index.

- Snapshots are taken by hand only, from the **Capture bar** or a global hotkey, and only while a Recording runs (including while paused, where they sit at the pause Marker). Each is written durably when taken, like a Chunk.
- The user draws the Capture region; later Snapshots in the Recording reuse it until the user draws a new one.
- The Capture bar and the region overlay are excluded from capture and from screen sharing.
- Snapshots never leave the PC. Summary, Ask and Library search over Transcript text do not read the images, and a Snapshot is never a Citation. A caption is searchable text.
- Images are PNG at native resolution, so slide and code text stays readable.

## Considered Options

- **Automatic capture on a timer or on screen change:** catches more, but stores far more images and captures private content without the user acting.
- **Send Snapshots to a vision model for OCR or descriptions:** would let Summary and Ask use shared content, but it sends images to the cloud and needs a new consent decision and a Citation rule for images. Deferred to a later ADR.
- **Treat a Snapshot as a kind of Marker:** Markers record events, and a Snapshot is user content.

## Consequences

- The Meeting folder layout gains a Snapshots folder that later versions must keep readable.
- A later vision feature needs its own ADR on consent and on how an image can be cited.

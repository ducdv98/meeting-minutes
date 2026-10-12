mod index;
mod library;
mod meeting_file;
mod naming;

pub use library::{Library, LibraryError, MalformedFolder, ScanReport};
pub use meeting_file::MeetingSummary;
pub use naming::folder_name;
pub use recording::{finalize_recording, FinalizedRecording};
pub mod capture;
pub mod recorder;
pub mod recording;
pub mod settings;

mod index;
mod library;
mod meeting_file;
mod naming;

pub use library::{Library, LibraryError, MalformedFolder, OrphanRecording, ScanReport};
pub use meeting_file::{MeetingDetail, MeetingSummary};
pub use naming::folder_name;
pub use recording::{finalize_recording, FinalizedRecording};
pub mod audio_prep;
pub mod capture;
pub mod import;
pub mod models;
pub mod processing;
pub mod recorder;
pub mod recording;
pub mod settings;
pub mod timeline;
pub mod transcript;

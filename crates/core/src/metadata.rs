use std::{
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
    sync::{Mutex, OnceLock},
};

use serde_json::Value;

use crate::library::LibraryError;

static METADATA_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) fn update_metadata(
    folder: &Path,
    update: impl FnOnce(&mut Value) -> Result<(), LibraryError>,
) -> Result<(), LibraryError> {
    update_metadata_and(folder, update, || Ok(()))
}

pub(crate) fn update_metadata_and(
    folder: &Path,
    update: impl FnOnce(&mut Value) -> Result<(), LibraryError>,
    after: impl FnOnce() -> Result<(), LibraryError>,
) -> Result<(), LibraryError> {
    let _guard = METADATA_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| LibraryError::Encode("Meeting metadata lock was poisoned".into()))?;
    let mut metadata: Value = serde_json::from_slice(&fs::read(folder.join("meeting.json"))?)?;
    update(&mut metadata)?;
    let tmp = folder.join("meeting.json.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    serde_json::to_writer_pretty(&mut file, &metadata)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(tmp, folder.join("meeting.json"))?;
    after()
}

pub(crate) fn with_current_title<T>(
    folder: &Path,
    write: impl FnOnce(&str) -> io::Result<T>,
) -> io::Result<T> {
    let _guard = METADATA_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| io::Error::other("Meeting metadata lock was poisoned"))?;
    let metadata: Value = serde_json::from_slice(&fs::read(folder.join("meeting.json"))?)?;
    let title = metadata["title"]
        .as_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing Meeting title"))?;
    write(title)
}

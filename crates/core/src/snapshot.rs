//! Local Snapshot files for a Meeting. Sidecars are the rebuildable index.

use crate::{
    capture::Clock,
    meeting_file::{MeetingFile, RecordingEntry},
    recording::CaptureSession,
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::fs::File;
use std::{
    fmt, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// Screen coordinates are chosen by the app layer; the grabber returns native pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub trait ScreenGrabber {
    fn grab(&self, region: CaptureRegion) -> Result<RgbaFrame, String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub png_path: PathBuf,
    pub timeline_ms: u64,
    pub order: u32,
    pub caption: Option<String>,
}

pub struct LoadedSnapshot {
    pub snapshot: Snapshot,
    pub png: Vec<u8>,
}

#[derive(Debug)]
pub enum SnapshotError {
    Io(io::Error),
    Json(serde_json::Error),
    Grab(String),
    Png(String),
    InvalidPixels,
    NoRecording,
    ImportedMeeting,
    ForeignRecording,
    NotFound,
    OrderOverflow,
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Snapshot I/O error: {e}"),
            Self::Json(e) => write!(f, "Snapshot metadata error: {e}"),
            Self::Grab(e) => write!(f, "Screen grab failed: {e}"),
            Self::Png(e) => write!(f, "PNG encoding failed: {e}"),
            Self::InvalidPixels => write!(f, "Screen grab returned invalid RGBA pixels"),
            Self::NoRecording => write!(f, "A Recording must be running to take a Snapshot"),
            Self::ImportedMeeting => write!(f, "An Import cannot have Snapshots"),
            Self::ForeignRecording => write!(f, "The Recording belongs to another Meeting"),
            Self::NotFound => write!(f, "Snapshot not found"),
            Self::OrderOverflow => write!(f, "Snapshot order has overflowed"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<io::Error> for SnapshotError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for SnapshotError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Serialize, Deserialize)]
struct Sidecar {
    timeline_ms: u64,
    order: u32,
    caption: Option<String>,
}

pub struct SnapshotStore {
    meeting_dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(meeting_dir: impl Into<PathBuf>) -> Self {
        Self {
            meeting_dir: meeting_dir.into(),
        }
    }

    /// Captures at the current Meeting Timeline position, including during a pause.
    pub fn take<C: Clock, G: ScreenGrabber>(
        &self,
        recording: Option<&CaptureSession<C>>,
        region: CaptureRegion,
        grabber: &G,
    ) -> Result<Snapshot, SnapshotError> {
        let recording = recording.ok_or(SnapshotError::NoRecording)?;
        let meeting: MeetingFile =
            serde_json::from_slice(&fs::read(self.meeting_dir.join("meeting.json"))?)?;
        if meeting.import.is_some() {
            return Err(SnapshotError::ImportedMeeting);
        }
        let (number, chunk_dir) = recording.snapshot_recording();
        if chunk_dir != self.meeting_dir.join(format!(".chunks/recording-{number}")) {
            return Err(SnapshotError::ForeignRecording);
        }
        let timeline_start_ms: u64 = meeting
            .recordings
            .iter()
            .filter_map(|entry| match entry {
                RecordingEntry::Known(value) if value.number < number => Some(value.duration_ms),
                _ => None,
            })
            .sum();
        let timeline_ms = timeline_start_ms + recording.elapsed_ms();
        let frame = grabber.grab(region).map_err(SnapshotError::Grab)?;
        let expected = (frame.width as usize)
            .checked_mul(frame.height as usize)
            .and_then(|count| count.checked_mul(4))
            .ok_or(SnapshotError::InvalidPixels)?;
        if frame.width == 0 || frame.height == 0 || frame.pixels.len() != expected {
            return Err(SnapshotError::InvalidPixels);
        }
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, frame.width, frame.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| SnapshotError::Png(e.to_string()))?;
            writer
                .write_image_data(&frame.pixels)
                .map_err(|e| SnapshotError::Png(e.to_string()))?;
        }

        let dir = self.meeting_dir.join("snapshots");
        fs::create_dir_all(&dir)?;
        sync_directory(&self.meeting_dir)?;
        let mut order = self.list()?.last().map_or(Ok(1), |last| {
            last.order
                .checked_add(1)
                .ok_or(SnapshotError::OrderOverflow)
        })?;
        let png_path = loop {
            let stem = format!(
                "{order:04}-{:02}-{:02}-{:02}",
                timeline_ms / 3_600_000,
                timeline_ms / 60_000 % 60,
                timeline_ms / 1_000 % 60
            );
            let path = dir.join(format!("{stem}.png"));
            if !path.exists() && !path.with_extension("json").exists() {
                break path;
            }
            order = order.checked_add(1).ok_or(SnapshotError::OrderOverflow)?;
        };
        let sidecar = Sidecar {
            timeline_ms,
            order,
            caption: None,
        };
        write_durable(&png_path, &png)?;
        let json_path = png_path.with_extension("json");
        let mut json = serde_json::to_vec_pretty(&sidecar)?;
        json.push(b'\n');
        write_durable(&json_path, &json)?;
        Ok(Snapshot {
            png_path,
            timeline_ms,
            order,
            caption: None,
        })
    }

    /// Rebuilds the Snapshot index from image and sidecar pairs on disk.
    pub fn list(&self) -> Result<Vec<Snapshot>, SnapshotError> {
        let dir = self.meeting_dir.join("snapshots");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let png_path = path.with_extension("png");
            if !png_path.is_file() {
                continue;
            }
            let sidecar: Sidecar = serde_json::from_slice(&fs::read(path)?)?;
            snapshots.push(Snapshot {
                png_path,
                timeline_ms: sidecar.timeline_ms,
                order: sidecar.order,
                caption: sidecar.caption,
            });
        }
        snapshots.sort_by_key(|snapshot| snapshot.order);
        Ok(snapshots)
    }

    pub fn load(&self, order: u32) -> Result<LoadedSnapshot, SnapshotError> {
        let snapshot = self
            .list()?
            .into_iter()
            .find(|snapshot| snapshot.order == order)
            .ok_or(SnapshotError::NotFound)?;
        let png = fs::read(&snapshot.png_path)?;
        Ok(LoadedSnapshot { snapshot, png })
    }

    /// Disk use of complete image and sidecar pairs in this Meeting.
    pub fn total_size_bytes(&self) -> Result<u64, SnapshotError> {
        self.list()?.into_iter().try_fold(0_u64, |total, snapshot| {
            let image = fs::metadata(&snapshot.png_path)?.len();
            let sidecar = fs::metadata(snapshot.png_path.with_extension("json"))?.len();
            Ok(total.saturating_add(image).saturating_add(sidecar))
        })
    }
}

fn write_durable(path: &Path, bytes: &[u8]) -> Result<(), SnapshotError> {
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension().unwrap().to_string_lossy()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        sync_directory(path.parent().unwrap())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

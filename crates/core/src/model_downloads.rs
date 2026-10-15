use crate::{
    models::{self, ModelInfo, MODELS, VAD_MODEL},
    processing::ProcessingError,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
pub struct ModelDownloadStatus {
    pub model_id: String,
    pub state: DownloadState,
    pub progress: u32,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    NotDownloaded,
    Downloading,
    Downloaded,
    Failed,
    Cancelled,
}

struct Entry {
    state: DownloadState,
    progress: u32,
    error: Option<String>,
    cancel: Arc<AtomicBool>,
}
struct Inner {
    entries: Mutex<HashMap<String, Entry>>,
    changed: Condvar,
    serial: Mutex<()>,
}

#[derive(Clone)]
pub struct ModelDownloads {
    dir: PathBuf,
    inner: Arc<Inner>,
}

impl ModelDownloads {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            inner: Arc::new(Inner {
                entries: Mutex::new(HashMap::new()),
                changed: Condvar::new(),
                serial: Mutex::new(()),
            }),
        }
    }

    pub fn model(id: &str) -> Result<&'static ModelInfo, String> {
        MODELS
            .iter()
            .find(|model| model.name == id)
            .ok_or_else(|| format!("Unknown local Whisper model: {id}"))
    }

    pub fn status(&self) -> Vec<ModelDownloadStatus> {
        MODELS.iter().map(|model| self.status_for(model)).collect()
    }

    fn status_for(&self, model: &ModelInfo) -> ModelDownloadStatus {
        let entries = self.inner.entries.lock().unwrap();
        if let Some(entry) = entries.get(model.name) {
            if entry.state == DownloadState::Downloading {
                return ModelDownloadStatus {
                    model_id: model.name.into(),
                    state: entry.state,
                    progress: entry.progress,
                    error: None,
                };
            }
        }
        let downloaded =
            models::is_installed(&self.dir, model) && models::is_installed(&self.dir, &VAD_MODEL);
        let entry = entries.get(model.name);
        ModelDownloadStatus {
            model_id: model.name.into(),
            state: if downloaded {
                DownloadState::Downloaded
            } else {
                entry.map_or(DownloadState::NotDownloaded, |entry| entry.state)
            },
            progress: if downloaded {
                100
            } else {
                entry.map_or(0, |entry| entry.progress)
            },
            error: entry.and_then(|entry| entry.error.clone()),
        }
    }

    /// Returns false when this model already has an active download or is ready.
    pub fn start(&self, id: &str) -> Result<bool, String> {
        let model = Self::model(id)?;
        self.start_info(model)
    }

    fn start_info(&self, model: &ModelInfo) -> Result<bool, String> {
        let mut entries = self
            .inner
            .entries
            .lock()
            .map_err(|error| error.to_string())?;
        if entries
            .get(model.name)
            .is_some_and(|entry| entry.state == DownloadState::Downloading)
            || (models::is_installed(&self.dir, model)
                && models::is_installed(&self.dir, &VAD_MODEL))
        {
            return Ok(false);
        }
        entries.insert(
            model.name.into(),
            Entry {
                state: DownloadState::Downloading,
                progress: 0,
                error: None,
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
        self.inner.changed.notify_all();
        Ok(true)
    }

    pub fn cancel(&self, id: &str) -> Result<(), String> {
        Self::model(id)?;
        let entries = self
            .inner
            .entries
            .lock()
            .map_err(|error| error.to_string())?;
        if let Some(entry) = entries.get(id) {
            entry.cancel.store(true, Ordering::SeqCst);
        }
        self.inner.changed.notify_all();
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let model = Self::model(id)?;
        let mut entries = self
            .inner
            .entries
            .lock()
            .map_err(|error| error.to_string())?;
        if entries
            .get(id)
            .is_some_and(|entry| entry.state == DownloadState::Downloading)
        {
            return Err("Cancel the download before deleting this model".into());
        }
        models::delete(&self.dir, model).map_err(|error| error.to_string())?;
        entries.remove(id);
        self.inner.changed.notify_all();
        Ok(())
    }

    pub fn run(
        &self,
        id: &str,
        changed: &dyn Fn(ModelDownloadStatus),
    ) -> Result<(), ProcessingError> {
        let model = Self::model(id).map_err(ProcessingError::Download)?;
        self.run_with(model, changed, |cancel, progress| {
            let _serial = self.inner.serial.lock().unwrap();
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            let need_model = !models::is_installed(&self.dir, model);
            let need_vad = !models::is_installed(&self.dir, &VAD_MODEL);
            let total = (if need_model { model.size } else { 0 })
                + (if need_vad { VAD_MODEL.size } else { 0 });
            if need_model {
                models::download(
                    &self.dir,
                    model,
                    &mut |bytes| progress(bytes.saturating_mul(100) / total),
                    cancel,
                )?;
            }
            if need_vad {
                models::download_vad(
                    &self.dir,
                    &mut |bytes| {
                        progress(
                            ((if need_model { model.size } else { 0 }) + bytes).saturating_mul(100)
                                / total,
                        )
                    },
                    cancel,
                )?;
            }
            Ok(())
        })
    }

    fn run_with(
        &self,
        model: &ModelInfo,
        changed: &dyn Fn(ModelDownloadStatus),
        operation: impl FnOnce(&AtomicBool, &mut dyn FnMut(u64)) -> Result<(), ProcessingError>,
    ) -> Result<(), ProcessingError> {
        let cancel = {
            let entries = self.inner.entries.lock().unwrap();
            entries
                .get(model.name)
                .ok_or_else(|| ProcessingError::Download("Download was not started".into()))?
                .cancel
                .clone()
        };
        let result = operation(&cancel, &mut |percent| {
            let status = {
                let mut entries = self.inner.entries.lock().unwrap();
                let entry = entries.get_mut(model.name).unwrap();
                let next = percent.min(99) as u32;
                if next == entry.progress {
                    None
                } else {
                    entry.progress = next;
                    Some(ModelDownloadStatus {
                        model_id: model.name.into(),
                        state: entry.state,
                        progress: next,
                        error: None,
                    })
                }
            };
            if let Some(status) = status {
                self.inner.changed.notify_all();
                changed(status);
            }
        });
        let status = {
            let mut entries = self.inner.entries.lock().unwrap();
            let entry = entries.get_mut(model.name).unwrap();
            entry.state = match &result {
                Ok(()) => DownloadState::Downloaded,
                Err(ProcessingError::Cancelled) => DownloadState::Cancelled,
                Err(_) => DownloadState::Failed,
            };
            entry.progress = if result.is_ok() { 100 } else { 0 };
            entry.error = result.as_ref().err().map(ToString::to_string);
            ModelDownloadStatus {
                model_id: model.name.into(),
                state: entry.state,
                progress: entry.progress,
                error: entry.error.clone(),
            }
        };
        self.inner.changed.notify_all();
        changed(status);
        result
    }

    /// Waits for a completed model and VAD. A failed download can be retried from Settings.
    pub fn wait_for(
        &self,
        model: &ModelInfo,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u32),
    ) -> Result<(PathBuf, PathBuf), ProcessingError> {
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            if models::is_installed(&self.dir, model) && models::is_installed(&self.dir, &VAD_MODEL)
            {
                return Ok((
                    models::model_path(&self.dir, model),
                    models::model_path(&self.dir, &VAD_MODEL),
                ));
            }
            let status = self.status_for(model);
            progress(status.progress);
            let entries = self.inner.entries.lock().unwrap();
            let _ = self
                .inner
                .changed
                .wait_timeout(entries, Duration::from_millis(100))
                .unwrap();
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, thread};

    const FIXTURE: ModelInfo = ModelInfo {
        name: "fixture",
        file: "fixture.bin",
        size: 4,
        sha256: "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
    };

    #[test]
    fn progress_cancel_and_retry_are_observable() {
        let dir = tempfile::tempdir().unwrap();
        let downloads = ModelDownloads::new(dir.path().into());
        assert!(downloads.start("base").unwrap());
        assert!(!downloads.start("base").unwrap());
        let (sent, received) = mpsc::channel();
        let worker = downloads.clone();
        let handle = thread::spawn(move || {
            worker.run_with(
                &MODELS[0],
                &|status| {
                    sent.send(status).unwrap();
                },
                |cancel, progress| {
                    progress(42);
                    while !cancel.load(Ordering::SeqCst) {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(ProcessingError::Cancelled)
                },
            )
        });
        assert_eq!(received.recv().unwrap().progress, 42);
        downloads.cancel("base").unwrap();
        assert!(matches!(
            handle.join().unwrap(),
            Err(ProcessingError::Cancelled)
        ));
        assert_eq!(received.recv().unwrap().state, DownloadState::Cancelled);
        assert_eq!(downloads.status()[0].state, DownloadState::Cancelled);
        assert!(downloads.start("base").unwrap());
        downloads
            .run_with(&MODELS[0], &|_| {}, |_, progress| {
                progress(100);
                Ok(())
            })
            .unwrap();
        assert_eq!(downloads.status()[0].state, DownloadState::Downloaded);
    }

    #[test]
    fn processing_waits_until_whisper_and_vad_are_ready() {
        let dir = tempfile::tempdir().unwrap();
        let downloads = ModelDownloads::new(dir.path().into());
        let waiter = downloads.clone();
        let (waiting, observed) = mpsc::channel();
        let handle = thread::spawn(move || {
            waiter.wait_for(&FIXTURE, &AtomicBool::new(false), &mut |percent| {
                let _ = waiting.send(percent);
            })
        });
        assert_eq!(observed.recv().unwrap(), 0);
        assert!(downloads.start_info(&FIXTURE).unwrap());
        downloads
            .run_with(&FIXTURE, &|_| {}, |_, progress| {
                progress(50);
                std::fs::write(models::model_path(dir.path(), &FIXTURE), [1, 2, 3, 4]).unwrap();
                std::fs::write(
                    models::model_path(dir.path(), &VAD_MODEL),
                    vec![0; VAD_MODEL.size as usize],
                )
                .unwrap();
                Ok(())
            })
            .unwrap();
        let (model, vad) = handle.join().unwrap().unwrap();
        assert!(model.exists());
        assert!(vad.exists());
    }

    #[test]
    fn deleting_one_model_keeps_the_shared_vad() {
        let dir = tempfile::tempdir().unwrap();
        let model = models::model_path(dir.path(), &MODELS[0]);
        let vad = models::model_path(dir.path(), &VAD_MODEL);
        std::fs::File::create(&model)
            .unwrap()
            .set_len(MODELS[0].size)
            .unwrap();
        std::fs::File::create(&vad)
            .unwrap()
            .set_len(VAD_MODEL.size)
            .unwrap();
        let downloads = ModelDownloads::new(dir.path().into());
        assert_eq!(downloads.status()[0].state, DownloadState::Downloaded);
        downloads.delete("base").unwrap();
        assert_eq!(downloads.status()[0].state, DownloadState::NotDownloaded);
        assert!(!model.exists());
        assert!(vad.exists());
    }
}

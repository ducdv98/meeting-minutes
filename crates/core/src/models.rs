use crate::processing::ProcessingError;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub struct ModelInfo {
    pub name: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}
pub const TURBO_MODEL: &str = "large-v3-turbo-q5_0";
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "base",
        file: "ggml-base.bin",
        size: 147_951_465,
        sha256: "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe",
    },
    ModelInfo {
        name: "small",
        file: "ggml-small.bin",
        size: 487_601_967,
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    },
    ModelInfo {
        name: TURBO_MODEL,
        file: "ggml-large-v3-turbo-q5_0.bin",
        size: 574_041_195,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
    },
];
pub const DEFAULT_MODEL: &str = "small";
/// Silero is an internal Processing dependency, not a Settings transcription choice.
pub const VAD_MODEL: ModelInfo = ModelInfo {
    name: "silero-v6.2.0",
    file: "ggml-silero-v6.2.0.bin",
    size: 885_098,
    sha256: "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987",
};

pub fn vad_model_url() -> &'static str {
    "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v6.2.0.bin"
}

pub fn model_url(info: &ModelInfo) -> String {
    format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        info.file
    )
}
pub fn model_path(models_dir: &Path, info: &ModelInfo) -> PathBuf {
    models_dir.join(info.file)
}
pub fn is_installed(models_dir: &Path, info: &ModelInfo) -> bool {
    fs::metadata(model_path(models_dir, info)).is_ok_and(|m| m.is_file() && m.len() == info.size)
}

pub fn install_from_reader(
    models_dir: &Path,
    info: &ModelInfo,
    mut body: impl Read,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, ProcessingError> {
    fs::create_dir_all(models_dir)?;
    let target = model_path(models_dir, info);
    if is_installed(models_dir, info) {
        return Ok(target);
    }
    let part = models_dir.join(format!("{}.part", info.file));
    let result = (|| -> Result<PathBuf, ProcessingError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&part)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err(ProcessingError::Cancelled);
            }
            let count = body.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            bytes += count as u64;
            progress(bytes);
        }
        if bytes != info.size || format!("{:x}", hash.finalize()) != info.sha256 {
            return Err(ProcessingError::Checksum);
        }
        file.sync_all()?;
        if target.exists() {
            return Err(ProcessingError::Download(
                "Model file already exists".into(),
            ));
        }
        fs::rename(&part, &target)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

pub fn download(
    models_dir: &Path,
    info: &ModelInfo,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, ProcessingError> {
    download_from_url(models_dir, info, &model_url(info), progress, cancel)
}

pub fn download_vad(
    models_dir: &Path,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, ProcessingError> {
    download_from_url(models_dir, &VAD_MODEL, vad_model_url(), progress, cancel)
}

fn download_from_url(
    models_dir: &Path,
    info: &ModelInfo,
    url: &str,
    progress: &mut dyn FnMut(u64),
    cancel: &AtomicBool,
) -> Result<PathBuf, ProcessingError> {
    if is_installed(models_dir, info) {
        return Ok(model_path(models_dir, info));
    }
    let response = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(30))
        .build()
        .get(url)
        .call()
        .map_err(|e| ProcessingError::Download(e.to_string()))?;
    install_from_reader(models_dir, info, response.into_reader(), progress, cancel).map_err(
        |error| match error {
            ProcessingError::Io(io_error)
                if matches!(
                    io_error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                ProcessingError::Download(io_error.to_string())
            }
            other => other,
        },
    )
}

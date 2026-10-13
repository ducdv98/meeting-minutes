use meeting_minutes_core::models::{install_from_reader, is_installed, ModelInfo, MODELS};
use meeting_minutes_core::processing::ProcessingError;
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

const TEST: ModelInfo = ModelInfo {
    name: "test",
    file: "test.bin",
    size: 3,
    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
};

#[test]
fn model_catalog_offers_quantized_turbo_with_verified_download_metadata() {
    assert_eq!(
        MODELS.iter().map(|model| model.name).collect::<Vec<_>>(),
        ["base", "small", "large-v3-turbo-q5_0"]
    );
    let turbo = &MODELS[2];
    assert_eq!(turbo.file, "ggml-large-v3-turbo-q5_0.bin");
    assert_eq!(turbo.size, 574_041_195);
    assert_eq!(
        turbo.sha256,
        "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2"
    );
}

#[test]
fn install_verifies_hash_and_renames_only_on_success() {
    let temp = TempDir::new().unwrap();
    let mut progress = Vec::new();
    let path = install_from_reader(
        temp.path(),
        &TEST,
        &b"abc"[..],
        &mut |n| progress.push(n),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"abc");
    assert!(is_installed(temp.path(), &TEST));
    assert_eq!(progress.last(), Some(&3));
}

#[test]
fn bad_checksum_and_cancel_leave_no_partial_file() {
    let temp = TempDir::new().unwrap();
    assert!(matches!(
        install_from_reader(
            temp.path(),
            &TEST,
            &b"abd"[..],
            &mut |_| {},
            &AtomicBool::new(false)
        ),
        Err(ProcessingError::Checksum)
    ));
    assert!(!temp.path().join("test.bin.part").exists());
    assert!(matches!(
        install_from_reader(
            temp.path(),
            &TEST,
            &b"abc"[..],
            &mut |_| {},
            &AtomicBool::new(true)
        ),
        Err(ProcessingError::Cancelled)
    ));
    assert!(!temp.path().join("test.bin.part").exists());
}

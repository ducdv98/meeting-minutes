#![cfg(all(windows, target_arch = "x86_64"))]

use meeting_minutes_whisper::WhisperEngine;

#[test]
#[ignore = "run in Windows CI and release jobs with pinned GGML flags"]
fn pinned_cpu_instruction_set() {
    let info = WhisperEngine::system_info();
    let avx2 = info.contains("AVX2 = 1");
    let avx512 = info.contains("AVX512 = 1");
    println!(
        "whisper.cpp: AVX2 = {}, AVX512 = {}",
        avx2 as u8, avx512 as u8
    );
    assert!(avx2, "AVX2 is absent from whisper.cpp system info: {info}");
    assert!(
        !avx512,
        "AVX512 is present in whisper.cpp system info: {info}"
    );
}

# Transcription benchmark

The benchmark uses 10 English and 10 Vietnamese clips from a [pinned FLEURS test split](https://huggingface.co/datasets/google/fleurs/tree/70bb2e84b976b7e960aa89f1c648e09c59f894dd), plus a roughly 10-minute English clip made by repeating those clips with two seconds of silence between them. It downloads the TSV references and extracts only the chosen WAV files from each test archive. Audio, the stitched clip, and model files belong in `benchmarks/data/`, which Git ignores. FLEURS is licensed CC BY 4.0.

Install Python 3, `curl`, FFmpeg, and Rust. Download a [Whisper ggml model](https://huggingface.co/ggerganov/whisper.cpp/tree/main) and the [Silero VAD model](https://huggingface.co/ggml-org/whisper-vad/blob/main/ggml-silero-v6.2.0.bin), or use model files already installed by the app. The app's supported model names, download URLs, and hashes are in `crates/core/src/models.rs`.

On Linux, run:

```sh
python3 scripts/benchmark-transcription download
python3 scripts/benchmark-transcription run \
  --model base=benchmarks/data/models/ggml-base.bin \
  --vad /path/to/ggml-silero-v6.2.0.bin \
  --device cpu
```

On Windows, use PowerShell from the repository root:

```powershell
python scripts/benchmark-transcription download
python scripts/benchmark-transcription list-devices
python scripts/benchmark-transcription run `
  --model base=C:\models\ggml-base.bin `
  --vad C:\models\ggml-silero-v6.2.0.bin `
  --device cpu --device 0
```

`list-devices` prints the Vulkan device IDs for this build. Pass an ID from that list with `--device`; `--device auto` uses the app's normal selection. Repeat `--model NAME=PATH` and `--device` to compare every requested combination. The runner sets `MM_WHISPER_DEVICE` for each run: `cpu` forces CPU, `auto` uses normal selection, and a numeric ID forces that Vulkan device. A requested Vulkan device that is unavailable fails instead of quietly measuring CPU. The override applies only to the process running the benchmark unless you set that environment variable yourself.

The runner imports each clip as a temporary Meeting, then calls `transcribe_meeting` with `WhisperEngine` and the selected Meeting language. It prints a Markdown table with corpus WER by language, Whisper real-time factor (RTF), audio duration after VAD, and Segment duration statistics. RTF measures the Whisper call, including VAD, divided by original audio duration; it excludes Import and preparation time. Lower WER and RTF are better. The stitched row exposes VAD performance on silence and long-form Segment behavior. Each run writes the temporary Transcript Revision in its temporary Meeting folder, which is removed after that clip.

# Cloud transcription research: a fast default paid for by the app owner

Research date: 2026-10-13. Prices are public USD list rates checked on this date. **RTF** means processing time ÷ audio duration; lower is faster. Anything the sources don't state is marked **UNVERIFIED**.

## Why

Local `small` on CPU took about 30 minutes for a 16-minute Meeting on the owner's laptop (RTF ≈ 1.9). That is too slow for the first release. The owner wants cloud transcription as the default:

- it works with no setup;
- it is paid for with the owner's account;
- free users are rate-limited;
- privacy is explained to the user.

Local stays as an option.

## Verdict

Use **Cloudflare Workers AI `@cf/openai/whisper-large-v3-turbo`**, called from a Worker on the owner's existing Cloudflare account.

- **Cost:** about $0.03 per audio hour, with a free allowance of about 3.6 audio hours a day.
- **No key in the app:** the call goes through the Worker's AI binding, so no provider key exists anywhere a user can reach.
- **Privacy:** Cloudflare neither trains on nor retains the audio.
- **Timestamps:** it returns real segment timestamps.

Put a second provider behind the same Worker as a fallback (its key held as a Worker secret, zero data retention on) if turbo's Vietnamese is weak.

**Do not ship a provider key inside the app.** Anyone can pull it out of the binary, and Groq's terms (§3.2, §6.3) make the account owner responsible for every use of the key.

The one open risk is turbo's accuracy on Vietnamese. No source publishes a figure, so test it on real Meetings.

## 1. Cloud providers

| Provider / model | $/audio hr | Free allowance | Vietnamese | Segment timestamps | Data policy |
|---|---|---|---|---|---|
| **Workers AI** `whisper-large-v3-turbo` | **≈0.031** ($0.0005/min, 46.63 neurons/min) | 10K neurons/day ≈ 214 min/day per account | Yes, via the `language` param (accuracy UNVERIFIED) | Yes: `segments[]` with `start`/`end`, plus `words[]` and `vtt` | No training, no retention |
| Workers AI `whisper` | ≈0.03 | same | No language param | Words only | same |
| Workers AI `deepgram/nova-3` | ≈0.31 | same pool | Yes (`vi`) | Words | Deepgram terms; training opt-out field (UNVERIFIED) |
| **Groq** `whisper-large-v3-turbo` | 0.04 | Free plan: 2 h/hour, 8 h/day **per organization** (shared by all of an app's users) | Yes | Yes (`verbose_json`) | Not retained by default; transcription may be logged up to 30 days; Zero Data Retention available; no training |
| Groq `whisper-large-v3` | 0.111 | same | Yes, better than turbo | Yes | same |
| OpenAI `whisper-1` | 0.36 | none | Yes | Yes (only `whisper-1` documents timestamps) | No training |
| OpenAI `gpt-4o-mini-transcribe` / `gpt-transcribe` | 0.18 / 0.27 | none | Yes | Not documented | No training |
| ElevenLabs Scribe v2 | 0.22 | ≈4.5 h on the free plan | **Best: ≤5% WER tier** | Words only | Logging on by default |
| Soniox async | 0.10 | UNVERIFIED | Yes | Token level | UNVERIFIED |
| Deepgram Nova-3 | 0.26–0.31 | $200 signup credit | Yes | Words / utterances | Model-improvement program unless `mip_opt_out=true` |
| AssemblyAI Universal-2 | 0.15 | $50 credit | Yes ("Good", WER 10–25%) | Words / utterances | Free users cannot opt out of training |
| Gemini 2.5 Flash-Lite | ≈0.04 | Free tier trains on data | UNVERIFIED | Written by the model, **not aligned** | Free tier is used for training |
| Mistral Voxtral | 0.18 | Experiment plan | **No** | Words | Unclear |
| Azure Speech F0 | free 5 h/month | Real-time only, 1 concurrent request | Yes | Phrase offsets | – |
| Google Speech V1 | free 60 min/month | – | UNVERIFIED | UNVERIFIED | – |

**Anthropic has no speech-to-text or audio-input API.** The Claude key can't be used for transcription.

Sources:
- Workers AI: [pricing](https://developers.cloudflare.com/workers-ai/platform/pricing/), [turbo model](https://developers.cloudflare.com/workers-ai/models/whisper-large-v3-turbo/), [limits](https://developers.cloudflare.com/workers-ai/platform/limits/), [data usage](https://developers.cloudflare.com/workers-ai/platform/data-usage/), [chunking tutorial](https://developers.cloudflare.com/workers-ai/guides/tutorials/build-a-workers-ai-whisper-with-chunking/)
- Groq: [speech-to-text](https://console.groq.com/docs/speech-to-text), [rate limits](https://console.groq.com/docs/rate-limits), [your data](https://console.groq.com/docs/your-data), [services agreement](https://console.groq.com/docs/legal/services-agreement), [spend limits](https://console.groq.com/docs/spend-limits)
- OpenAI: [pricing](https://developers.openai.com/api/docs/pricing), [STT guide](https://developers.openai.com/api/docs/guides/speech-to-text)
- ElevenLabs: [pricing](https://elevenlabs.io/pricing/api), [STT](https://elevenlabs.io/docs/capabilities/speech-to-text)
- Others: [Soniox](https://soniox.com/pricing), [Deepgram](https://deepgram.com/pricing), [AssemblyAI](https://www.assemblyai.com/pricing), [Gemini](https://ai.google.dev/gemini-api/docs/pricing), [Mistral](https://docs.mistral.ai/capabilities/audio_transcription), [Azure quotas](https://raw.githubusercontent.com/MicrosoftDocs/azure-ai-docs/main/articles/ai-services/speech-service/speech-services-quotas-and-limits.md), [Google](https://cloud.google.com/speech-to-text/pricing)
- Anthropic: [Claude API features](https://platform.claude.com/docs/en/build-with-claude/overview)

## 2. Relay on Cloudflare (abuse and spend control)

| Piece | Use | Notes |
|---|---|---|
| Install ID + Turnstile | Registration on first run: a webview opens `https://<site>/verify`, and the Worker returns a signed install token | Turnstile hostnames must be real domains, so it can't run in the app's local origin. Install IDs can be minted at will, so this only raises the cost of abuse. |
| Rate Limiting binding | Burst limits per install and per IP | 10 s / 60 s windows only, eventually consistent. Not usable for daily quotas. |
| Durable Objects | Per-install daily and monthly audio-minute quotas, plus a **global daily spend cap** | Strongly consistent. **The global cap is what actually bounds spend.** |
| KV | Not for counters | Eventually consistent, no atomic updates |
| AI Gateway | Optional gateway-wide circuit breaker | Turn logging off (it would store Meeting content); caching is useless for audio |

Worker limits that matter ([Workers limits](https://developers.cloudflare.com/workers/platform/limits/)):
- Request body: 100 MB on Free and Pro zones.
- CPU time: 10 ms on Workers Free, which is why Workers Paid ($5/month) is effectively required.
- Memory: 128 MB per isolate.

A 1-hour 16 kHz mono WAV is about 115 MB; as 24 kbps Opus it is about 11 MB.

**Upload:** the client splits each Recording into chunks of ≤10 minutes, encodes them as Opus or FLAC, and sends each one with its timeline offset. Every request stays under 25 MB, so no R2 is needed, and a retry resends only one chunk. The alternative is an R2 presigned upload with a lifecycle rule that deletes uploads after a day.

## 3. Cost at 5 audio hours per user per month

| | 100 users | 1,000 users |
|---|---|---|
| Workers AI turbo (+$5 Workers Paid) | ≈ $17/month | ≈ $156/month |
| Groq v3-turbo (+$5 Workers Paid) | ≈ $25/month | ≈ $205/month |
| Groq v3 (if needed for Vietnamese) | ≈ $61/month | ≈ $560/month |

Groq's free tier (8 h/day per organization) can't cover even 100 users.

## 4. Local speedups (whisper-rs 0.16 bundles whisper.cpp 1.8.3)

Today the app runs on CPU only (`use_gpu(false)`) with greedy decoding, at most 8 threads, and no VAD.

Expected RTF on a typical laptop, estimated from encoder timings:

| | `small` | `large-v3-turbo` |
|---|---|---|
| CPU | ≈0.2–0.35 | ≈0.8–1.2 |
| iGPU (Vulkan) | ≈0.05–0.1 | ≈0.25–0.35 |

**The owner's measured RTF of 1.9 on `small` points to a problem, not to normal CPU speed.** Diagnose it before tuning anything.

Ranked by gain for the effort:

1. **Silero VAD (built in):**
   - What: `FullParams::set_vad_model_path(...)` plus `enable_vad(true)`, and ship a model of about 1 MB.
   - Gain: skips silence and reduces hallucinations.
   - Timestamps are remapped to the original audio.
2. **Threads = physical cores:**
   - What: change one setting.
   - Gain: small.
3. **Quantized models** (`small-q8_0`, `large-v3-turbo-q5_0`/`q8_0`):
   - What: swap the model file.
   - Gain: mostly memory and download size.
   - Vietnamese impact is UNVERIFIED.
4. **Vulkan:**
   - What: the `vulkan` cargo feature, plus `use_gpu(true)` and `flash_attn(true)`. The build machine needs the Vulkan SDK; end users only need their GPU drivers.
   - Gain: 3–4× on recent iGPUs, much more on discrete GPUs.
   - Toolchain: support on the windows-gnu toolchain is UNVERIFIED.
5. **CUDA:** NVIDIA only, needs MSVC and CUDA DLLs to ship. Not worth it before Vulkan.
6. **OpenVINO:** no feature in whisper-rs and speeds up the encoder only. Skip it.

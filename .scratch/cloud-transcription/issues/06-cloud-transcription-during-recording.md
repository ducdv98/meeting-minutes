# Cloud transcription during a Recording

Status: ready-for-human
Spec: ../spec.md
Blocked by: 03

## What

- Start the background transcriber (`crates/core/src/background.rs`) when a Recording starts and a Groq key is set, on any PC and in any power state. It sends each ready span through the engine chain from ticket 03.
- Without a Groq key, ADR 0010's rule is unchanged: background transcription runs only on a discrete GPU on AC power.
- When a span falls back to local, the existing discrete-GPU and AC-power rule decides whether the local engine runs now or after Stop.
- No network: pause cloud work, and resume when the network returns. Whatever is left is done after Stop.
- Results stay a cache outside the Library, and are discarded after a crash (ADR 0010).
- After Stop, the Draft shows at once and only the remainder is transcribed, as today.

## Acceptance criteria

- [x] Core tests with a fake Groq engine:
  - spans are transcribed during a Recording on "integrated GPU, battery";
  - offline pauses and online resumes;
  - Stop transcribes only the remainder.
- [ ] Laptop check (record in `## Comments`): a one-hour call on battery with a Groq key publishes its Revision within about a minute of Stop.

## Comments

- The one-hour laptop check remains open: this Linux environment cannot run the Windows Recording app or verify real Groq latency on a battery-powered laptop.

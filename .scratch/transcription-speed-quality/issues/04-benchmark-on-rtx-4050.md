# Run the benchmark on the RTX 4050 and decide Vulkan vs CUDA

Status: ready-for-human
Spec: ../spec.md
Blocked by: 01, 03

## What

On the 8745H / RTX 4050 Laptop machine, on AC power, run the benchmark for:

- the Radeon 780M (Vulkan) and the RTX 4050 (Vulkan) with `small`, `large-v3-turbo-q5_0`, turbo `q8_0`, and `large-v3` (q5_0 and f16 if VRAM allows);
- the RTX 4050 with a CUDA build of the same models. This is a throwaway local build and is not shipped.

Paste the tables under `## Comments`.

## Decisions to record

- The model table for Auto (ticket 05): which model each device class gets.
- If CUDA is more than 1.5× faster than Vulkan on the RTX 4050, open an amendment to ADR 0008. Otherwise note "Vulkan stays" here.
- Whether the Segment thresholds (1 s and 3 words; 20 s) need tuning.

# 0003. Smaller and newer vision models on a CPU

- Date: 2026-11-20
- Status: partly done. The 450M result has summary lines only; LFM3-VL-2B result is invalid and must be rerun.
- Confidence: measured for the 1.6B model's accuracy. Latency is unreliable (see Analysis).
- Decisions informed: [ADR 0005](../adr/0005-ui-automation-first-pointing.md), [ADR 0004](../adr/0004-local-first-provider-agnostic-models.md)
- Builds on: [finding 0001](0001-vision-grounding-lfm25-vl-cpu.md)

## Question

Does a smaller model (LFM2.5-VL-450M) or a newer one (LFM3-VL-2B) give usable pointing accuracy at lower latency than LFM2.5-VL-1.6B on a CPU-only laptop?

## Hypothesis

Written before running: the 450M model would be faster but less accurate; LFM3-VL-2B would be more accurate but slower than the 1.6B model.

## Setup

- Machine: Intel i7-10750H (6 cores / 12 threads), 15.8 GB RAM, Windows 11, CPU only.
- Runtime: llama.cpp `b12549` `llama-server`, `win-cpu-x64`, `-t 6 -c 4096`, temperature 0.
- Models (Q4_K_M + Q8_0 projector each):
  - LFM2.5-VL-450M: 229 MB model, 103 MB projector
  - LFM2.5-VL-1.6B: 731 MB model, 583 MB projector
  - LFM3-VL-2B: 1100 MB model, 203 MB projector
- Test image and scoring: same as finding 0001 (8 buttons, hit means the centre falls in the box). Images: 1280x720 and 640x360.
- Prompts: `bbox` (fractions box) and `pixel` (point). Harness: [`benchmark.mjs`](../../spikes/vision-grounding/benchmark.mjs), driven by [`run-matrix.sh`](../../spikes/vision-grounding/run-matrix.sh).

## Results

| Model | Image | Prompt | Hits | Mean error | Mean latency | Data |
|---|---|---|---|---|---|---|
| LFM2.5-VL-450M | 640x360 | bbox | 3/8 | 84 px | 2.0 s | summary only |
| LFM2.5-VL-450M | 640x360 | pixel | 0/8 | 705 px | 1.9 s | summary only |
| LFM2.5-VL-450M | 1280x720 | bbox | 4/8 | 84 px | 17.7 s | summary only |
| LFM2.5-VL-450M | 1280x720 | pixel | 0/8 | 754 px | 23.6 s | summary only |
| LFM2.5-VL-1.6B | 640x360 | bbox | 3/8 | 109 px | 27.8 s* | [raw](data/0003-lfm25-vl-1.6b-q4km.synthetic-640.bbox.json) |
| LFM2.5-VL-1.6B | 640x360 | pixel | 1/8 | 305 px | 26.7 s* | [raw](data/0003-lfm25-vl-1.6b-q4km.synthetic-640.pixel.json) |
| LFM2.5-VL-1.6B | 1280x720 | bbox | 4/8 | 66 px | 89.7 s* | [raw](data/0003-lfm25-vl-1.6b-q4km.synthetic-ui.bbox.json) |
| LFM2.5-VL-1.6B | 1280x720 | pixel | 0/8 | 287 px | 88.4 s* | [raw](data/0003-lfm25-vl-1.6b-q4km.synthetic-ui.pixel.json) |
| LFM3-VL-2B | 640x360 | bbox | 0/8 | n/a | 23.3 s* | [raw, invalid](data/0003-lfm3-vl-2b-q4km.synthetic-640.bbox.json) |
| LFM3-VL-2B | 640x360 | pixel | 0/8 | n/a | 22.9 s* | [raw, invalid](data/0003-lfm3-vl-2b-q4km.synthetic-640.pixel.json) |

\* Latency measured while the machine was short of memory (about 3 GB free); treat as an upper bound. The 640x360 1.6B figures were earlier in the run (27 s) than the uncontended measurement from finding 0001 (7.6 s).

Summary log: [`data/0003-matrix-log-summary.txt`](data/0003-matrix-log-summary.txt).

## Analysis

- **450M:** same accuracy band as the 1.6B model on boxes (3/8 and 4/8) and about 4x faster at 640x360 (2.0 s against roughly 8 s uncontended). Like the larger model it cannot give pixel-accurate points (0/8 on pixels).
- **1.6B:** same results as finding 0001, so the run reproduces.
- **LFM3-VL-2B: invalid.** Every reply was empty. Inspection showed the model is a reasoning model: it spent all 40 to 60 tokens of the `max_tokens` budget on hidden reasoning (`reasoning_content`) and never emitted an answer. The benchmark scored those as misses. This is a harness problem, not a verdict on the model. A rerun needs a much larger token budget, or thinking disabled through the chat template, before the model is judged.
- **Latency:** the 1.6B 1280x720 timings of 88 to 90 s are far above the 24 s seen in finding 0001 because free memory dropped to about 3 GB during the run. Those numbers should not be used. Only the 450M 640x360 figure (2.0 s) was measured fairly, and it is the number to trust.
- **Process gap:** the 450M per-target JSON was not written because the `--out` option was added after those runs. The summary above is all we kept.

## Conclusion

- Pointing accuracy did not improve with model size in this test, and no vision model here is click-accurate. Confirms [ADR 0005](../adr/0005-ui-automation-first-pointing.md): the UI Automation tree should provide targets.
- LFM2.5-VL-450M is the best candidate for a fast local "describe what I'm looking at" path, at about 2 s per 640x360 frame, if its answers prove useful for that task. Not yet tested for that.
- LFM3-VL-2B is undecided. Rerun before drawing any conclusion.

## Eval potential

- Pointing eval: keep the synthetic UI as a smoke test; the real gate should use the UI Automation element list ([ADR 0005](../adr/0005-ui-automation-first-pointing.md)).
- Latency eval: rerun only with more than 6 GB free and nothing else heavy running; record free memory in the data file.
- Harness fix needed before evals: set `max_tokens` per model and disable reasoning for reasoning-capable models.

## Reproduce

```bash
powershell -File spikes/vision-grounding/make-test-image.ps1 -OutDir ./data
spikes/vision-grounding/run-matrix.sh <llama-dir> <models-dir> ./data ./out
```

# Spike: vision grounding

Benchmark harness for the question "can a small local vision model point at UI elements on a CPU?".

- `make-test-image.ps1` draws a synthetic UI with known button positions.
- `benchmark.mjs` asks a local `llama-server` to locate each button and scores the answer.

Results, analysis and reproduction steps live in the finding:
[docs/findings/0001-vision-grounding-lfm25-vl-cpu.md](../../docs/findings/0001-vision-grounding-lfm25-vl-cpu.md).

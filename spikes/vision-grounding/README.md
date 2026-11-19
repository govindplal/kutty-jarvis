# Spike: can a small local VLM point at UI elements on a CPU?

Question: can LFM2.5-VL-1.6B (Q4_K_M, llama.cpp CPU build, no GPU) return
click-accurate coordinates for a UI element on a Windows screenshot?

## Setup

- Machine: Intel i7-10750H (6 cores / 12 threads), 15.8 GB RAM, CPU only.
- Runtime: llama.cpp `b12549` `llama-server`, `win-cpu-x64` build.
- Model: `LiquidAI/LFM2.5-VL-1.6B-GGUF` Q4_K_M (731 MB) + mmproj Q8_0 (583 MB).
- Test: synthetic 1280x720 UI with 8 labelled buttons at known positions
  (`make-test-image.ps1`). A hit means the predicted centre falls inside
  the button. Scored by `benchmark.mjs`.

## Results (8 targets, temperature 0)

| Prompt asks for | Image sent | Hits | Mean error | Latency per new image |
|---|---|---|---|---|
| centre in pixels | 1280x720 | 0/8 | 287 px | ~23 s |
| centre as 0-1 fractions | 1280x720 | 1/8 | 203 px | ~23 s |
| bounding box as 0-1 fractions | 1280x720 | 4/8 | 66 px | ~24 s |
| bounding box as 0-1 fractions | 640x360 | 3/8 | 109 px | ~7.6 s |

Other observations:

- Almost all the time is image prompt processing (~35 tokens/s on this CPU,
  810 tokens at 1280x720, 263 at 640x360). Generation is ~30 tokens/s.
- Thread count (6, 8, 12) and batch size barely changed latency.
- Re-sending the identical image is served from the prompt cache (0.7 s),
  but a real screenshot is new every time.
- Answers are coarse (values land on 0.05 steps) and sometimes repeat the
  same box for different targets.

## Conclusions

1. Raw coordinates from a 1.6B model are not click-accurate. Do not rely on
   them as the primary pointing mechanism.
2. Make Windows UI Automation the primary source of targets. Give the model
   a numbered list of on-screen elements (name, role, bounds) and have it
   answer with an element id; the app then points at the exact rectangle.
   Use VLM bounding boxes only as a coarse fallback for apps that expose
   no accessibility tree (games, canvas apps, remote desktops).
3. Local vision is slow on CPU (about 8 s at 640x360). Local mode needs a
   downscaled screenshot, streaming speech, and a "thinking" cue. The cloud
   path stays the fast option.
4. Not yet tried: the 450M model, Q4_0 weights, other image sizes, and
   ONNX runtime. These could cut latency.

## Licence note

The LFM weights use the LFM Open License v1.0, which is not OSI-approved
and has commercial-use conditions. kutty-jarvis must not redistribute the
weights; download them at first run from Hugging Face and show the licence.

## Reproduce

```bash
powershell -File make-test-image.ps1 -OutDir ./data
llama-server -m LFM2.5-VL-1.6B-Q4_K_M.gguf --mmproj mmproj-LFM2.5-VL-1.6b-Q8_0.gguf -t 6 -c 4096 --port 8089
node benchmark.mjs data/synthetic-ui.png data/synthetic-ui.json http://127.0.0.1:8089 bbox
```

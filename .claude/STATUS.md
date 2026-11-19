# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-20

## Now
- Repo: https://github.com/govindplal/kutty-jarvis (private). `main` has the license, docs, commitlint and branch-name hooks (PR #1 merged).
- PR #2 open (`feat/vision-grounding-spike`): benchmark harness and findings, no production code.
- Decided: Tauri v2 + React/TS (best for CPU-only modest laptops), Windows first, MIT, buddy-first.

## Spike result (details in spikes/vision-grounding/README.md)
- LFM2.5-VL-1.6B Q4 on CPU (i7-10750H): bbox prompt 4/8 hits at 1280x720 (~24 s), 3/8 at 640x360 (~7.6 s). Not click-accurate.
- Design consequence: Windows UI Automation is the primary pointing source; the model picks an element id from a list. VLM boxes are a coarse fallback only.

## Next
- Merge PR #2 after review.
- Spike 2: local speech-to-text and text-to-speech on Windows CPU (LFM2.5-Audio ONNX vs whisper.cpp / Piper); measure latency.
- Try the 450M vision model and smaller images to cut local latency.
- Scaffold Tauri app (branch `feat/tauri-app-scaffold`): tray, global hotkey, transparent overlay.

## Open questions / blocked on the user
- Branch protection needs GitHub Pro or a public repo (free plan, private repo returned 403). Decide: make the repo public, or keep private and rely on the local pre-push hook.

## Learned this project
- Local tooling lives in gitignored `.tools/` (llama.cpp b12549 CPU build) and `.models/` (GGUF weights). Large HF downloads can truncate silently: compare size to Content-Length, resume with `curl -C -`.
- LFM weights use the LFM Open License v1.0 (not OSI): download at first run, never redistribute.
- Commit bodies must wrap at 72 columns (commitlint) and end with the Co-Authored-By trailer.

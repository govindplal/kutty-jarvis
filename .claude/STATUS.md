# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-20

## Now
- Research done (HeyClicky, farzaa/clicky MIT source, Flicky, ClickyX, Liquid AI models). Decisions recorded in CLAUDE.md. No code yet.

## Next
- User confirms proposed stack (Tauri v2 + React/TS, llama.cpp sidecar, UI Automation pointing).
- Spike 1: can LFM2.5-VL-1.6B (GGUF, llama-server) return usable click coordinates from a Windows screenshot?
- Spike 2: local speech-to-text and text-to-speech options on Windows (LFM2.5-Audio via ONNX vs whisper.cpp / Piper).
- Then scaffold the app and build the hotkey -> capture -> model -> voice -> pointer loop.

## Open questions / blocked on the user
- Confirm stack; choose GitHub remote (ask before pushing).
- Minimum hardware to target for local models (GPU vs CPU-only).

## Learned this project (not yet in CLAUDE.md)
- Reference design (farzaa/clicky): POINT tag protocol `[POINT:x,y:label:screenN]` in model output, click-through overlay window animates cursor along a bezier arc.
- rea skill not needed: the original core is MIT; closed-app internals are off limits (ToS, taint).

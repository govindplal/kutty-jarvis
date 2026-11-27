# Project status (2026-11-27)

- `main` is protected; required checks include commit/PR title lint, branch name, and app checks. Rebase-merge only.
- Merged: tray/hotkey, screen capture, microphone recording and latency fix, UI element listing/pointing overlay, model loop, and spoken replies (Piper with SAPI fallback).
- On hotkey release the app transcribes with LFM3-Audio, sends screenshot and numbered UI elements to LFM2.5-VL, validates `[POINT:n]`, points the overlay and speaks the reply. Models run as lazy `llama-server` sidecars; setup is currently manual.
- Landing (branch `feat/cloud-api-keys`): optional cloud providers with the user's own keys (ADR 0015). Anthropic/OpenAI answering, OpenAI transcription, keys in the OS credential store, Models settings section. Local stays default; no silent fallback. Verified with mock servers only.
- Next: download llama-server and models on first run; real-laptop tests of hotkey, answer, speech and pointing; rerun vision benchmarks; spoken (real microphone) test; Windows job-object cleanup for sidecars.
- Open questions: whether ~15-20 s per CPU answer is acceptable; whether the local model can reliably select UI elements for open-ended questions; real cloud calls and Credential Manager are unverified.

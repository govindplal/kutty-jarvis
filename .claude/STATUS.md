# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-27

## Now
- `main` protected; required checks: `Commit and PR title lint`, `Branch name`, `App checks`; branch must be up to date.
- Merge method: rebase-merge only (ADR 0011). No Claude trailers on commits or PRs (`attribution` in `.claude/settings.json`).
- Merged: tray and hotkey (#10), screen capture (#11), microphone recording (#12), recording start-gap fix (#13), UI element list and pointing overlay.
- Model loop built on `feat/model-loop` (ADR 0012): on key release the app transcribes the recording (LFM3-Audio), asks LFM2.5-VL with screenshot + numbered element list, parses `[POINT:n]`, points the overlay, and emits `assistant-thinking` / `assistant-replied` / `assistant-failed`.
- Local models run as lazily started `llama-server` sidecars; the files are placed by hand (docs/setup/local-models.md).
- Speech output on `feat/speech-output-piper`: `tts::speak`/`tts::stop` with Piper (downloaded on first use) and SAPI fallback (ADR 0014), not yet called by the model loop. Vision benchmark reruns are a separate branch.

## Next
- Spoken test at the laptop: real microphone, real windows, real latency (the loop was only run on a Linux sandbox with synthetic speech).
- Speak the reply: subscribe to `assistant-replied` (Piper, ADR 0007).
- Model download step (pinned llama.cpp and GGUF files with size checks) so setup is not manual.
- Cloud providers with user keys: another `SpeechToText` / `Assistant` implementation; enable TLS on `ureq`.
- Windows job object so a sidecar cannot outlive a killed app.
- Finding: can the local model pick the right UIA element id from a list? (Sandbox: right for "save"/"settings", wrong for an open question.)

## Open questions / blocked on the user
- Spoken test sentences need the user present.
- Is ~15-20 s per answer on CPU acceptable, or do we need a smaller model or streaming first?

## Learned this project
- Check exit codes and scan full test output; a summary line like "5 passed" hid unhandled errors once.
- When resolving merge conflicts, `--theirs` means the branch being merged in (main), not your branch. Confirm the side before choosing.
- Machine is memory-constrained during model runs. Close other apps before benchmarks.
- Kill only processes you started (a broad WebView2 kill was blocked).
- Commit bodies must wrap at 72 columns.
- Only one app instance at a time: the global hotkey and simulated key presses are system-wide.
- Small local VLMs need worked examples in the prompt; image size drives CPU latency far more than the element list.

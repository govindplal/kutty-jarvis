# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-20

## Now
- `main` protected; required checks: `Commit and PR title lint`, `Branch name`, `App checks`.
- Merged: docs (#8), lint and tests (#9), tray and hotkey (#10), screen capture (#11).
- Microphone recording while the key is held: built on `feat/microphone-recording-on-hotkey` (ADR 0010). Verified in the built app: a 1.3 s recording appeared. Speech quality untested; the recording is 16 kHz mono WAV in memory.

## Next
- Open PR for screen capture; merge after `App checks` passes.
- Measure the start-up gap (held ~2 s, recorded 1.3 s) and test a spoken question end to end.
- Connect the captured frame and transcript to the model (llama-server, LFM2.5-VL-450M as the candidate).
- Rerun LFM3-VL-2B with reasoning off and a bigger token budget (finding 0003 is invalid for it).
- Voice licence review for the Lessac Piper voices before shipping.

## Open questions / blocked on the user
- None blocking.

## Learned this project
- Check exit codes and scan full test output; a summary line like "5 passed" hid unhandled errors once.
- When resolving merge conflicts, `--theirs` means the branch being merged in (main), not your branch. Confirm the side before choosing.
- Squash merges leave branches that conflict with main; merge main in and check each conflict.
- Machine is memory-constrained during model runs. Close other apps before benchmarks.
- Kill only processes you started (a broad WebView2 kill was blocked).
- Commit bodies must wrap at 72 columns; use `.tools/gc.sh`.

# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-27

## Now
- `main` protected; required checks: `Commit and PR title lint`, `Branch name`, `App checks`; branch must be up to date.
- Merge method: rebase-merge only (ADR 0011). No Claude trailers on commits or PRs (`attribution` in `.claude/settings.json`).
- Merged: tray and hotkey (#10), screen capture (#11), microphone recording (#12), recording start-gap fix (#13).
- The app records screen + voice on Ctrl+Alt+Space but has no model call, no speech output, no pointing yet.
- Swarm running (2026-11-27), one worktree branch per task:
  - debug-only WAV export of the last recording
  - voice and Piper licence review (docs)
  - model loop: llama-server sidecar, transcript + frame -> reply text
  - pointing overlay + UI Automation element list
  - then: vision benchmark reruns (alone, memory), Piper speech output, cloud keys

## Next
- Spoken-question test with the user at the laptop, using the WAV export.
- Finding: can the local model pick the right UIA element id from a list?

## Open questions / blocked on the user
- Spoken test sentences need the user present.

## Learned this project
- Check exit codes and scan full test output; a summary line like "5 passed" hid unhandled errors once.
- When resolving merge conflicts, `--theirs` means the branch being merged in (main), not your branch. Confirm the side before choosing.
- Machine is memory-constrained during model runs. Close other apps before benchmarks.
- Kill only processes you started (a broad WebView2 kill was blocked).
- Commit bodies must wrap at 72 columns; use `.tools/gc.sh`.
- Only one app instance at a time: the global hotkey and simulated key presses are system-wide.

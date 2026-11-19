# Status (rewrite at every checkpoint; snapshot, not a log; under ~40 lines)

Updated: 2026-11-20

## Now
- Repo public, `main` protected (PR required, linear history, CI checks `Commit and PR title lint` and `Branch name` required, admins bound).
- Merged: #1 commitlint, #2 vision spike, #3 docs/ADRs, #4 CI, #5 speech spike.
- Open: Tauri scaffold (`feat/tauri-app-scaffold`, `app/`), vision matrix finding 0003 (`feat/small-vision-model-spike`).
- C++ Build Tools installed; Rust builds. `cargo check` and debug binary build pass; the app launches and stays up.

## Next
- Measure idle RAM of the Tauri app and compare to an Electron baseline (ADR 0003 asks for this). Not measured yet: the attempt was blocked and not repeated.
- Rerun LFM3-VL-2B with reasoning disabled and a larger token budget (finding 0003 is invalid for it).
- First feature: hotkey plus tray icon (`feat/tray-and-hotkey`), then screen capture.
- Add lint and test tooling (eslint, clippy, vitest, cargo test) in its own chore branch.

## Open questions / blocked on the user
- None blocking. Speech voice licence review (Lessac dataset) still open (finding 0002).

## Learned this project
- Machine is memory-constrained during model runs (16 GB, about 3 GB free while a 1.6B model and browsers run). Claude Code may stop background waits under memory pressure. Close other apps before benchmarks.
- Avoid killing processes you did not start: a broad `taskkill` of WebView2 was blocked. Kill only your own PIDs.
- commitlint rejects body lines over 72 chars: use `.tools/gc.sh` (wraps bodies automatically).
- Branches cut before switching: `git switch -c` fails if the name exists and the commit lands on the current branch. Check `git branch --show-current` before committing.

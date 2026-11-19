# kutty-jarvis

Free, open-source (MIT) AI buddy that lives on your PC: press a hotkey, ask out loud, it sees your screen, answers by voice and flies a cursor to the thing it is talking about. Inspired by HeyClicky; clean-room build, no code or prompts copied from the closed app. Main constraint: must be genuinely free to run (local models by default), with cloud API keys as an optional upgrade.

## Architecture (non-negotiable principles)
- Local-first: nothing leaves the machine unless the user adds a cloud key. No telemetry.
- Provider-agnostic: model, speech-to-text and text-to-speech sit behind interfaces; local (Liquid AI LFM) and cloud (user's own keys) are interchangeable.
- Screen captured only on hotkey press; screenshots never stored.
- No proxy server required; keys stay in the OS credential store.
- Windows first, but keep platform code behind a thin layer so macOS/Linux can follow.

## Built vs still to build
Nothing built yet. Buddy-first scope (v1): hotkey push-to-talk, screen capture, model call, voice reply, pointing overlay. Agents/routines are out of scope until the buddy is solid.

## Stack (decided)
- Name: kutty-jarvis. License: MIT. Built from scratch (not a fork). See ADRs 0002 and 0003.
- Platform: Windows 10/11 first.
- Desktop shell: Tauri v2 scaffold in `app/` (React 19, Vite 8, TypeScript 6). Needs MSVC C++ Build Tools to build Rust on Windows.
- Models: user API keys (e.g. Anthropic/OpenAI) AND local Liquid AI models (LFM2.5-VL vision, LFM2.5-Audio) via llama.cpp or ONNX.
- Proposed, not yet confirmed: Tauri v2 (Rust) + React/TS; llama-server sidecar for local models; Windows UI Automation for exact pointing targets with VLM grounding as fallback.

## Commands
- App folder: `app/` (Tauri v2 + React + TypeScript, crate `kutty_jarvis_lib`).
- Install: `cd app && npm install`
- Typecheck and frontend build: `cd app && npm run build`
- Rust check: `cd app/src-tauri && cargo check`
- Run the desktop app: `cd app && npm run tauri dev`
- Debug binary (no installer): `cd app && npx tauri build --debug --no-bundle`
- Test and lint: not set up yet (planned: vitest, eslint, clippy, cargo test).

## Gotchas
- LFM2.5-Audio llama.cpp support was a draft PR (#18641) at last check; the ONNX path may be more mature. Verify before committing.
- Unverified: whether LFM2.5-VL gives usable click-point grounding. Spike this first.

## Workflow
- Explore and plan before non-trivial edits; use `/start-feature` and `/fix-bug`, and `/wrap-up` to end a session.
- Run tests/lint and report real results before calling work done.
- **Ship every checkpoint:** after each completed feature (tests/lint green), commit and push to `origin` (TODO: remote, ask before adding it). If it deploys, confirm the deploy succeeded and give the live URL.

## Current status
@.claude/STATUS.md

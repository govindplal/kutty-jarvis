# Research 0001. HeyClicky and the open-source landscape

- Date researched: 2026-11-20
- Confidence: product page and `farzaa/clicky` README/AGENTS.md read directly; clone details come from READMEs and search summaries and are unverified unless noted.

## The product

[HeyClicky](https://www.heyclicky.com/) is a Mac (Sonoma 14.2+) and Windows 10/11 menu-bar AI buddy by Farza. Press a hotkey, ask out loud, and it sees the screen, answers by voice and moves a cursor to what it is explaining. The paid app has grown beyond the open-source snapshot:

- Draws on screen to teach; walkthroughs "all the way to done".
- Voice-spawned agents that research, write and build; connectors to Gmail, Notion and "100+ apps".
- Routines and a "drives your computer all day" mode on the top tier.
- Privacy claims (self-reported): screen read only on hotkey press, screenshots never stored, text summaries kept for context, account deletion in settings.
- Pricing (the site contradicts itself): cards show Free, Pro $16/mo billed yearly, Max $80/mo billed yearly; the FAQ says Pro $20/mo and Max $100/mo. Free has limited daily usage. The site claims 80,000+ users.

## The open-source original

[farzaa/clicky](https://github.com/farzaa/clicky) is **MIT licensed**. In April 2026 the author said the existing codebase stays open ("tinker with it, make it yours, start a company out of it") while new work stays private. Architecture, read from its `AGENTS.md`:

- Native macOS Swift app, menu-bar only, no Dock icon. A state machine (idle, listening, processing, responding) coordinates everything.
- Push-to-talk (ctrl+option) through a listen-only CGEvent tap, because modifier-only shortcuts are detected more reliably that way.
- Mic audio streams to AssemblyAI over a websocket for live transcription; screenshots of every display via ScreenCaptureKit; transcript, screenshots and history go to Claude (Sonnet 4.6 default); the streamed reply is spoken with ElevenLabs.
- **Pointing:** the model embeds tags like `[POINT:x,y:label:screenN]` in its reply. A full-screen transparent non-activating overlay parses the tag, maps coordinates to the right monitor and animates a cursor along an arc.
- A Cloudflare Worker proxy holds the API keys (routes: chat, text-to-speech, transcription token) so the app ships no secrets.
- Documented gotchas: reuse one long-lived URL session for the websocket (per-connection sessions caused "Socket is not connected"); building from the terminal invalidates macOS permissions.

## Community clones and ports

Not verified beyond README level; maturity and licences should be checked before relying on any of them.

| Project | Stack | Notes |
|---|---|---|
| [Flicky](https://github.com/jvaught01/flicky) | Electron, TypeScript | Windows, macOS, Linux; MIT; cloud providers (Claude, OpenAI), ElevenLabs, Groq Whisper; user keys required; no local model option seen. |
| [ClickyX](https://github.com/unn-Known1/clickyX) | Rust, Tauri v2, React | Windows, Linux, macOS; MIT; push-to-talk, wake word, agents, computer use, MCP, local HTTP bridge; user keys or OpenAI-compatible endpoints; system text-to-speech works offline; no local LLM mentioned. |
| Skilly, Vidi, OpenClicky | Mixed | Mostly Mac menu-bar forks; some use OpenAI Realtime. |

**Gap we target:** none of them is genuinely free to run. All need paid keys for the model, and none ships a local-first CPU path.

## Local model landscape (Liquid AI)

- LFM2.5-VL (1.6B) runs in llama.cpp (needs the model GGUF plus an `mmproj` projector) and in ONNX. A third-party guide put the Q4 size at 696 MB; we measured 731 MB for Q4_K_M. See [Liquid docs](https://docs.liquid.ai/deployment/on-device/llama-cpp/multimodal).
- LFM2.5-Audio (speech recognition and synthesis) has an ONNX path ([docs](https://docs.liquid.ai/deployment/on-device/onnx)); the llama.cpp path was a draft pull request (#18641) at last check.
- Weights are under the LFM Open License v1.0 (`license: other`), not OSI-approved, with commercial conditions. We must not redistribute the weights.

## The `rea` tool

[morluto/rea](https://github.com/morluto/rea) is an MCP server that gives agents reverse-engineering tools (binaries, Electron/JS, traffic captures). Its own documentation limits use to software you own or are authorised to examine. We decided not to use it on the closed HeyClicky app; see [ADR 0002](../adr/0002-build-from-scratch-clean-room.md).

## Sources

- https://www.heyclicky.com/
- https://github.com/farzaa/clicky (README, AGENTS.md)
- https://github.com/farzaa/clicky-releases (update feed only, no app source)
- https://github.com/jvaught01/flicky
- https://github.com/unn-Known1/clickyX
- https://github.com/morluto/rea
- https://docs.liquid.ai/deployment/on-device/llama-cpp/multimodal
- https://docs.liquid.ai/deployment/on-device/onnx
- https://github.com/ggml-org/llama.cpp/pull/18641

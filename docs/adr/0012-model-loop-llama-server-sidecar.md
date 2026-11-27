# 0012. Run the model loop on llama-server sidecars behind provider interfaces

- Status: accepted
- Date: 2026-11-27
- Deciders: Claude Code (model-loop agent); owner to review in the PR
- Evidence: [finding 0001](../findings/0001-vision-grounding-lfm25-vl-cpu.md), [finding 0002](../findings/0002-local-speech-on-cpu.md), [finding 0003](../findings/0003-small-vision-models-on-cpu.md), [ADR 0004](0004-local-first-provider-agnostic-models.md), [ADR 0005](0005-ui-automation-first-pointing.md)

## Context

Push-to-talk records a question and takes a screenshot, and the pointing overlay can fly to a UI element, but nothing connects them. The loop must turn the recording into text, ask a vision-language model about the screen, and point. It has to be free to run (local models by default), must not send anything off the machine unless the user adds a cloud key, and must let a cloud provider replace either model later (ADR 0004).

The findings fix the local models: LFM3-Audio-2B (Q4_K_M) for speech recognition and LFM2.5-VL-1.6B (Q4_K_M) for vision, both served by `llama-server` from llama.cpp through the OpenAI-style `/v1/chat/completions` API.

## Options considered

1. **Link llama.cpp into the app** (Rust bindings). One process, no ports. Cons: native build of llama.cpp on every platform, audio models need the newest llama.cpp (finding 0002), long Windows CI builds.
2. **One `llama-server` per model as a sidecar process**, started lazily. Pros: the exact setup measured in the findings; runtime upgrades are a file swap; the same HTTP client works for cloud. Cons: two processes, memory, a model load on first use.
3. **A single server for both models.** llama-server loads one model per process. Not possible.
4. **Python/ONNX runtime.** Rejected earlier (ADR 0003).

## Decision

We will run speech recognition and vision as two `llama-server` sidecars, reached through two small traits (`SpeechToText`, `Assistant`) implemented by one OpenAI-compatible client.

- **Lifecycle.** Each server starts on first use, listens on 127.0.0.1 on a free port, hides its console window on Windows, and is killed when the app exits. We only stop processes we started. `KJ_ASR_URL` and `KJ_VLM_URL` point at a server the user already runs; those are never started or stopped.
- **Files.** `llama-server` and the GGUF files are found under the app's local data folder (`llama/`, `models/`), with `KJ_LLAMA_SERVER`, `KJ_MODELS_DIR` and per-file overrides. Missing files give an error that names the folder and file. There is no automatic download yet.
- **Turn.** On key press we also list the foreground window's UI elements (the overlay is never foreground, and listing before pointing avoids our own overlay hiding windows from UI Automation, finding 0007). On release, when the recording and screenshot are ready, a background thread transcribes, asks the model with screenshot, transcript and numbered element list, strips a `[POINT:n]` tag from the reply, checks `n` against the list, and flies the overlay to that element's rectangle.
- **Nothing leaves the machine.** The HTTP client for local servers ignores proxy settings and follows no redirects, and `KJ_ASR_URL` / `KJ_VLM_URL` are rejected unless the host is `localhost`, `127.0.0.0/8` or `::1`. The press and the release are paired on the hotkey thread: the recorder and its screen context move into the worker together, so a quick second press cannot hand its screen to the first question.
- **What the model sees.** The element list holds ids, roles and names only, introduced as data from the screen rather than instructions; pixel rectangles stay local and the overlay resolves the chosen id itself. Only a final `[POINT:n]` tag at the end of the reply can point, and `pointedElement` is reported only if the overlay actually showed it. If the screenshot failed, the model is told it has none.
- **Monitors.** The screenshot and overlay cover the primary monitor only (the existing limit from ADR 0013). A foreground window on another monitor is still listed, but pointing at it may fail.
- **Events.** `assistant-thinking`, `assistant-replied` (transcript, reply, pointed element, timings) and `assistant-failed` (stage, message). Speech output will subscribe to `assistant-replied` in its own change.
- **Overlap.** One question at a time. A question that finishes recording while another is being answered is dropped and reported as `assistant-failed` with stage `busy`.
- **Prompt.** Our own system prompt with two short examples (without them the 1.6B model echoed the list instead of answering). The local vision client shrinks the screenshot to 960 px on the long edge (`KJ_VLM_IMAGE_EDGE`): pointing comes from the element list, and a 1280 px frame took about three times as long on CPU.

## Consequences

- Good: matches the measured setup; cloud providers are one more trait implementation with the same client (TLS is a one-line feature flag on `ureq`, not enabled yet because only localhost is used); the pipeline is tested with fakes and no server.
- Bad / costs: two servers hold roughly 3 GB of RAM once both are loaded; the first question pays a model load; users must install `llama-server` and the models by hand until a download step exists; if the app is killed hard, a sidecar can outlive it (a Windows job object would fix that).
- What would make us revisit this: a llama.cpp release that serves audio and vision from one process, a download manager, or the answer latency on typical laptops staying above what feels conversational (finding 0003 data suggests it may).

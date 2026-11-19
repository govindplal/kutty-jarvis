# 0004. Local-first, provider-agnostic models

- Status: accepted
- Date: 2026-11-20
- Deciders: project owner
- Evidence: [finding 0001](../findings/0001-vision-grounding-lfm25-vl-cpu.md)

## Context

The goal is a product that is genuinely free to run. Existing clones all require paid API keys. The owner wants local models from Liquid AI (LFM family) and optional cloud keys. Privacy is a selling point: the screen should only be read on hotkey press and nothing should leave the machine by default.

## Options considered

1. **Cloud only, user supplies keys.** Best quality and latency, but not free and not private.
2. **Local only.** Free and private, but slow and less capable on CPU.
3. **Provider-agnostic core, local by default, cloud optional.**

## Decision

Model access sits behind three interfaces: vision-language model, speech-to-text and text-to-speech. Each has local (Liquid LFM through llama.cpp or ONNX) and cloud (user's own API keys) implementations that can be swapped at runtime. Cloud keys live in the Windows Credential Manager. There is no proxy server and no telemetry.

## Consequences

- Good: free by default, private, vendor-neutral, easy to benchmark providers against each other.
- Cost: more interface surface; local mode is slow (about 8 s for a new image at 640x360 on an i7-10750H, see finding 0001), so the UI must handle latency gracefully.
- Licence: LFM weights use the LFM Open License v1.0, which is not OSI-approved. We never bundle the weights; we download them on first run and show the licence.

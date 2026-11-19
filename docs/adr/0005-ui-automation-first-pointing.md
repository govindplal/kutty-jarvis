# 0005. Use UI Automation first for pointing

- Status: accepted
- Date: 2026-11-20
- Deciders: Claude Code, confirmed by measurement; owner informed
- Evidence: [finding 0001](../findings/0001-vision-grounding-lfm25-vl-cpu.md)

## Context

The buddy's signature feature is moving a cursor to the UI element it is talking about. The original design asks a large cloud model for pixel coordinates in a tag. We measured whether a small local vision model can do the same on a CPU.

## Options considered

1. **Model returns pixel coordinates.** Measured 0/8 hits on a synthetic UI.
2. **Model returns a normalised bounding box.** Best raw result: 4/8 hits, 66 px mean error at 1280x720, but about 24 s per image on CPU; 3/8 and 109 px at 640x360 (about 7.6 s).
3. **Windows UI Automation (UIA) tree supplies targets.** The app enumerates visible elements with name, role and exact bounds; the model chooses an element id from a numbered list. The cursor goes to the exact rectangle.

## Decision

UIA is the primary source of pointing targets. The model answers with an element id from the list we give it. Raw model boxes are a coarse fallback for apps that expose no accessibility tree (games, canvas apps, remote desktops, some Electron apps).

## Consequences

- Good: pixel-accurate pointing independent of model size; cheaper prompts (text list instead of a high-resolution image) which also helps latency.
- Cost: UIA enumeration can be slow or incomplete in some apps; we need a quality fallback path and per-app testing.
- Open: whether LFM2.5-VL can pick the right element id from a list reliably. Needs its own finding and eval before we rely on it.

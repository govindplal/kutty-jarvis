# 0007. Local speech stack: LFM3-Audio for recognition, Piper for speech output

- Status: proposed (pending noisy-speech accuracy, a listening test and voice licence review)
- Date: 2026-11-20
- Deciders: Claude Code, owner informed
- Evidence: [finding 0002](../findings/0002-local-speech-on-cpu.md)

## Context

The buddy needs speech-to-text for the user's question and text-to-speech for the answer, both locally on a CPU-only laptop, with a cloud option behind the same interfaces ([ADR 0004](0004-local-first-provider-agnostic-models.md)). The owner asked for Liquid AI models where possible.

## Options considered

1. **LFM3-Audio-2B for both.** One model, one vendor. Recognition was perfect and fast (WER 0%, RTF 0.35), but speech output ran at 1.35 to 2.1 times real time.
2. **LFM3-Audio for recognition, Piper for speech output.** Piper ran at 0.11 to 0.25 times real time. Adds a second small engine (about 63 MB voice).
3. **Operating-system speech (Windows SAPI) for output.** Instant and dependency-free, but robotic. Useful as an emergency fallback.
4. **Cloud speech services.** Best quality, not free; stays an optional provider.

## Decision

- Recognition: LFM3-Audio-2B (`Q4_K_M` with `Q8_0` projector) served by llama.cpp.
- Speech output default: Piper run as a separate process.
- Optional "high quality" mode using LFM3 speech output for users who prefer voice quality over speed (quality not yet evaluated).
- Fallback when nothing is downloaded: Windows SAPI.
- All behind the speech-to-text and text-to-speech interfaces from ADR 0004.

## Consequences

- Good: a conversational local loop is feasible; speech is no longer the slow part.
- Cost: two local engines to download and manage; extra memory (unmeasured).
- Risks: the Lessac voice dataset licence and Piper's archived and GPL-successor status need review; recognition accuracy on real, noisy speech is untested.
- Revisit when: a faster LFM speech-output path appears, a noisy-speech eval shows high WER, or a voice licence blocks distribution.

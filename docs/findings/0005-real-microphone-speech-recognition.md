# 0005. Real-microphone speech recognition

- Date: 2026-11-27
- Status: pending owner recording
- Confidence: not yet measured
- Decisions informed: [ADR 0007](../adr/0007-speech-stack.md), [ADR 0010](../adr/0010-record-microphone-only-while-key-held.md)

## Question

Does LFM3-Audio (Q4_K_M) transcribe typical buddy questions spoken into the real laptop microphone with a word error rate low enough to use (target: at most 15% overall)? Finding 0002 only used synthetic speech.

## Hypothesis

Written before measuring: WER on real microphone audio will be higher than the synthetic 0%-class result of finding 0002, but under 15% for short English questions in a quiet room.

## Setup

- App: debug build of kutty-jarvis; the recording is exported to `%TEMP%/kutty-jarvis/last-recording.wav` (16 kHz mono 16-bit, ADR 0010 amendment).
- Model: `LFM3-Audio-2B-Q4_K_M.gguf` with `mmproj-LFM3-Audio-2B-Q8_0.gguf`, llama.cpp `b12549`, `-t 6 -c 4096`, temperature 0, prompt "Perform ASR in English."
- Script: [`transcribe-wav.mjs`](../../spikes/speech-latency/transcribe-wav.mjs), which scores like `asr-benchmark.mjs`.

## Method

Protocol in [`mic-test.md`](../../spikes/speech-latency/mic-test.md): eight sentences, one hold of Ctrl+Alt+Space each, transcribe each recording right after it is made. Record the room and microphone used.

## Results

Room / microphone: _to fill in_

| # | Reference | Transcript | Word errors | WER | Latency (s) |
|---|---|---|---|---|---|
| 1 | What does this button do? | | | | |
| 2 | Where is the settings menu? | | | | |
| 3 | How do I export this project as a video file? | | | | |
| 4 | Can you explain this error message? | | | | |
| 5 | Show me how to change the font size. | | | | |
| 6 | What is the blue icon in the top right corner? | | | | |
| 7 | Why is this window not responding? | | | | |
| 8 | Read me the title of this page. | | | | |

Overall WER: _to fill in_

## Analysis

_Pending._

## Conclusion

_Pending._

## Eval potential

If kept, the recorded WAVs plus references form a real-speech regression set. Do not commit the audio without the owner's consent (private voice).

## Reproduce

See the protocol in `spikes/speech-latency/mic-test.md`.

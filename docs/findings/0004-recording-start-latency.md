# 0004. Why the first recording lost a second, and the fix

- Date: 2026-11-20
- Status: done
- Confidence: measured (one machine, 9 trials across 5 launches, including a control rebuild)
- Decisions informed: [ADR 0010](../adr/0010-record-microphone-only-while-key-held.md)

## Question

A held push-to-talk key of about 2 s produced a 1.3 s recording (PR #12). Where does the missing time go, and can we remove it without opening the microphone outside a key press?

## Hypothesis

Written before measuring: the audio stream takes a fixed time to start after every key press.

## Setup

- Machine: Intel i7-10750H, Windows 11, Realtek HD Audio microphone.
- App: debug build of kutty-jarvis, cpal 0.15 on WASAPI.
- Instrumentation added to the recorder: time from key down to stream ready, to first audio sample, and to key release. The values travel in the `recording-finished` event and are shown in the window.
- Input: a simulated Ctrl+Alt+Space hold of exactly 2000 ms (`keybd_event`), read back over the WebView debug port.

## Method

1. Three holds in one launch of the original build.
2. Add a warm-up that looks up the default input device and its settings at app start, without building or starting a stream. Two fresh launches, two holds each.
3. Control: rebuild with the warm-up commented out, two fresh launches, one hold each.

## Results

| Build | Launch | Trial | Held | Recorded | First audio after |
|---|---|---|---|---|---|
| no warm-up | 1 | 1 | 2.0 s | 0.8 s | **1.3 s** |
| no warm-up | 1 | 2 | 2.0 s | 1.9 s | 0.1 s |
| no warm-up | 1 | 3 | 2.0 s | 1.9 s | 0.1 s |
| warm-up | A | 1 | 2.0 s | 1.9 s | 0.2 s |
| warm-up | A | 2 | 2.0 s | 1.9 s | 0.1 s |
| warm-up | B | 1 | 2.1 s | 1.9 s | 0.2 s |
| warm-up | B | 2 | 2.0 s | 1.9 s | 0.1 s |
| no warm-up (control) | C | 1 | 2.0 s | 0.9 s | **1.2 s** |
| no warm-up (control) | D | 1 | 2.0 s | 0.9 s | **1.2 s** |

Raw data: [`data/0004-recording-start-latency.json`](data/0004-recording-start-latency.json).

## Analysis

- The hypothesis was wrong. The delay is not per press: it is a one-time cost on the **first** recording after launch (1.2 to 1.3 s). Later presses start in about 0.1 s.
- Looking up the default device and its settings at launch moves that cost out of the first press: first audio after 0.2 s on both fresh launches. The control rebuild, run under the same conditions, went back to 1.2 s, so the warm-up is the cause, not a warm system.
- The warm-up does not build or start a stream, so the microphone is not opened at launch (the privacy rule in ADR 0010 holds). We did not check whether Windows briefly shows its microphone-in-use indicator during the lookup.
- Even when warm, recordings are about 0.1 s shorter than the hold: the stream takes that long to deliver its first buffer.

## Conclusion

The gap was a cold start of the Windows audio device on the first recording. A device lookup at launch removes it (first audio after 0.2 s instead of 1.2 to 1.3 s) without opening the microphone.

## Eval potential

- Regression check: first-press first-audio time under 0.5 s on a fresh launch; recorded duration within 0.3 s of the held duration.
- Needs a real input device, so it suits a manual or on-device test, not CI.

## Reproduce

Build the debug app, launch it with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, hold Ctrl+Alt+Space for 2 s, and read the recording status line.

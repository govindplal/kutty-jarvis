# 0010. Record the microphone only while the push-to-talk key is held, in memory only

- Status: accepted
- Date: 2026-11-20
- Deciders: Claude Code, owner informed
- Evidence: 18 Rust tests (downmix, resampling, WAV round trip); end-to-end run in the built app: holding the key produced a recording shown in the window (1.3 s, 39 KB, 16 kHz mono 16-bit).

## Context

The buddy listens for a question. Microphone access is the most sensitive capability the app has: an always-on microphone is exactly the "listening all the time" feeling we promised to avoid. The speech spike (finding 0002) showed that LFM3-Audio recognises clean 16 kHz mono speech with low word error, so that is the format we should produce.

## Options considered

1. **Always-on listening with a wake word.** Hands-free, but the microphone is open all the time. Rejected for v1 (ADR 0008 already rejected the wake word).
2. **Record while the hotkey is held, keep in memory.** The microphone is open only while the user holds Ctrl+Alt+Space. The audio exists only until the next recording, is never written to disk, and is never sent anywhere automatically.
3. **Record while held, save WAV files for debugging.** Useful, but leaves a trail of private speech on disk. Rejected for the shipped app.

## Decision

- Recording starts on a real push-to-talk press and stops on release, the same transitions that already capture the screen (ADR 0009). The capture thread owns the audio stream, and dropping it closes the microphone.
- The default input device is used at its native format. Samples are converted to `f32`, downmixed to mono, and resampled to **16 kHz**, the format that finding 0002 tested.
- The result is encoded as a 16-bit mono WAV in memory (`hound`) and kept as the latest recording. Nothing is written to disk.
- The UI shows only the duration and size, so the user can see that the microphone was used.

## Consequences

- Good: microphone access is bounded by the key, and a test can check the output format. The 16 kHz mono input matches the tested speech model.
- Cost: the first recording may miss a few milliseconds while the stream starts. Resampling is simple linear interpolation, which is adequate for speech but not for music.
- Measured gap: a held key of about 2 s produced a 1.3 s recording. The missing time is probably stream start-up after the key goes down; not yet measured precisely. Speech quality was not tested either: the run used room noise, not a spoken question.
- Not yet covered: a microphone-in-use indicator in the tray, a first-run privacy notice, device selection in settings, and choosing a device other than the default.
- Revisit if we add a wake word or continuous listening; that would need a new ADR and a visible indicator.

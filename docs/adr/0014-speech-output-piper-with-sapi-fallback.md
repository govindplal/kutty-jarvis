# 0014. Speak replies with Piper, downloaded on first use, and Windows SAPI as fallback

- Status: accepted
- Date: 2026-11-27
- Deciders: Claude Code (speech output agent); owner to review in the PR
- Evidence: [ADR 0007](0007-speech-stack.md), [finding 0002](../findings/0002-local-speech-on-cpu.md), [research 0002](../research/0002-speech-voice-licences.md), [third-party notices](../third-party-notices.md)

## Context

ADR 0007 chose Piper with the public-domain `en_US-ljspeech-medium` voice as the default speech output. The app now needs the code behind it: something the model loop can call with a reply and that stops when the user presses the hotkey again. Constraints:

- Piper's 2023 build links GPL-3.0 espeak-ng, so it must stay a separate process and must not be bundled in the MIT installer.
- Engine (about 22 MB) plus voice (63.5 MB) is a download of roughly 90 MB.
- The first spoken reply must not be silent or fail while that download runs, or when the user is offline.
- Model, speech-to-text and text-to-speech sit behind interfaces (ADR 0004).

## Options considered

1. **Download at install or first start, without being asked.** Voice ready by the first reply. Cost: 90 MB the user never requested, before they have used a voice feature.
2. **Download on first spoken reply; speak with Windows SAPI until it finishes.** The download happens only once the user triggers a spoken reply. The first replies sound robotic but work.
3. **Ask in a dialog.** Needs settings UI that does not exist yet.
4. **SAPI only.** Zero download; poor quality, and nothing for macOS/Linux later.
5. **Bundle Piper.** Rejected: GPL espeak-ng inside the installer.

## Decision

We take option 2. `tts::speak` picks Piper when it is installed in `<app local data>/piper/`. If not, it starts one background download (once per app run), and speaks with SAPI meanwhile. A failed download is not retried until the next app start and SAPI keeps working. `KJ_TTS_DOWNLOAD=0` turns the download off (SAPI only).

Design:

- **Interface.** `tts::TextToSpeech` turns text into a WAV; Piper implements it. Playback is shared, so a future cloud engine only needs `synthesize`. SAPI speaks to the device itself, so it is a separate path with no WAV.
- **Piper** runs as a child process (no console window on Windows): text on stdin, raw 16-bit PCM on stdout, wrapped into an in-memory WAV with the sample rate from the voice's config. Nothing is written to disk. Synthesis is not interruptible; a stop during synthesis suppresses playback instead.
- **Playback** uses cpal on the default output device. Mono 16-bit audio at the voice's rate is resampled linearly to the device rate and copied to every device channel (reusing the recorder's helpers).
- **One utterance at a time, barge-in.** `speak` cancels the current utterance, and a newer one waits for the output to be released; `stop` cancels without replacing. A cancelled utterance that never reached the speakers emits no event.
- **Downloads** go to `<name>.part`, are hashed while streaming, compared with a pinned SHA-256, and renamed into place only on a match. The engine is extracted into a staging directory and renamed atomically. Zip entries with absolute paths, drive prefixes or `..` are rejected, and only regular files are written.
- **Pins.** Engine: `rhasspy/piper` release `2023.11.14-2` (`piper_windows_amd64.zip`, plus `piper_linux_x86_64.tar.gz` for development). Voice files: `rhasspy/piper-voices` commit `5ec43b186ef915a195d3ddbcfd9916e76c503c3f`. The `MODEL_CARD` is kept beside the voice. Hashes are in `tts/assets.rs`: the `.onnx` equals the Hugging Face LFS `oid`; the two small files equal the git blob ids reported by the tree API; engine archives were hashed after download (GitHub shows no digest for this old release).
- **Events** for the model loop and UI: `speech-started` (with `engine` and, for SAPI, the `fallback` reason), `speech-finished` (`interrupted`), `speech-failed` (`error`) and `speech-download` (`started`, `ready`, `failed`).
- **API.** `tts::speak(&AppHandle, String)` (non-blocking), `tts::stop(&AppHandle)`. State is managed by Tauri (`tts::init` in setup). Debug builds add `debug_speak` and `debug_stop_speaking` and a "Speech (debug build)" panel.

## Consequences

- Good: no surprise download; replies speak from day one; the engine stays outside the installer; the engine and voice are verified before use.
- Good: playback and barge-in are independent of the engine, so a cloud or LFM3 engine slots in beside Piper.
- Bad / costs: the first replies use SAPI's voice; the download has no progress UI yet (only the event); no resume of partial downloads; a Piper process starts for every utterance (about 1 s cold, see the real-run numbers in the PR). ureq with rustls adds `ring` to the Windows build.
- Bad / costs: Piper's 2023 binary is archived and unmaintained; LJSpeech compatibility with it was confirmed only on Linux, and SAPI/playback only compile-checked on Windows until a human tests the debug panel.
- What would make us revisit: an offline-first requirement (bundle a smaller voice licensed for it), a persistent Piper process to cut per-utterance startup, or the maintained GPL Piper replacing the archived binary.

## Licences

The voice is public domain (LJSpeech). Piper is MIT, but the 2023 binary bundles GPL-3.0 espeak-ng. We never ship or link it: the user's machine downloads it from the upstream release and the app runs it as a separate process. See [third-party notices](../third-party-notices.md).

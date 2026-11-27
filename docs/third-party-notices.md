# Third-party notices

Components that kutty-jarvis downloads or runs but does not ship in its installer.

## Piper text-to-speech engine (downloaded on first use)

- Project: [rhasspy/piper](https://github.com/rhasspy/piper), release `2023.11.14-2`. Piper itself is MIT-licensed.
- The prebuilt binary bundles [espeak-ng](https://github.com/espeak-ng/espeak-ng), which is GPL-3.0. We do not include the binary or link it into the app: it is downloaded from the upstream release (SHA-256 pinned) into the user's local app data and started as a separate process. Its licence texts come with the download.

## Voice `en_US-ljspeech-medium` (downloaded on first use)

- From [rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices), trained on the LJSpeech dataset, which is public domain. The voice's `MODEL_CARD` is saved next to it.
- Voices derived from the Lessac dataset are not used: their licence restricts use and redistribution. See [research 0002](research/0002-speech-voice-licences.md).

## Windows SAPI

Uses the speech voices already installed with Windows; nothing is bundled.

See [ADR 0014](adr/0014-speech-output-piper-with-sapi-fallback.md).

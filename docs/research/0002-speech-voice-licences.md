# Research 0002. Licences for local speech output (Piper, Lessac voices, alternatives)

- Date: 2026-11-27 (all URLs accessed on this date)
- Status: done
- Feeds: [ADR 0007](../adr/0007-speech-stack.md), [finding 0002](../findings/0002-local-speech-on-cpu.md)
- Not legal advice. Confidence tags: **[sourced]** read in a primary source, **[inferred]** our reading of a source, **[assumed]** not verified.

## Summary

1. The Lessac voices (`en_US-lessac-medium`, `-low`) descend from a dataset licensed for **research only**, with no commercial use and no redistribution. They are the wrong default for a free MIT app that ships to the public. Do not bundle them, and do not make them the first-run download.
2. Piper voices trained on public-domain or CC0 recordings exist. `en_US-ljspeech-medium` (public domain, trained from scratch, 63.5 MB) is the closest drop-in replacement for lessac-medium.
3. Piper's engine side is GPL either way: the archived MIT repo links espeak-ng (GPL-3.0), and the maintained successor is GPL-3.0. Running it as a separate process, talking over stdin/stdout and files, is the standard way to keep GPL out of our MIT code. We ship no GPL code ourselves if the binary is downloaded on first run.
4. Recommendation: Piper 2023.11.14-2 Windows binary plus `en_US-ljspeech-medium`, both downloaded on first run with a pinned checksum, with the `MODEL_CARD` kept next to the voice. Details and caveats below.

## 1. Lessac voices and the Blizzard 2013 dataset

| Claim | Source | Confidence |
|---|---|---|
| The lessac medium and low model cards name the dataset `lessac_blizzard2013` and link its licence page. "Trained from scratch." | https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/lessac/medium/MODEL_CARD and `.../lessac/low/MODEL_CARD` | sourced |
| The `rhasspy/piper-voices` repository metadata says `license: mit`, but the README has no per-voice statement. The model card `License` line covers only the recordings. | https://huggingface.co/rhasspy/piper-voices ; https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/LICENSING.md | sourced |
| The dataset licence is a "Research Licence Agreement" between Voice Factory International, Lessac Technologies and the named user. It grants a "non-exclusive and non-transferable licence, without the right to sub-licence, to use the Materials exclusively for Research Purposes only" (cl. 3.2). | https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html | sourced |
| "Research Purposes" excludes any commercial use "including the development, marketing, commercialisation, sale or licencing of voice synthesis or speech recognition products or services" (cl. 1.1). | same | sourced |
| The user must not "lend, hire, sell, distribute or otherwise part with the Materials" (cl. 7) and may not let third parties use them (cl. 8). The licence is granted only to the person or organisation named on the form. | same | sourced |
| Piper's own docs say lessac-derived voices are limited to "research and exploration of speech technologies" and that Piper has no permission from Lessac Technologies or CSTR; commercial or redistribution rights would need a separate agreement. | https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/LICENSING.md | sourced |
| Whether trained model weights count as "Materials" or a derivative is unsettled. Piper "takes no position". | same | sourced |

Answers to the questions asked:

- **Can a free MIT app download and use them?** Unclear and risky. The licence is per named person, for research. A user tinkering personally is arguably doing "exploration", but our app would hand the voice to every user without the licence form, and we cannot grant sublicences. [inferred]
- **Can it bundle them?** No. Bundling is redistribution, which cl. 7 forbids without written consent, and Piper's docs say the same. [inferred from sourced text]
- **Is commercial use allowed?** No, not without a separate agreement. [sourced]
- **Does "download on first run" fix it?** It avoids us redistributing the file, but not the users' own right to use it, and it ties our default to a licence we cannot confirm. Not recommended as the default. [inferred]
- Many other voices are fine-tuned from lessac and inherit the doubt, for example `hfc_male`, `kusal` and `libritts_r` (cards say "Finetuned from U.S. English lessac"). Check the `Training` line of every card. Piper says some voices were retrained away from lessac while keeping the old file name, so check the copy actually downloaded. [sourced]

## 2. Piper engine

| Item | Finding | Source | Confidence |
|---|---|---|---|
| `rhasspy/piper` | MIT, archived read-only on 2025-10-06, "Development has moved" to OHF-Voice/piper1-gpl. Last release `2023.11.14-2` (2023-11-14) with `piper_windows_amd64.zip`. | https://github.com/rhasspy/piper ; its releases list | sourced |
| `OHF-Voice/piper1-gpl` | GPL-3.0, "embeds espeak-ng for phonemization". Releases up to v1.8.0 (2026-09-04) are Python wheels (`piper_tts-1.8.0-cp39-abi3-win_amd64.whl`) and an sdist, no standalone `.exe`. Voices download via `python -m piper.download_voices`. | https://github.com/OHF-Voice/piper1-gpl ; https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/CLI.md | sourced |
| Old MIT repo and espeak-ng | The old build installs `espeak-ng-data` and links espeak-ng (its `CMakeLists.txt` references both). espeak-ng is GPL-3.0. So the "MIT" Windows zip is in practice a combined work that includes GPL parts. | https://github.com/rhasspy/piper (CMakeLists.txt) ; https://github.com/espeak-ng/espeak-ng (COPYING is GPL v3) | CMake lines sourced; "combined GPL" reading inferred |
| Newer voices | Newer voice cards say "Requires Piper 1.5 or higher", so they need the GPL engine and not the 2023 binary. The `ljspeech` and `lessac` cards carry no such note. | model cards in huggingface.co/rhasspy/piper-voices | sourced; that ljspeech runs on 2023.11.14-2 is **assumed**, not tested |

What running a prebuilt binary as a separate process means for our MIT code:

- The GNU FAQ says "pipes, sockets and command-line arguments are communication mechanisms normally used between two separate programs", so such programs are normally separate works (https://www.gnu.org/licenses/gpl-faq.html, "mere aggregation" entry). Our Tauri app launching `piper.exe` and exchanging text and WAV data is that pattern. [sourced rule, inferred application]
- Our repo stays MIT. We must not copy or link Piper or espeak-ng code into it. If we ever ship the binary ourselves we owe GPL-3.0 obligations for it (licence text, source offer). Downloading it on first run from upstream avoids that. [inferred]
- Keep a third-party notice listing Piper, espeak-ng and the voice with licences and URLs.

Which binary to pin:

- Preferred for v1: `rhasspy/piper` release `2023.11.14-2`, `piper_windows_amd64.zip`. It is a standalone executable, already benchmarked in finding 0002, and matches the existing spike. Downside: frozen, unmaintained, cannot run newer voices. Pin the SHA-256 of the zip (to be computed when the downloader is written; not recorded here).
- Later alternative: `OHF-Voice/piper1-gpl` v1.8.0 Windows wheel. It needs a Python runtime or an extracted wheel, so it is a heavier dependency we have not tried. Revisit for newer voices (joe, kathleen) or fixes.
- Uncertainty: we have not checked whether the 2023 zip ships a separate licence file for espeak-ng, nor how long upstream keeps an archived repo's release assets available. [assumed]

## 3. Permissive alternatives

Piper voices (Hugging Face `rhasspy/piper-voices`, cards read 2026-11-27; Piper's own list of commercially usable voices is https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/COMMERCIAL_USE.md):

| Voice | Dataset licence | Size | Notes | Confidence |
|---|---|---|---|---|
| `en_US-ljspeech-medium` | public domain (LJ Speech; https://keithito.com/LJ-Speech-Dataset/ says it is in the public domain and "you may use it without attribution") | 63.5 MB | trained from scratch, single female voice, 22.05 kHz; on Piper's commercially usable list | sourced |
| `en_US-ljspeech-high` | public domain | 114 MB | same data, bigger model | sourced |
| `en_US-norman-medium` | public domain (LibriVox) | 63.5 MB | from scratch, male voice | sourced |
| `en_US-kristin-medium`, `-john-medium` | public domain | not checked | on the commercial list | sourced (list) |
| `en_US-joe-medium` | CC0 | 63.2 MB | needs Piper 1.5+ (GPL wheel) | sourced |
| hifi_tts_92-based voices (e.g. `en_GB-alba`) | CC BY 4.0, attribution required, also for the base dataset | about 63 MB | needs Piper 1.5+ | sourced |
| `en_US-ryan`, `hfc_female`, `hfc_male` | CC BY-NC-SA 4.0 | about 63 MB | non-commercial, avoid | sourced |
| `en_US-amy-medium` | "See URL" (Mimic3 voices, no documented licence) | not checked | avoid | sourced |

Other small CPU engines:

| Engine | Licence | Size | Notes | Confidence |
|---|---|---|---|---|
| Kokoro-82M (`hexgrad/Kokoro-82M`; ONNX at `onnx-community/Kokoro-82M-v1.0-ONNX`) | Apache-2.0 weights; card says trained on permissive or non-copyrighted audio. The `kokoro-onnx` wrapper is MIT | ONNX variants 86 MB (`q8f16`), 92 MB (`quantized`), 326 MB (fp32), plus small voice files | https://huggingface.co/hexgrad/Kokoro-82M ; https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX ; https://github.com/thewh1teagle/kokoro-onnx | sourced. Whether its phonemizer needs espeak-ng (GPL) was **not checked**; CPU speed not measured |
| Windows SAPI | OS component, nothing to redistribute | 0 | already the ADR 0007 fallback | sourced (ADR) |

## 4. Recommendation

- **Default engine:** Piper `2023.11.14-2` Windows amd64 binary, as a separate process, downloaded on first run.
- **Default voice:** `en_US-ljspeech-medium` (public domain, trained from scratch). The lessac voices stop being the default. Expect similar speed to lessac-medium because the model size is the same (63.5 MB vs 63.2 MB) [assumed; repeat the RTF benchmark before claiming it]. Voice quality needs a listening test; ljspeech is a single audiobook-style narrator.
- **Bundle or download:** download on first run, verify a pinned SHA-256, store under the app data folder, and keep the voice's `MODEL_CARD` and the engine's licence files beside it. Do not bundle the engine or any voice in the installer, so no GPL binary or voice licence enters our distribution.
- **Keep the interface swappable (ADR 0004):** Kokoro is the candidate second engine if ljspeech quality disappoints, after a speed and espeak-ng dependency check.
- **Not for shipping:** the Lessac voices and everything fine-tuned from them. They may stay in local spikes for benchmarking, never downloaded by the app.
- A written reply from Lessac Technologies or CSTR would be needed to use lessac legitimately; unlikely to be worth pursuing.

## Open items

- Compute and record the SHA-256 of the pinned Piper zip and the ljspeech `.onnx` and `.onnx.json`.
- Run `en_US-ljspeech-medium` on the 2023.11.14-2 binary (compatibility assumed) and repeat the RTF measurement.
- Blind listening test (ljspeech vs lessac vs Kokoro).
- Check espeak-ng licence files inside the 2023 zip and add a third-party notice.

# 0002. Local speech recognition and synthesis on a CPU

- Date: 2026-11-20
- Status: done
- Confidence: measured, but on clean synthetic speech, one machine and six utterances; treat accuracy as an upper bound
- Decisions informed: [ADR 0007](../adr/0007-speech-stack.md), [ADR 0004](../adr/0004-local-first-provider-agnostic-models.md)

## Question

On a CPU-only laptop, can local models transcribe a spoken question accurately and speak a reply fast enough for a conversational buddy? Which engines are viable?

## Hypothesis

Written before running: Liquid's LFM audio model would handle recognition well (it is purpose-built), but generating speech with a 2B language model on CPU might be too slow for streaming, so a small dedicated synthesizer would be needed.

## Setup

- Machine: Intel i7-10750H (6 cores / 12 threads), 15.8 GB RAM, Windows 11, no GPU.
- Runtime: llama.cpp `b12549` (`llama-server`, `llama-tts`), `win-cpu-x64`.
- Recognition and synthesis model: `LiquidAI/LFM3-Audio-2B-GGUF`, `Q4_K_M` (1.10 GB) with `mmproj-...-Q8_0` (304 MB). Chosen over LFM2.5-Audio because the llama.cpp path for LFM3 is supported in released builds and exposes ASR and TTS.
- Comparison synthesizer: Piper (`rhasspy/piper` 2023.11.14-2 Windows build, MIT) with voices `en_US-lessac-medium` and `en_US-lessac-low` (63 MB each).
- Recognition test set: six spoken questions generated with the default Windows voice (SAPI) at 16 kHz mono by [`make-speech-samples.ps1`](../../spikes/speech-latency/make-speech-samples.ps1); each has known reference text.
- Server flags for recognition: `-t 6 -c 4096`, temperature 0, system prompt `Perform ASR in English.`

## Method

1. Recognition: send each WAV to `llama-server` as `input_audio`, compute word error rate (WER) against the reference ([`asr-benchmark.mjs`](../../spikes/speech-latency/asr-benchmark.mjs)).
2. Synthesis: for a 9-word and a 31-word reply, run each engine as a fresh process three times and record wall time and audio length ([`tts-benchmark.py`](../../spikes/speech-latency/tts-benchmark.py)). Wall time therefore includes process start and model load.
3. Model downloads were verified against `Content-Length` ([`fetch-hf-file.sh`](../../spikes/speech-latency/fetch-hf-file.sh)).

## Results

### Recognition (LFM3-Audio-2B Q4_K_M, persistent server)

| Metric | Value |
|---|---|
| Word error rate | 0.0% (0 errors in 62 words) |
| Mean latency per utterance | 1.36 s for 3.3 to 4.9 s of audio |
| Real-time factor | 0.35 |

Raw output: [`data/0002-asr-lfm3-audio-q4km.json`](data/0002-asr-lfm3-audio-q4km.json). A one-off CLI run including model load took 7.1 s for the first sample.

### Synthesis (fresh process each run, 3 repeats, RTF = wall / audio, lower is better)

| Engine | Reply | Mean wall | Audio | RTF (min / median) |
|---|---|---|---|---|
| LFM3-Audio-2B Q4_K_M | short (9 words) | 5.81 s | 3.04 s | 1.97 / 2.09 |
| LFM3-Audio-2B Q4_K_M | long (31 words) | 13.27 s | 9.92 s | 1.35 / 1.36 |
| Piper lessac-medium | short | 0.63 s | 2.64 s | 0.24 / 0.25 |
| Piper lessac-medium | long | 1.13 s | 8.65 s | 0.13 / 0.13 |
| Piper lessac-low | short | 0.60 s | 2.60 s | 0.21 / 0.22 |
| Piper lessac-low | long | 1.02 s | 8.54 s | 0.11 / 0.12 |

Raw output: [`data/0002-tts-benchmark.json`](data/0002-tts-benchmark.json). Inside the LFM3 process the log reported about 0.9 to 1.0 times real time for generation alone (3.2 s of audio in 3.64 s; 9.76 s in 10.2 s), so roughly 2.5 s of the wall time above is model load.

## Analysis

- Recognition is not the bottleneck: it is faster than real time and was perfect here. The caveat is large: the audio was clean, synthetic, single-voice and the questions were simple. Real microphones, accents and background noise will cost accuracy. This needs a noisy, human-recorded set before we trust it.
- LFM3 synthesis is slightly slower than real time even with the model loaded, so it cannot stream a reply without gaps and a user would wait seconds before hearing anything. A dedicated synthesizer is about 10 times faster.
- Piper's first run in an earlier ad-hoc test (about 2.3 s) was a cold start; warm runs are about 0.6 s. A persistent process would remove most of that.
- **Not measured:** voice quality (we did not listen systematically), memory use of each engine, CPU load while the vision model also runs, and noisy-speech accuracy.

## Conclusion

Use LFM3-Audio for local speech recognition. For local speech output use a small dedicated synthesizer such as Piper; LFM3's own speech output is too slow for conversation on CPU, though it may suit a "high quality, wait for it" mode. A full local round trip is dominated by vision (about 8 s, finding 0001), not speech.

## Licensing notes (unresolved)

- Piper's original repository is MIT but archived; its maintained successor is GPL-3.0. Running the old binary as a separate process keeps GPL out of our code; confirm before shipping.
- The Lessac voices are trained on the Blizzard 2013 Lessac dataset, which has its own licence terms we have not reviewed. Do not bundle a voice until its licence is checked; download on first run instead.
- LFM weights are under the LFM Open License v1.0 (see ADR 0004).

## Eval potential

- Recognition eval: audio clips with reference text; metric WER; add noisy and accented clips; set the gate after a human-recorded baseline exists.
- Synthesis eval: RTF per engine per reply length, gated on RTF below 0.5 for the default engine; quality via blind listening or a MOS-style rating, tracked separately.

## Reproduce

```bash
powershell -File spikes/speech-latency/make-speech-samples.ps1 -OutDir ./data/speech
llama-server -m LFM3-Audio-2B-Q4_K_M.gguf --mmproj mmproj-LFM3-Audio-2B-Q8_0.gguf -t 6 -c 4096 --port 8090
node spikes/speech-latency/asr-benchmark.mjs data/speech http://127.0.0.1:8090 --out asr.json
python spikes/speech-latency/tts-benchmark.py --llama-dir <llama> --lfm-model <gguf> --lfm-mmproj <mmproj> \
  --piper-exe <piper.exe> --piper-voice <voice.onnx> --out tts.json
```

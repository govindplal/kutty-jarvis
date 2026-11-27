# Setting up the local models

The buddy answers with two local models served by `llama-server` (ADR [0012](../adr/0012-model-loop-llama-server-sidecar.md)). Nothing is downloaded automatically yet; put the files in place once.

## Folder layout

The app looks under its local data folder, on Windows `%LOCALAPPDATA%\dev.kuttyjarvis.app\`:

```
llama\llama-server.exe        (plus the DLLs from the same zip)
models\LFM3-Audio-2B-Q4_K_M.gguf
models\mmproj-LFM3-Audio-2B-Q8_0.gguf
models\LFM2.5-VL-1.6B-Q4_K_M.gguf
models\mmproj-LFM2.5-VL-1.6b-Q8_0.gguf
```

## Where to get them

- **llama-server:** llama.cpp release `b12549` (the version used in the findings), `llama-b12549-bin-win-cpu-x64.zip` from <https://github.com/ggml-org/llama.cpp/releases/tag/b12549>. Unzip all of it into `llama\`.
- **Speech recognition (about 1.4 GB):** `LFM3-Audio-2B-Q4_K_M.gguf` and `mmproj-LFM3-Audio-2B-Q8_0.gguf` from <https://huggingface.co/LiquidAI/LFM3-Audio-2B-GGUF>.
- **Vision (about 1.3 GB):** `LFM2.5-VL-1.6B-Q4_K_M.gguf` and `mmproj-LFM2.5-VL-1.6b-Q8_0.gguf` from <https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF>.

[`spikes/speech-latency/fetch-hf-file.sh`](../../spikes/speech-latency/fetch-hf-file.sh) downloads one Hugging Face file with resume and size check. Check each model's licence on its Hugging Face page before use.

## Environment variables

All optional. The app reads them at start-up and on each server start.

| Variable | Meaning |
|---|---|
| `KJ_LLAMA_SERVER` | Full path of `llama-server`; replaces `llama\llama-server(.exe)`. |
| `KJ_MODELS_DIR` | Folder holding the four GGUF files; replaces `models\`. |
| `KJ_ASR_MODEL`, `KJ_ASR_MMPROJ` | Full paths of the speech model and its projector. |
| `KJ_VLM_MODEL`, `KJ_VLM_MMPROJ` | Full paths of the vision model and its projector. |
| `KJ_ASR_URL`, `KJ_VLM_URL` | Use a server you already started (for example `http://127.0.0.1:8090`) instead of starting one. The app never stops these. |
| `KJ_VLM_IMAGE_EDGE` | Longest edge in pixels of the screenshot sent to the vision model (default 960). Smaller is faster. |

## How it behaves

- The first question after start-up loads the models (a few seconds, longer on a cold disk). Each server stays loaded until the app quits.
- The servers listen on `127.0.0.1` only, on a free port chosen at start.
- If a file is missing, the app shows which folder and file name it expected.

## Trying the loop without the app

```
python3 app/src-tauri/examples/mock_screen.py /tmp/mock.png
cd app/src-tauri
KJ_PNG=/tmp/mock.png KJ_WAV=/tmp/question.wav KJ_DATA_DIR=<folder with llama\ and models\> cargo run --release --example ask
```

`KJ_WAV` must be 16 kHz mono 16-bit PCM; set `KJ_TRANSCRIPT="..."` instead to skip speech recognition.

# Real-microphone speech test (finding 0005)

Goal: check that LFM3-Audio recognises your voice through the real microphone, using the WAV that debug builds of the app save after each recording.

## Sentences

Say each one in a normal voice, at your normal distance from the microphone, in your usual room.

1. What does this button do?
2. Where is the settings menu?
3. How do I export this project as a video file?
4. Can you explain this error message?
5. Show me how to change the font size.
6. What is the blue icon in the top right corner?
7. Why is this window not responding?
8. Read me the title of this page.

## Steps

You need three terminals or one terminal used in turn. Close other heavy apps first (the model needs about 2 GB of RAM).

1. Build and start the debug app (from the repo root):
   ```
   cd app
   npm install
   npm run tauri dev
   ```
   Wait for the window. A tray icon appears.
2. For sentence N: hold **Ctrl+Alt+Space**, say the sentence, release. The window shows `Recorded ... Debug copy: <path>`. The file is `%TEMP%/kutty-jarvis/last-recording.wav` and is overwritten by the next recording, so do step 3 before recording the next sentence.
3. Run the script with the exact sentence as reference (from the repo root, quotes included):
   ```
   node spikes/speech-latency/transcribe-wav.mjs --ref "What does this button do?"
   ```
   With no file argument it reads the last recording. The first run starts llama-server (about 20 s to load) and stops it when done. To keep the server loaded between sentences, start it yourself once and the script will reuse it:
   ```
   .tools/llama/llama-server.exe -m .models/LFM3-Audio-2B-Q4_K_M.gguf --mmproj .models/mmproj-LFM3-Audio-2B-Q8_0.gguf -t 6 -c 4096 --port 8090
   ```
4. Copy the transcript, latency and WER line for each sentence into the results table of [finding 0005](../../docs/findings/0005-real-microphone-speech-recognition.md).
5. If a recording is empty or cut off at the start, note it: that is a finding too (see finding 0004 for the start-up gap).

Note that the script takes the path of the llama checkout from `KJ_MAIN` (default `C:/Users/govin/repos/portfolios/jarvis`).

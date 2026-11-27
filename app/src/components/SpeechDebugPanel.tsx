import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  SAMPLE_TEXT,
  describeDownload,
  describeSpeech,
  type DownloadEvent,
  type SpeechEvent,
} from "../lib/speech";

async function debugCommandsAvailable(): Promise<boolean> {
  try {
    // Only debug builds register this command; release builds reject it.
    return (await invoke<boolean>("debug_speech_available")) === true;
  } catch {
    return false;
  }
}

/**
 * Debug builds only: speak a text through the normal engine choice (Piper,
 * else the Windows voice) and interrupt it. Renders nothing in release builds.
 */
export function SpeechDebugPanel() {
  const [available, setAvailable] = useState(false);
  const [text, setText] = useState(SAMPLE_TEXT);
  const [status, setStatus] = useState("");

  useEffect(() => {
    let live = true;
    debugCommandsAvailable().then((ok) => {
      if (live) setAvailable(ok);
    });
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    if (!available) return;
    const unlisten = ["speech-started", "speech-finished", "speech-failed"].map((name) =>
      listen<SpeechEvent>(name, (event) => setStatus(describeSpeech(name, event.payload))),
    );
    const download = listen<DownloadEvent>("speech-download", (event) =>
      setStatus(describeDownload(event.payload)),
    );
    return () => {
      for (const pending of [...unlisten, download]) pending.then((off) => off());
    };
  }, [available]);

  if (!available) return null;

  async function speak() {
    try {
      await invoke("debug_speak", { text });
      setStatus("Preparing speech…");
    } catch (error) {
      setStatus(`Speaking failed: ${String(error)}`);
    }
  }

  return (
    <section className="speech-debug" aria-label="Speech debug">
      <h2>Speech (debug build)</h2>
      <form
        className="row"
        onSubmit={(event) => {
          event.preventDefault();
          speak();
        }}
      >
        <input
          aria-label="Text to speak"
          value={text}
          onChange={(event) => setText(event.currentTarget.value)}
        />
        <button type="submit" disabled={!text.trim()}>
          Speak
        </button>
        <button type="button" onClick={() => invoke("debug_stop_speaking")}>
          Stop
        </button>
      </form>
      <p role="status" aria-live="polite">
        {status}
      </p>
    </section>
  );
}

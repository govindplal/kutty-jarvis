import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export const PUSH_TO_TALK_EVENT = "push-to-talk";
export type PushToTalkState = "idle" | "listening";

/** Shows whether the push-to-talk hotkey is held. Driven by the Rust hotkey module. */
export function PushToTalkStatus() {
  const [state, setState] = useState<PushToTalkState>("idle");

  useEffect(() => {
    const unlisten = listen<PushToTalkState>(PUSH_TO_TALK_EVENT, (event) => setState(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const listening = state === "listening";
  return (
    <p className={`ptt-status ${listening ? "ptt-listening" : ""}`} role="status" aria-live="polite">
      {listening ? "Listening…" : "Hold Ctrl+Alt+Space to talk"}
    </p>
  );
}

import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const PUSH_TO_TALK_EVENT = "push-to-talk";
export type PushToTalkState = "idle" | "listening";

/** Mirrors `ShortcutStatus` in app/src-tauri/src/hotkey.rs. */
export type ShortcutStatus = { label: string; registered: boolean; notice: string | null };

const DEFAULT_STATUS: ShortcutStatus = { label: "Ctrl+Alt+Space", registered: true, notice: null };

/** Shows whether the push-to-talk hotkey is held, or why it is unavailable. Driven by the Rust hotkey module. */
export function PushToTalkStatus() {
  const [state, setState] = useState<PushToTalkState>("idle");
  const [shortcut, setShortcut] = useState<ShortcutStatus>(DEFAULT_STATUS);

  useEffect(() => {
    const unlisten = listen<PushToTalkState>(PUSH_TO_TALK_EVENT, (event) => setState(event.payload));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    let active = true;
    Promise.resolve(invoke<ShortcutStatus | undefined>("push_to_talk_status"))
      .then((status) => {
        if (active && status) setShortcut(status);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  if (!shortcut.registered) {
    return (
      <p className="ptt-status ptt-unavailable" role="alert">
        {shortcut.notice ?? `${shortcut.label} is used by another app; push-to-talk is off`}
      </p>
    );
  }

  const listening = state === "listening";
  return (
    <>
      <p className={`ptt-status ${listening ? "ptt-listening" : ""}`} role="status" aria-live="polite">
        {listening ? "Listening…" : `Hold ${shortcut.label} to talk`}
      </p>
      {shortcut.notice && <p role="alert">{shortcut.notice}</p>}
    </>
  );
}

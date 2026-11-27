import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  describeAssistant,
  type AssistantFailedPayload,
  type AssistantRepliedPayload,
  type AssistantState,
  type AssistantThinkingPayload,
} from "../lib/assistant";

export const ASSISTANT_THINKING_EVENT = "assistant-thinking";
export const ASSISTANT_REPLIED_EVENT = "assistant-replied";
export const ASSISTANT_FAILED_EVENT = "assistant-failed";

/** Shows what the buddy heard, is working on, and answered. */
export function AssistantStatus() {
  const [state, setState] = useState<AssistantState>({ kind: "idle" });

  useEffect(() => {
    const thinking = listen<AssistantThinkingPayload>(ASSISTANT_THINKING_EVENT, (event) => {
      setState({ kind: "thinking", payload: event.payload });
    });
    const replied = listen<AssistantRepliedPayload>(ASSISTANT_REPLIED_EVENT, (event) => {
      setState({ kind: "replied", payload: event.payload });
    });
    const failed = listen<AssistantFailedPayload>(ASSISTANT_FAILED_EVENT, (event) => {
      setState({ kind: "failed", payload: event.payload });
    });
    return () => {
      thinking.then((stop) => stop());
      replied.then((stop) => stop());
      failed.then((stop) => stop());
    };
  }, []);

  return (
    <div className="capture-status" role="status" aria-live="polite">
      {describeAssistant(state).map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );
}

/** Payloads of the `assistant-*` events (see app/src-tauri/src/assistant/runtime.rs). */
export type AssistantThinkingPayload = {
  stage: "transcribing" | "thinking";
  transcript?: string;
};

export type AssistantRepliedPayload = {
  transcript: string;
  reply: string;
  pointedElement?: { id: number; role: string; name: string };
  transcribeMs: number;
  answerMs: number;
};

export type AssistantFailedPayload = {
  stage: "recording" | "busy" | "transcribe" | "answer";
  message: string;
};

export type AssistantState =
  | { kind: "idle" }
  | { kind: "thinking"; payload: AssistantThinkingPayload }
  | { kind: "replied"; payload: AssistantRepliedPayload }
  | { kind: "failed"; payload: AssistantFailedPayload };

const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;

/** Lines to show for the current state, one per paragraph. */
export function describeAssistant(state: AssistantState): string[] {
  switch (state.kind) {
    case "idle":
      return [];
    case "thinking":
      return state.payload.stage === "transcribing"
        ? ["Listening to what you said..."]
        : [`You said: "${state.payload.transcript ?? ""}"`, "Thinking..."];
    case "replied": {
      const { transcript, reply, pointedElement, transcribeMs, answerMs } = state.payload;
      const lines = [`You said: "${transcript}"`, `Buddy: ${reply}`];
      if (pointedElement) {
        lines.push(`Pointing at ${pointedElement.role} "${pointedElement.name}".`);
      }
      lines.push(`Speech to text ${seconds(transcribeMs)}, answer ${seconds(answerMs)}.`);
      return lines;
    }
    case "failed":
      return [`Could not answer (${state.payload.stage}): ${state.payload.message}`];
  }
}

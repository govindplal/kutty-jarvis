import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  ASSISTANT_FAILED_EVENT,
  ASSISTANT_REPLIED_EVENT,
  ASSISTANT_THINKING_EVENT,
  AssistantStatus,
} from "./AssistantStatus";

type Handler = (event: { payload: unknown }) => void;
const handlers = vi.hoisted(() => new Map<string, Handler>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: Handler) => {
    handlers.set(name, handler);
    return () => {};
  }),
}));

afterEach(() => {
  cleanup();
  handlers.clear();
});

async function setup() {
  render(<AssistantStatus />);
  await act(async () => {});
}

const send = (name: string, payload: unknown) => act(() => handlers.get(name)?.({ payload }));

describe("AssistantStatus", () => {
  it("is empty before any question", async () => {
    await setup();
    expect(screen.getByRole("status").textContent).toBe("");
  });

  it("says it is listening while transcribing", async () => {
    await setup();
    send(ASSISTANT_THINKING_EVENT, { stage: "transcribing" });
    expect(screen.getByRole("status").textContent).toBe("Listening to what you said...");
  });

  it("shows the transcript while the model thinks", async () => {
    await setup();
    send(ASSISTANT_THINKING_EVENT, { stage: "thinking", transcript: "where is save" });
    expect(screen.getByRole("status").textContent).toBe('You said: "where is save"Thinking...');
  });

  it("shows the reply, the pointed element and timings", async () => {
    await setup();
    send(ASSISTANT_REPLIED_EVENT, {
      transcript: "where is save",
      reply: "It is the Save button.",
      pointedElement: { id: 2, role: "button", name: "Save" },
      transcribeMs: 3200,
      answerMs: 12100,
    });
    const text = screen.getByRole("status").textContent;
    expect(text).toContain("Buddy: It is the Save button.");
    expect(text).toContain('Pointing at button "Save".');
    expect(text).toContain("Speech to text 3.2 s, answer 12.1 s.");
  });

  it("does not claim to point when nothing was pointed at", async () => {
    await setup();
    send(ASSISTANT_REPLIED_EVENT, {
      transcript: "hi",
      reply: "Hello!",
      transcribeMs: 1000,
      answerMs: 2000,
    });
    expect(screen.getByRole("status").textContent).not.toContain("Pointing");
  });

  it("explains which step failed", async () => {
    await setup();
    send(ASSISTANT_FAILED_EVENT, { stage: "transcribe", message: "the speech model file is missing" });
    expect(screen.getByRole("status").textContent).toBe(
      "Could not answer (transcribe): the speech model file is missing",
    );
  });

  it("a new question replaces the previous answer", async () => {
    await setup();
    send(ASSISTANT_FAILED_EVENT, { stage: "busy", message: "Still answering" });
    send(ASSISTANT_THINKING_EVENT, { stage: "transcribing" });
    expect(screen.getByRole("status").textContent).toBe("Listening to what you said...");
  });

  it("shows Speaking... until that utterance finishes, ignoring older ones", async () => {
    await setup();
    send("speech-started", { id: 2, engine: "piper" });
    expect(screen.getByRole("status").textContent).toBe("Speaking...");
    send("speech-finished", { id: 1, engine: "piper", interrupted: true });
    expect(screen.getByRole("status").textContent).toBe("Speaking...");
    send("speech-finished", { id: 2, engine: "piper" });
    expect(screen.getByRole("status").textContent).toBe("");
  });
});

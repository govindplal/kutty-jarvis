import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SpeechDebugPanel } from "./SpeechDebugPanel";
import { SAMPLE_TEXT, describeDownload, describeSpeech } from "../lib/speech";

const invoke = vi.hoisted(() => vi.fn());
type Handler = (event: { payload: unknown }) => void;
const handlers = vi.hoisted(() => new Map<string, Handler>());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: Handler) => {
    handlers.set(name, handler);
    return () => handlers.delete(name);
  }),
}));

afterEach(() => {
  cleanup();
  invoke.mockReset();
  handlers.clear();
  lastId = 0;
});

let lastId = 0;

function debugBuild() {
  invoke.mockImplementation(async (command: string) => {
    if (command === "debug_speech_available") return true;
    if (command === "debug_speak") return ++lastId;
    return undefined;
  });
}

describe("SpeechDebugPanel", () => {
  it("renders nothing in a release build", async () => {
    invoke.mockRejectedValue("command debug_speech_available not found");
    const { container } = render(<SpeechDebugPanel />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container.innerHTML).toBe("");
  });

  it("speaks the typed text and can be stopped", async () => {
    debugBuild();
    render(<SpeechDebugPanel />);

    const input = await screen.findByLabelText("Text to speak");
    expect(input).toHaveProperty("value", SAMPLE_TEXT);
    await userEvent.clear(input);
    await userEvent.type(input, "Testing one two");
    await userEvent.click(screen.getByRole("button", { name: "Speak" }));
    expect(invoke).toHaveBeenCalledWith("debug_speak", { text: "Testing one two" });

    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(invoke).toHaveBeenCalledWith("debug_stop_speaking");
  });

  it("does not speak an empty text", async () => {
    debugBuild();
    render(<SpeechDebugPanel />);
    await userEvent.clear(await screen.findByLabelText("Text to speak"));
    expect(screen.getByRole("button", { name: "Speak" })).toHaveProperty("disabled", true);
  });

  it("shows which engine spoke and why", async () => {
    debugBuild();
    render(<SpeechDebugPanel />);
    await screen.findByLabelText("Text to speak");
    await vi.waitFor(() => expect(handlers.has("speech-started")).toBe(true));

    act(() =>
      handlers.get("speech-started")?.({
        payload: { id: 1, engine: "sapi", fallback: "piper is still downloading" },
      }),
    );
    expect(screen.getByRole("status").textContent).toBe(
      "Speaking with sapi (piper is still downloading)",
    );

    act(() => handlers.get("speech-download")?.({ payload: { status: "ready" } }));
    expect(screen.getByRole("status").textContent).toBe("Piper voice ready");
  });
});

describe("SpeechDebugPanel utterance ids", () => {
  it("ignores late events of an earlier utterance", async () => {
    debugBuild();
    render(<SpeechDebugPanel />);
    await screen.findByLabelText("Text to speak");
    await vi.waitFor(() => expect(handlers.has("speech-finished")).toBe(true));
    const emit = (name: string, payload: object) =>
      act(() => handlers.get(name)?.({ payload }));

    await userEvent.click(screen.getByRole("button", { name: "Speak" }));
    emit("speech-started", { id: 1, engine: "piper" });
    await userEvent.click(screen.getByRole("button", { name: "Speak" }));
    expect(screen.getByRole("status").textContent).toBe("Preparing speech…");

    // The first utterance is cut off after the second was requested.
    emit("speech-finished", { id: 1, engine: "piper", interrupted: true });
    expect(screen.getByRole("status").textContent).toBe("Preparing speech…");

    emit("speech-started", { id: 2, engine: "piper" });
    expect(screen.getByRole("status").textContent).toBe("Speaking with piper");
    emit("speech-finished", { id: 1, engine: "piper", interrupted: true });
    expect(screen.getByRole("status").textContent).toBe("Speaking with piper");
    emit("speech-finished", { id: 2, engine: "piper", interrupted: false });
    expect(screen.getByRole("status").textContent).toBe("Finished (piper)");
  });
});

describe("describeSpeech / describeDownload", () => {
  it("describes interruptions, failures and download problems", () => {
    expect(describeSpeech("speech-finished", { id: 1, engine: "piper", interrupted: true })).toBe(
      "Interrupted (piper)",
    );
    expect(describeSpeech("speech-failed", { id: 1, engine: "sapi", error: "no device" })).toBe(
      "Speech failed (sapi): no device",
    );
    expect(describeDownload({ status: "failed", error: "offline" })).toBe(
      "Piper download failed: offline",
    );
  });
});

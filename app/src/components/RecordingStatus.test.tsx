import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RECORDING_FAILED_EVENT, RECORDING_FINISHED_EVENT, RecordingStatus } from "./RecordingStatus";

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

describe("RecordingStatus", () => {
  it("is empty before any recording", () => {
    render(<RecordingStatus />);
    expect(screen.getByRole("status").textContent).toBe("");
  });

  it("shows the duration and size of a finished recording", async () => {
    render(<RecordingStatus />);
    await act(async () => {});
    act(() => handlers.get(RECORDING_FINISHED_EVENT)?.({ payload: { durationMs: 2450, wavBytes: 81920 } }));
    expect(screen.getByRole("status").textContent).toBe(
      "Recorded 2.5 s (80 KB, kept in memory only)",
    );
  });

  it("shows why a recording failed", async () => {
    render(<RecordingStatus />);
    await act(async () => {});
    act(() => handlers.get(RECORDING_FAILED_EVENT)?.({ payload: "no microphone found" }));
    expect(screen.getByRole("status").textContent).toBe("Recording failed: no microphone found");
  });
});

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
    act(() =>
      handlers.get(RECORDING_FINISHED_EVENT)?.({
        payload: { durationMs: 1300, wavBytes: 40960, heldMs: 2000, readyMs: 150, firstSampleMs: 700 },
      }),
    );
    expect(screen.getByRole("status").textContent).toBe(
      "Recorded 1.3 s of 2.0 s held; first audio after 0.7 s (40 KB, kept in memory only)",
    );
  });

  it("says when no audio arrived at all", async () => {
    render(<RecordingStatus />);
    await act(async () => {});
    act(() =>
      handlers.get(RECORDING_FINISHED_EVENT)?.({
        payload: { durationMs: 0, wavBytes: 44, heldMs: 900, readyMs: 120, firstSampleMs: null },
      }),
    );
    expect(screen.getByRole("status").textContent).toContain("no audio arrived");
  });

  it("shows why a recording failed", async () => {
    render(<RecordingStatus />);
    await act(async () => {});
    act(() => handlers.get(RECORDING_FAILED_EVENT)?.({ payload: "no microphone found" }));
    expect(screen.getByRole("status").textContent).toBe("Recording failed: no microphone found");
  });
});

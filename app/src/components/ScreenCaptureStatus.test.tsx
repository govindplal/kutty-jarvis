import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SCREEN_CAPTURE_FAILED_EVENT, SCREEN_CAPTURED_EVENT, ScreenCaptureStatus } from "./ScreenCaptureStatus";

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

describe("ScreenCaptureStatus", () => {
  it("is empty until the first capture", () => {
    render(<ScreenCaptureStatus />);
    expect(screen.getByRole("status").textContent).toBe("");
  });

  it("reports the size of a successful capture without implying it was saved", async () => {
    render(<ScreenCaptureStatus />);
    await act(async () => {});
    act(() => handlers.get(SCREEN_CAPTURED_EVENT)?.({ payload: { width: 1280, height: 720, pngBytes: 204800 } }));
    expect(screen.getByRole("status").textContent).toBe(
      "Screen captured: 1280×720 (200 KB, kept in memory only)",
    );
  });

  it("reports a failed capture", async () => {
    render(<ScreenCaptureStatus />);
    await act(async () => {});
    act(() => handlers.get(SCREEN_CAPTURE_FAILED_EVENT)?.({ payload: "no display found" }));
    expect(screen.getByRole("status").textContent).toBe("Screen capture failed: no display found");
  });
});

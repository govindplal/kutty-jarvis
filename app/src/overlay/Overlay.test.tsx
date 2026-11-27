import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Overlay } from "./Overlay";
import { POINT_EVENT, type PointRequest } from "../lib/pointing";

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

const request: PointRequest = {
  sequence: 1,
  from: { x: 10, y: 20 },
  target: { x: 200, y: 100, width: 80, height: 30 },
  scale: 1,
  flightMs: 500,
  holdMs: 900,
  fadeMs: 300,
};

describe("Overlay", () => {
  it("draws nothing until asked to point", () => {
    render(<Overlay />);
    expect(screen.getByTestId("overlay-cursor").style.opacity).toBe("");
  });

  it("rings the target and starts the cursor at the mouse position", async () => {
    render(<Overlay />);
    await act(async () => {});
    act(() => handlers.get(POINT_EVENT)?.({ payload: request }));

    const ring = screen.getByTestId("overlay-ring");
    expect([ring.style.left, ring.style.top, ring.style.width, ring.style.height]).toEqual([
      "197px",
      "97px",
      "86px",
      "36px",
    ]);
    const cursor = screen.getByTestId("overlay-cursor");
    expect(cursor.style.transform).toBe("translate(10px, 20px)");
    expect(cursor.style.opacity).toBe("1");
  });
});

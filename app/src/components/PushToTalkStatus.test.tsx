import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PUSH_TO_TALK_EVENT, PushToTalkStatus } from "./PushToTalkStatus";

type Handler = (event: { payload: string }) => void;
const handlers = vi.hoisted(() => new Map<string, Handler>());
const unlisten = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: Handler) => {
    handlers.set(name, handler);
    return unlisten;
  }),
}));

afterEach(() => {
  cleanup();
  handlers.clear();
});

describe("PushToTalkStatus", () => {
  it("shows the hotkey hint while idle", () => {
    render(<PushToTalkStatus />);
    expect(screen.getByRole("status").textContent).toBe("Hold Ctrl+Alt+Space to talk");
  });

  it("switches to listening and back when the hotkey events arrive", async () => {
    render(<PushToTalkStatus />);
    await act(async () => {});

    act(() => handlers.get(PUSH_TO_TALK_EVENT)?.({ payload: "listening" }));
    expect(screen.getByRole("status").textContent).toBe("Listening…");

    act(() => handlers.get(PUSH_TO_TALK_EVENT)?.({ payload: "idle" }));
    expect(screen.getByRole("status").textContent).toBe("Hold Ctrl+Alt+Space to talk");
  });

  it("stops listening when unmounted", async () => {
    const { unmount } = render(<PushToTalkStatus />);
    await act(async () => {});
    unmount();
    await act(async () => {});
    expect(unlisten).toHaveBeenCalled();
  });
});

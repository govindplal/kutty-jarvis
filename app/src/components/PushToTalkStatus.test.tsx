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

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => {
  invoke.mockReset();
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

  it("shows the configured shortcut", async () => {
    invoke.mockResolvedValue({ label: "ctrl+alt+k", registered: true, notice: null });
    render(<PushToTalkStatus />);
    expect((await screen.findByText("Hold ctrl+alt+k to talk")).getAttribute("role")).toBe("status");
  });

  it("explains a fallback from an invalid shortcut", async () => {
    const notice = 'KJ_PUSH_TO_TALK="x" is not a valid shortcut; using Ctrl+Alt+Space';
    invoke.mockResolvedValue({ label: "Ctrl+Alt+Space", registered: true, notice });
    render(<PushToTalkStatus />);
    expect((await screen.findByRole("alert")).textContent).toBe(notice);
    expect(screen.getByRole("status").textContent).toBe("Hold Ctrl+Alt+Space to talk");
  });

  it("shows the conflict message when the shortcut is taken", async () => {
    const notice = "Ctrl+Alt+Space is used by another app; push-to-talk is off";
    invoke.mockResolvedValue({ label: "Ctrl+Alt+Space", registered: false, notice });
    render(<PushToTalkStatus />);
    expect((await screen.findByRole("alert")).textContent).toBe(notice);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("stops listening when unmounted", async () => {
    const { unmount } = render(<PushToTalkStatus />);
    await act(async () => {});
    unmount();
    await act(async () => {});
    expect(unlisten).toHaveBeenCalled();
  });
});

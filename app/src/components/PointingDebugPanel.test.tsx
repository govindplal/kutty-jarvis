import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PointingDebugPanel } from "./PointingDebugPanel";
import type { Listing } from "../lib/elements";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(cleanup);

const listing: Listing = {
  list: {
    windowTitle: "Untitled - Notepad",
    processId: 1,
    windowRect: { x: 0, y: 0, width: 800, height: 600 },
    elements: [{ id: 1, role: "button", name: "Save", rect: { x: 10, y: 20, width: 80, height: 24 } }],
    candidates: 1,
    visited: 12,
    prunedOffscreen: 0,
    elapsedMs: 34,
    capped: false,
    partial: false,
  },
  prompt: '1 button "Save" (10,20,80,24)\n',
};

function debugBuild(extra: Record<string, unknown> = {}) {
  invoke.mockImplementation(async (command: string) => {
    if (command === "debug_pointing_available") return true;
    if (command in extra) return extra[command];
    throw new Error(`unexpected ${command}`);
  });
}

describe("PointingDebugPanel", () => {
  it("renders nothing in a release build", async () => {
    invoke.mockRejectedValue("command debug_pointing_available not found");
    const { container } = render(<PointingDebugPanel />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container.innerHTML).toBe("");
  });

  it("lists the foreground window and shows the prompt text", async () => {
    debugBuild({ debug_list_elements: listing });
    render(<PointingDebugPanel />);

    await userEvent.click(await screen.findByRole("button", { name: /List foreground window/ }));

    expect(invoke).toHaveBeenCalledWith("debug_list_elements", { delayMs: 3000 });
    expect(await screen.findByText("1 elements from 12 nodes in 34 ms")).toBeTruthy();
    expect(screen.getByText(/1 button "Save"/)).toBeTruthy();
  });

  it("points at the chosen element after a listing", async () => {
    debugBuild({ debug_list_elements: listing, debug_point_at: {} });
    render(<PointingDebugPanel />);
    const point = await screen.findByRole("button", { name: "Point" });
    expect((point as HTMLButtonElement).disabled).toBe(true);

    await userEvent.click(screen.getByRole("button", { name: /List foreground window/ }));
    await screen.findByText(/elements from/);
    const input = screen.getByRole("spinbutton", { name: "Element number" });
    await userEvent.clear(input);
    await userEvent.type(input, "1");
    await userEvent.click(point);

    expect(invoke).toHaveBeenCalledWith("debug_point_at", { id: 1 });
    expect(await screen.findByText("Pointing at element 1")).toBeTruthy();
  });

  it("shows why a listing failed", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "debug_pointing_available") return true;
      throw "there is no foreground window";
    });
    render(<PointingDebugPanel />);
    await userEvent.click(await screen.findByRole("button", { name: /List foreground window/ }));
    expect(await screen.findByText("Listing failed: there is no foreground window")).toBeTruthy();
  });
});

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(cleanup);

describe("App", () => {
  it("labels the name input for screen readers", () => {
    render(<App />);
    expect(screen.getByRole("textbox", { name: "Name to greet" })).toBeTruthy();
  });

  it("sends the typed name to the greet command and shows the reply", async () => {
    invoke.mockResolvedValue("Hello, Ada!");
    render(<App />);

    await userEvent.type(screen.getByRole("textbox", { name: "Name to greet" }), "Ada");
    await userEvent.click(screen.getByRole("button", { name: "Greet" }));

    expect(invoke).toHaveBeenCalledWith("greet", { name: "Ada" });
    expect(await screen.findByText("Hello, Ada!")).toBeTruthy();
  });
});

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ModelSettings } from "./ModelSettings";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const settings = {
  speechToText: "local",
  assistant: "local",
  models: { anthropic: "a", openaiChat: "b", openaiTranscribe: "c" },
};
const none = { anthropic: { configured: false }, openai: { configured: false } };

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (command: string) => {
    if (command === "get_model_settings") return settings;
    if (command === "api_key_status") return none;
    return undefined;
  });
});
afterEach(cleanup);

describe("ModelSettings", () => {
  it("starts local with no keys and no privacy warning", async () => {
    render(<ModelSettings />);
    expect(await screen.findByText("Models")).toBeTruthy();
    expect(screen.getAllByText("no key saved")).toHaveLength(2);
    expect(screen.queryByText(/are sent to/)).toBeNull();
  });

  it("saves a key through a password field and shows only the last four", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "get_model_settings") return settings;
      if (command === "api_key_status") return none;
      if (command === "set_api_key") {
        return { configured: true, last4: "abcd" };
      }
    });
    render(<ModelSettings />);
    const input = await screen.findByLabelText("Anthropic API key");
    expect(input.getAttribute("type")).toBe("password");
    await userEvent.type(input, "sk-ant-secret-abcd");
    await userEvent.click(screen.getByRole("button", { name: "Save Anthropic key" }));

    expect(invoke).toHaveBeenCalledWith("set_api_key", {
      provider: "anthropic",
      key: "sk-ant-secret-abcd",
    });
    expect(await screen.findByText("saved (…abcd)")).toBeTruthy();
    expect((input as HTMLInputElement).value).toBe("");
    expect(document.body.textContent).not.toContain("secret");
  });

  it("clears a saved key", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "get_model_settings") return settings;
      if (command === "api_key_status") {
        return { ...none, openai: { configured: true, last4: "wxyz" } };
      }
      if (command === "clear_api_key") return { configured: false };
    });
    render(<ModelSettings />);
    await userEvent.click(await screen.findByRole("button", { name: "Clear OpenAI key" }));
    expect(invoke).toHaveBeenCalledWith("clear_api_key", { provider: "openai" });
    expect(await screen.findAllByText("no key saved")).toHaveLength(2);
  });

  it("states what is sent when a cloud provider is chosen", async () => {
    invoke.mockImplementation(async (command: string, args?: { patch?: object }) => {
      if (command === "get_model_settings") return settings;
      if (command === "api_key_status") return none;
      if (command === "patch_model_settings") return { ...settings, ...args?.patch };
    });
    render(<ModelSettings />);
    await userEvent.selectOptions(await screen.findByLabelText("Answering"), "anthropic");
    expect(
      await screen.findByText(
        "Your question, transcript, screenshot, the foreground window title and the names of on-screen UI elements are sent to Anthropic when selected.",
      ),
    ).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("patch_model_settings", {
      patch: { assistant: "anthropic" },
    });
  });

  it("shows errors from the backend", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "get_model_settings") return settings;
      if (command === "api_key_status") return none;
      if (command === "set_api_key") throw "an API key has no spaces or line breaks in it";
    });
    render(<ModelSettings />);
    await userEvent.type(await screen.findByLabelText("OpenAI API key"), "x");
    await userEvent.click(screen.getByRole("button", { name: "Save OpenAI key" }));
    expect((await screen.findByRole("alert")).textContent).toContain("no spaces");
  });

  it("keeps Local/Local when both stages are switched back quickly", async () => {
    const cloud = { ...settings, speechToText: "openai", assistant: "anthropic" };
    let stored = { ...cloud };
    const replies: Array<() => void> = [];
    invoke.mockImplementation(async (command: string, args?: { patch?: object }) => {
      if (command === "get_model_settings") return { ...stored };
      if (command === "api_key_status") return none;
      if (command === "patch_model_settings") {
        const patch = args?.patch ?? {};
        // Applied in call order on the "disk"; acknowledged later, in reverse.
        stored = { ...stored, ...patch };
        const snapshot = { ...stored };
        return new Promise((resolve) => replies.push(() => resolve(snapshot)));
      }
    });
    render(<ModelSettings />);
    const stt = await screen.findByLabelText("Speech to text");
    const answering = screen.getByLabelText("Answering");
    await userEvent.selectOptions(stt, "local");
    await userEvent.selectOptions(answering, "local");
    await vi.waitFor(() => expect(replies.length).toBeGreaterThan(0));
    while (replies.length) replies.pop()?.();
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(4));
    replies.pop()?.();

    expect(stored.speechToText).toBe("local");
    expect(stored.assistant).toBe("local");
    await vi.waitFor(() => {
      expect((stt as HTMLSelectElement).value).toBe("local");
      expect((answering as HTMLSelectElement).value).toBe("local");
    });
    expect(screen.queryByText(/are sent to/)).toBeNull();
  });

  it("clears the key draft even when other statuses cannot be read", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "get_model_settings") return settings;
      if (command === "api_key_status") throw "credential store unavailable";
      if (command === "set_api_key") return { configured: true, last4: "wxyz" };
    });
    render(<ModelSettings />);
    const input = await screen.findByLabelText("OpenAI API key");
    await userEvent.type(input, "sk-secret-wxyz");
    await userEvent.click(screen.getByRole("button", { name: "Save OpenAI key" }));
    expect(await screen.findByText("saved (…wxyz)")).toBeTruthy();
    expect((input as HTMLInputElement).value).toBe("");
  });
});

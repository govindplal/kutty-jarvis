import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  PROVIDER_LABEL,
  describeKey,
  privacyNotes,
  type AnswerProvider,
  type KeyProvider,
  type KeyStatus,
  type KeyStatuses,
  type SettingsPatch,
  type ModelSettings as Settings,
  type SttProvider,
} from "../lib/models";

const message = (error: unknown) => (typeof error === "string" ? error : "Something went wrong");

/** Choose local or cloud models and keep the user's own API keys. */
export function ModelSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [keys, setKeys] = useState<KeyStatuses | null>(null);
  const [drafts, setDrafts] = useState<Record<KeyProvider, string>>({ anthropic: "", openai: "" });
  const [error, setError] = useState("");

  useEffect(() => {
    invoke<Settings>("get_model_settings")
      .then((value) => value && setSettings(value))
      .catch((e) => setError(message(e)));
    invoke<KeyStatuses>("api_key_status")
      .then((value) => value && setKeys(value))
      .catch((e) => setError(message(e)));
  }, []);

  // One change at a time, in the order made, each patching only its own
  // field, so quick successive changes can never restore an older value.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  function update(patch: SettingsPatch) {
    setError("");
    setSettings((current) => (current ? { ...current, ...patch } : current));
    queue.current = queue.current.then(async () => {
      try {
        setSettings(await invoke<Settings>("patch_model_settings", { patch }));
      } catch (e) {
        setError(message(e));
        invoke<Settings>("get_model_settings")
          .then((value) => value && setSettings(value))
          .catch(() => {});
      }
    });
  }

  const noKey: KeyStatus = { configured: false };
  function setStatus(provider: KeyProvider, status: KeyStatus) {
    setKeys((current) => ({ anthropic: noKey, openai: noKey, ...current, [provider]: status }));
  }

  async function saveKey(provider: KeyProvider) {
    setError("");
    try {
      const status = await invoke<KeyStatus>("set_api_key", { provider, key: drafts[provider] });
      setDrafts((current) => ({ ...current, [provider]: "" }));
      if (status) setStatus(provider, status);
    } catch (e) {
      setError(message(e));
    }
  }

  async function clearKey(provider: KeyProvider) {
    setError("");
    try {
      const status = await invoke<KeyStatus>("clear_api_key", { provider });
      if (status) setStatus(provider, status);
    } catch (e) {
      setError(message(e));
    }
  }

  if (!settings) {
    return <section aria-label="Models">{error && <p role="alert">{error}</p>}</section>;
  }

  return (
    <section aria-label="Models" className="capture-status">
      <h2>Models</h2>
      <p>Local models are the default. Add your own API key to use a cloud model instead.</p>
      <div className="row">
        <label>
          Speech to text{" "}
          <select
            value={settings.speechToText}
            onChange={(e) => update({ speechToText: e.target.value as SttProvider })}
          >
            <option value="local">Local</option>
            <option value="openai">OpenAI</option>
          </select>
        </label>
        <label>
          Answering{" "}
          <select
            value={settings.assistant}
            onChange={(e) => update({ assistant: e.target.value as AnswerProvider })}
          >
            <option value="local">Local</option>
            <option value="anthropic">Anthropic</option>
            <option value="openai">OpenAI</option>
          </select>
        </label>
      </div>
      {privacyNotes(settings).map((note) => (
        <p key={note}>{note}</p>
      ))}
      {(["anthropic", "openai"] as const).map((provider) => (
        <div className="row" key={provider}>
          <input
            type="password"
            autoComplete="off"
            aria-label={`${PROVIDER_LABEL[provider]} API key`}
            placeholder={`${PROVIDER_LABEL[provider]} API key`}
            value={drafts[provider]}
            onChange={(e) => setDrafts({ ...drafts, [provider]: e.target.value })}
          />
          <button
            type="button"
            disabled={!drafts[provider]}
            aria-label={`Save ${PROVIDER_LABEL[provider]} key`}
            onClick={() => saveKey(provider)}
          >
            Save key
          </button>
          <button
            type="button"
            disabled={!keys?.[provider]?.configured}
            aria-label={`Clear ${PROVIDER_LABEL[provider]} key`}
            onClick={() => clearKey(provider)}
          >
            Clear
          </button>
          <span aria-label={`${PROVIDER_LABEL[provider]} key status`}>
            {describeKey(keys?.[provider])}
          </span>
        </div>
      ))}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}

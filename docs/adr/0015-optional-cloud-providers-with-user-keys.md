## ADR 0015: Optional cloud providers with the user's own keys

- **Status:** Accepted (2026-11-27)
- **Decision:** Local models stay the default. The user can switch speech-to-text (OpenAI) and answering (Anthropic or OpenAI) to cloud models with their own API keys, chosen per stage in a Models settings section. No proxy server; the app calls the provider directly.
- **Rationale:** Local CPU answers take 15-20 s (findings 0001, 0003). A cloud model is an optional speed and quality upgrade, and the product stays free for anyone who does not want to pay (ADR 0004).
- **Keys:** One credential per provider in the OS store (the `keyring` crate; Windows Credential Manager), service `kutty-jarvis`. A key is never returned to the frontend, logged or written to a file. The UI gets only "configured" and the last four characters. Saving validates loosely (non-empty, ASCII, no whitespace, sane length), so a provider can change its key format.
- **Settings:** Non-secret `model-settings.json` in the app config folder, written atomically. Defaults are local for both stages. Model ids are overridable there.
- **Selection:** Providers are picked per question from the settings and key presence. If a cloud provider is selected and its key is missing, the question fails with a clear message; the app never falls back to another provider silently.
- **Clients:** Anthropic Messages API, OpenAI chat completions (vision) and OpenAI audio transcriptions, behind the existing `SpeechToText` and `Assistant` traits. They reuse the local prompt builders and the same screenshot shrink. HTTPS only, no redirects, connect and request timeouts. 401, 429 and 5xx get actionable messages that never include the key.
- **Privacy:** With a cloud provider selected, the question audio (OpenAI transcription), the transcript and the screenshot (with the UI element list) leave the machine for that provider. The UI says so next to the selects.
- **Consequences:** Cloud use costs the user money at the provider's rates. Model ids change over time; defaults live in `settings.rs`. Keys cannot be stored on non-Windows platforms yet (a stub store reports an error).

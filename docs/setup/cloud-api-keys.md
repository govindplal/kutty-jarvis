# Using your own cloud API keys (optional)

The buddy runs on local models by default and sends nothing off your machine (ADR [0004](../adr/0004-local-first-provider-agnostic-models.md)). If you would rather use a faster cloud model, add your own key. See ADR [0015](../adr/0015-optional-cloud-providers-with-user-keys.md).

## Steps

1. Create an API key with the provider: Anthropic at <https://console.anthropic.com/>, OpenAI at <https://platform.openai.com/api-keys>. Billing is between you and the provider.
2. Open the app window and find the **Models** section.
3. Paste the key into the matching field and press **Save key**. The status then reads "saved (...abcd)", the last four characters. The key goes to Windows Credential Manager (entry `kutty-jarvis`); it is never shown again, logged or stored in a file.
4. Pick the providers: **Speech to text** is Local or OpenAI; **Answering** is Local, Anthropic or OpenAI. The choice applies from the next question.

**Clear** removes the key from the credential store. If you select a cloud provider whose key is missing, the question fails with a message; it is never sent elsewhere instead.

## What leaves your machine

With **Answering** set to a cloud provider, that provider receives your transcript, the screenshot, the title of the foreground window and the names of the on-screen UI elements, plus what you asked. With **Speech to text** set to OpenAI, OpenAI receives the recorded audio. The Models section says the same next to the selects. Screenshots are still never stored by the app.

Provider error text is never shown in the app, since it could quote your key; you see a short message, at most with the provider's error code.

## Models and overrides

Defaults: Anthropic `claude-haiku-5-5`, OpenAI chat `gpt-6-luna`, OpenAI transcription `gpt-transcribe`. To use others, edit `model-settings.json` in the app config folder (on Windows `%APPDATA%\dev.kuttyjarvis.app\`); its model fields hold the ids.

Answers ask for low reasoning effort (OpenAI `reasoning_effort: "low"`, Anthropic `output_config.effort: "low"`) so spoken replies stay quick, with a 1024-token limit that leaves room for reasoning. If a model still runs out of room before writing text, the app says so instead of staying silent.

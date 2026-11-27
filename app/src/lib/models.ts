export type SttProvider = "local" | "openai";
export type AnswerProvider = "local" | "anthropic" | "openai";
export type KeyProvider = "anthropic" | "openai";

export type SettingsPatch = { speechToText?: SttProvider; assistant?: AnswerProvider };

export type ModelSettings = {
  speechToText: SttProvider;
  assistant: AnswerProvider;
  models: { anthropic: string; openaiChat: string; openaiTranscribe: string };
};

export type KeyStatus = { configured: boolean; last4?: string };
export type KeyStatuses = Record<KeyProvider, KeyStatus>;

export const PROVIDER_LABEL: Record<KeyProvider, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
};

/** What leaves the machine for the current choices; empty when all local. */
export function privacyNotes(settings: ModelSettings): string[] {
  const notes: string[] = [];
  if (settings.assistant !== "local") {
    notes.push(
      `Your question, transcript, screenshot, the foreground window title and the names of on-screen UI elements are sent to ${PROVIDER_LABEL[settings.assistant]} when selected.`,
    );
  }
  if (settings.speechToText === "openai") {
    notes.push("Your recorded audio is sent to OpenAI for transcription when selected.");
  }
  return notes;
}

export function describeKey(status: KeyStatus | undefined): string {
  if (!status?.configured) return "no key saved";
  return status.last4 ? `saved (…${status.last4})` : "saved";
}

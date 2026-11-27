export const SAMPLE_TEXT = "Hello! I can see your screen. The save button is in the top left.";

export type SpeechEvent = {
  id: number;
  engine: string;
  fallback?: string;
  interrupted?: boolean;
  error?: string;
};
export type DownloadEvent = { status: "started" | "ready" | "failed"; error?: string };

export function describeSpeech(name: string, event: SpeechEvent): string {
  switch (name) {
    case "speech-started":
      return event.fallback
        ? `Speaking with ${event.engine} (${event.fallback})`
        : `Speaking with ${event.engine}`;
    case "speech-finished":
      return event.interrupted ? `Interrupted (${event.engine})` : `Finished (${event.engine})`;
    default:
      return `Speech failed (${event.engine}): ${event.error ?? "unknown error"}`;
  }
}

export function describeDownload(event: DownloadEvent): string {
  if (event.status === "started") return "Downloading the Piper voice (about 90 MB)…";
  if (event.status === "ready") return "Piper voice ready";
  return `Piper download failed: ${event.error ?? "unknown error"}`;
}

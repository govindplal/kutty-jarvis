import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export const RECORDING_FINISHED_EVENT = "recording-finished";
export const RECORDING_FAILED_EVENT = "recording-failed";

export type RecordingFinishedPayload = { durationMs: number; wavBytes: number };

/** Shows the last microphone recording (taken while the hotkey is held). */
export function RecordingStatus() {
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    const finished = listen<RecordingFinishedPayload>(RECORDING_FINISHED_EVENT, (event) => {
      const { durationMs, wavBytes } = event.payload;
      setStatus(
        `Recorded ${(durationMs / 1000).toFixed(1)} s (${Math.round(wavBytes / 1024)} KB, kept in memory only)`,
      );
    });
    const failed = listen<string>(RECORDING_FAILED_EVENT, (event) => {
      setStatus(`Recording failed: ${event.payload}`);
    });
    return () => {
      finished.then((stop) => stop());
      failed.then((stop) => stop());
    };
  }, []);

  return (
    <p className="capture-status" role="status" aria-live="polite">
      {status}
    </p>
  );
}

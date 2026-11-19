/** Payload of the `recording-finished` event (see app/src-tauri/src/hotkey.rs). */
export type RecordingFinishedPayload = {
  durationMs: number;
  wavBytes: number;
  heldMs: number;
  readyMs: number;
  firstSampleMs: number | null;
};

const seconds = (ms: number) => `${(ms / 1000).toFixed(1)} s`;

export function describeRecording(p: RecordingFinishedPayload): string {
  const startup = p.firstSampleMs === null ? "no audio arrived" : `first audio after ${seconds(p.firstSampleMs)}`;
  return `Recorded ${seconds(p.durationMs)} of ${seconds(p.heldMs)} held; ${startup} (${Math.round(p.wavBytes / 1024)} KB, kept in memory only)`;
}

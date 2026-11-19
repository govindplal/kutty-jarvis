import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

export const SCREEN_CAPTURED_EVENT = "screen-captured";
export const SCREEN_CAPTURE_FAILED_EVENT = "screen-capture-failed";

export type ScreenCapturedPayload = { width: number; height: number; pngBytes: number };

/** Shows the size of the latest screen capture (taken on each hotkey press). */
export function ScreenCaptureStatus() {
  const [status, setStatus] = useState<string>("");

  useEffect(() => {
    const captured = listen<ScreenCapturedPayload>(SCREEN_CAPTURED_EVENT, (event) => {
      const { width, height, pngBytes } = event.payload;
      setStatus(`Screen captured: ${width}×${height} (${Math.round(pngBytes / 1024)} KB, kept in memory only)`);
    });
    const failed = listen<string>(SCREEN_CAPTURE_FAILED_EVENT, (event) => {
      setStatus(`Screen capture failed: ${event.payload}`);
    });
    return () => {
      captured.then((stop) => stop());
      failed.then((stop) => stop());
    };
  }, []);

  return (
    <p className="capture-status" role="status" aria-live="polite">
      {status}
    </p>
  );
}

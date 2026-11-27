import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { describeListing, type Listing } from "../lib/elements";

/** Delay before listing, so there is time to switch to the window to list. */
export const LIST_DELAY_MS = 3000;

async function debugCommandsAvailable(): Promise<boolean> {
  try {
    // Only debug builds register this command; release builds reject it.
    return (await invoke<boolean>("debug_pointing_available")) === true;
  } catch {
    return false;
  }
}

/**
 * Debug builds only: list the foreground window's UI elements and point the
 * overlay at one of them. Renders nothing in release builds.
 */
export function PointingDebugPanel() {
  const [available, setAvailable] = useState(false);
  const [listing, setListing] = useState<Listing | null>(null);
  const [target, setTarget] = useState("1");
  const [status, setStatus] = useState("");

  useEffect(() => {
    let live = true;
    debugCommandsAvailable().then((ok) => {
      if (live) setAvailable(ok);
    });
    return () => {
      live = false;
    };
  }, []);

  if (!available) return null;

  async function list() {
    setStatus(`Switch to the window to list; reading it in ${LIST_DELAY_MS / 1000} s…`);
    try {
      const result = await invoke<Listing>("debug_list_elements", { delayMs: LIST_DELAY_MS });
      setListing(result);
      setStatus(describeListing(result.list));
    } catch (error) {
      setStatus(`Listing failed: ${String(error)}`);
    }
  }

  async function point() {
    try {
      await invoke("debug_point_at", { id: Number(target) });
      setStatus(`Pointing at element ${target}`);
    } catch (error) {
      setStatus(`Pointing failed: ${String(error)}`);
    }
  }

  return (
    <section className="pointing-debug" aria-label="Pointing debug">
      <h2>Pointing (debug build)</h2>
      <button type="button" onClick={list}>
        List foreground window in {LIST_DELAY_MS / 1000} s
      </button>
      <form
        className="row"
        onSubmit={(event) => {
          event.preventDefault();
          point();
        }}
      >
        <input
          aria-label="Element number"
          type="number"
          min={1}
          value={target}
          onChange={(event) => setTarget(event.currentTarget.value)}
        />
        <button type="submit" disabled={!listing}>
          Point
        </button>
      </form>
      <p role="status" aria-live="polite">
        {status}
      </p>
      {listing && <pre className="pointing-prompt">{listing.prompt}</pre>}
    </section>
  );
}

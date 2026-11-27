/** Types of the debug element listing (see app/src-tauri/src/uia/mod.rs). */

export type ElementRect = { x: number; y: number; width: number; height: number };

export type UiElement = {
  id: number;
  role: string;
  name: string;
  rect: ElementRect;
  automationId?: string;
};

export type ElementList = {
  windowTitle: string;
  processId: number;
  windowRect: ElementRect;
  elements: UiElement[];
  candidates: number;
  visited: number;
  prunedOffscreen: number;
  elapsedMs: number;
  capped: boolean;
  partial: boolean;
  stopReason?: "timeBudget" | "nodeLimit";
};

export type Listing = { list: ElementList; prompt: string };

export function describeListing(list: ElementList): string {
  const flags = [
    list.capped ? `capped, ${list.candidates} candidates` : "",
    list.partial ? `partial: ${list.stopReason === "nodeLimit" ? "node limit" : "time budget"}` : "",
  ].filter(Boolean);
  const suffix = flags.length ? ` (${flags.join("; ")})` : "";
  return `${list.elements.length} elements from ${list.visited} nodes in ${list.elapsedMs} ms${suffix}`;
}

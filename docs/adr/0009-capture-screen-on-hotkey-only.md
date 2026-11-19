# 0009. Capture the screen only on hotkey press, in memory only

- Status: accepted
- Date: 2026-11-20
- Deciders: Claude Code, owner informed
- Evidence: end-to-end run (hotkey press produced a 1280x720 capture; no PNG files written to disk); finding 0003 for image sizes

## Context

The buddy needs to see what the user sees. Privacy is one of the product's promises, and HeyClicky's own FAQ says it sees the screen only when the hotkey is pressed and never stores screenshots. We want the same guarantee, checkable in code.

## Options considered

1. **Continuous capture** (a rolling buffer). Lets the buddy react without being asked, but records the user's screen all the time. Rejected.
2. **Capture on hotkey press, keep in memory.** The image exists only until the next capture, is never written to disk, and is never sent anywhere until the user asks.
3. **Capture on hotkey press, save to disk for debugging.** Useful, but creates a copy of private screens. Rejected for the shipped app; debugging can use the in-memory frame.

## Decision

- Capture happens only on a real push-to-talk press (see [ADR 0008](0008-tray-app-and-push-to-talk-hotkey.md)), on a background thread so the hotkey stays responsive.
- The primary display is captured and scaled so the longest edge is at most 1280 px. The smaller size keeps local vision models fast (finding 0003).
- The latest frame is kept in memory (`LatestScreenshot`) and replaced on each press. Nothing is written to disk.
- The UI shows only the size and byte count, so the user can see that a capture happened.

## Consequences

- Good: privacy is a property the code enforces and a test can check; the capture cost is paid only when asked.
- Cost: only the primary display is captured for now; multi-monitor is future work. Some protected content (for example DRM video) may come out black.
- Not yet covered: a visible capture indicator beyond the status text, and a first-run privacy notice.
- Revisit if the buddy needs to watch the screen continuously (that would need a new ADR and a visible indicator).

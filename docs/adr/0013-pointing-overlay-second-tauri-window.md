# 0013. Draw the pointing cursor in a second, click-through Tauri window

- Status: accepted
- Date: 2026-11-27
- Deciders: Claude Code (pointing agent); owner to review in the PR
- Evidence: [finding 0007](../findings/0007-uia-enumeration-across-apps.md), [ADR 0005](0005-ui-automation-first-pointing.md)

## Context

ADR 0005 made UI Automation the source of pointing targets: we get an exact rectangle in physical screen pixels. The buddy then has to *show* the target: fly a cursor from the mouse to the element, ring it briefly and fade. That drawing layer must:

- cover the screen, sit above every app and let all clicks through;
- never take focus, and stay out of the taskbar and Alt+Tab (the user is working in another app);
- line up with UIA's physical pixels at any display scale (the dev machine runs 1920x1080 at 125%);
- animate smoothly, and be easy to restyle later (themes, a speech bubble next to the cursor).

## Options considered

1. **Second Tauri window with its own small React entry** (`overlay.html`). Transparent, undecorated, always on top, `focusable: false`, `skipTaskbar`, `set_ignore_cursor_events(true)`, plus `WS_EX_TOOLWINDOW` set natively for Alt+Tab. Pros: same stack as the rest of the UI, CSS/SVG drawing, devtools, testable in Vitest; DPI handled by the webview (CSS px = physical / scale). Cons: a second WebView2 renderer (extra memory, not measured yet); Tauri does not expose `WS_EX_TOOLWINDOW` or show-without-activate, so a few Win32 calls are still needed; WebView2 transparency is less battle-tested than native layered windows.
2. **Native layered Win32 window** drawn with GDI+/Direct2D and `UpdateLayeredWindow`. Pros: smallest footprint, full control of styles and timing. Cons: hand-written rendering and animation in Rust, Windows-only drawing code (more to port to macOS/Linux), harder to style and test; we would rebuild what CSS gives us.
3. **Draw inside the main window.** Rejected: the main window is not full-screen, takes focus and is often hidden in the tray.

## Decision

We use option 1: a second Tauri window labelled `overlay`, hidden while idle. Rust converts the mouse position and the target rectangle to the overlay's CSS pixels (`overlay::MonitorGeometry`), positions the window over the primary monitor with `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE | SWP_SHOWWINDOW)`, and sends one `overlay-point` event. The page animates the gesture (`src/lib/pointing.ts`: eased quadratic arc bowing upwards, then hold, then fade), and Rust hides the window when the animation has had time to finish. The page also checks that the scale Rust used equals its own `devicePixelRatio` and flags a mismatch.

Reasoning: pointing accuracy comes from UIA, not from the drawing layer, so the drawing layer should be the cheapest to build, restyle and test. Measured in finding 0007, the webview overlay put the cursor tip at the exact centre of all 15 targets across five apps at 125% scale, and it never became the foreground window.

## Consequences

- Good: the overlay is plain React/CSS; the maths is pure TypeScript and Rust with unit tests; DPI conversion is one division by the monitor's scale factor, confirmed live (overlay 1536x864 CSS = 1920x1080 physical at 1.25).
- Good: per-OS code is small (`overlay/windows.rs`: style bits, show without activation, hide). A macOS/Linux port mainly needs the equivalents of those three calls.
- Cost: a second WebView2 instance stays loaded (hidden) for the life of the app.
- Cost / surprise: while the overlay is visible, our **own** main window's accessibility tree shrinks (19 elements to 3); other apps are unaffected (finding 0007). The buddy must list elements before showing the overlay, which is the natural order anyway.
- Primary monitor only. A target whose centre is on another monitor is refused with an error. Follow-up: one overlay per monitor (or one spanning the virtual desktop), choosing the scale factor per monitor.
- Not yet verified on screen: Alt+Tab/taskbar absence and the visual result were checked by design and by reading the overlay's own render, not by a desktop screenshot (see finding 0007, "Not verified").
- Revisit if: the second webview's memory matters on low-end machines, WebView2 transparency misbehaves on some GPUs/drivers, or we need the cursor to follow the target while windows move (then a native layer with tighter frame timing may be worth it).

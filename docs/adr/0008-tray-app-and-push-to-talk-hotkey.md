# 0008. Tray app with a Ctrl+Alt+Space push-to-talk hotkey

- Status: accepted
- Date: 2026-11-20
- Deciders: Claude Code, owner informed
- Evidence: end-to-end check on 2026-11-20 (simulated key hold changes the window status from idle to "Listening…" and back); unit tests in `app/src-tauri/src/push_to_talk.rs`

## Context

The buddy should be one keypress away from any app, without a window in the way. Users hold a key while speaking and release it to send, like the original Clicky (ctrl+option on macOS). Windows needs a combination that works system-wide and rarely conflicts.

## Options considered

1. **Ctrl+Alt+Space.** Rarely bound on Windows, easy to hold with one hand, registers as a global shortcut.
2. **Modifier-only (Ctrl+Win, like some clones).** Feels closest to Clicky, but modifier-only combinations cannot be registered as global shortcuts; they need a low-level keyboard hook, which antivirus tools treat with suspicion.
3. **Wake word.** Hands-free, but needs an always-on microphone, which conflicts with our privacy goal of capturing only on request.

## Decision

- Default push-to-talk is **Ctrl+Alt+Space**, registered through `tauri-plugin-global-shortcut`. A small state machine turns the key auto-repeat stream into one `listening` and one `idle` event, sent to the UI as `push-to-talk`.
- The app lives in the **system tray** (menu: Open, Quit). Closing the window hides it; Quit is in the tray menu.

## Consequences

- Good: no keyboard hook, nothing listening when the key is not held, simple tested state logic.
- Cost: Ctrl+Alt+Space may clash with some IME or app shortcuts; the shortcut must become configurable.
- Not yet verified: the tray icon and menu were not checked visually or by automation.
- Baseline: main process working set about 27 MB at idle (debug build, WebView2 child processes not included).

## Update 2026-11-27: a taken shortcut is not fatal, and it can be overridden

- **Problem.** If another program already owned Ctrl+Alt+Space, registering it failed and the app panicked at start-up with "HotKey already registered". The most likely holder on the owner's machine is the Claude desktop app (its quick-entry shortcut); this is a suspicion, not confirmed. Many target users will run Claude desktop too.
- **Change.** A registration failure is now logged, the app stays in the tray, the window shows "Ctrl+Alt+Space is used by another app; push-to-talk is off" and the tray tooltip says the same.
- **Override.** The environment variable `KJ_PUSH_TO_TALK` (for example `ctrl+alt+k`) replaces the default, parsed with the plugin's own shortcut parser. An invalid value is reported in the window and the default is used. The default key is unchanged; there is still no settings UI.

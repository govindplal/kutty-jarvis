# 0003. Tauri v2 over Electron and Python

- Status: accepted
- Date: 2026-11-20
- Deciders: project owner (delegated the choice: "best for majority users, CPU-only modest laptop")
- Evidence: reasoning from framework characteristics; to be validated by measuring idle memory in the scaffold (see Consequences)

## Context

The app is a tray utility with a global hotkey, a transparent click-through overlay, screen capture and audio. It must run beside a local language and vision model on a modest, CPU-only laptop where RAM and CPU are the scarcest resources. Most users are on Windows.

## Options considered

1. **Electron.** Mature, huge ecosystem, easiest overlay and capture. Bundles Chromium: large install and a few hundred MB of idle RAM, which competes with the model.
2. **Python (PySide6 or similar).** Fast to prototype and closest to the ML tooling. Hard to package for ordinary users (bundled runtime, larger installers, frequent antivirus false positives) and awkward for a transparent click-through overlay.
3. **Tauri v2 (Rust core, React/TypeScript UI).** Uses the system WebView2 on Windows, small installer, low idle memory, native access for hotkey, capture and UI Automation through Rust crates.

## Decision

Tauri v2 with a Rust core and a React/TypeScript front end. Platform-specific code (capture, hotkey, accessibility) sits behind traits so macOS and Linux can be added.

## Consequences

- Good: small download, low overhead, a native-feeling tray app, a safe language for system code.
- Cost: Rust learning curve for contributors; the overlay and UI Automation need Win32 work; WebView2 is required (preinstalled on Windows 11 and current Windows 10).
- Validate: record idle RAM of the scaffolded app in a finding. If it is not clearly lower than an Electron baseline, revisit.

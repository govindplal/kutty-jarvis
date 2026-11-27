# 0007. UI Automation enumeration across apps

- Date: 2026-11-27
- Status: done
- Confidence: measured (one machine, one display scale, five apps, 3 listing runs and 3 pointing targets per app)
- Decisions informed: [ADR 0005](../adr/0005-ui-automation-first-pointing.md), [ADR 0013](../adr/0013-pointing-overlay-second-tauri-window.md)

## Question

Does our UI Automation walker return a usable, numbered target list (under the 150 cap, within the 1.5 s budget) for typical Windows apps, and does the overlay cursor land inside the chosen element at the machine's real display scale?

## Hypothesis

Written before running: native apps (Notepad, Explorer) list in well under 200 ms with clean names; Chromium/Electron apps (Edge, VS Code) are slower and noisier and may hit the cap; pointing lands inside every element because UIA rectangles and the overlay share physical pixels, as long as the scale factor is applied once.

## Setup

- Machine: Intel64 Family 6 Model 165 (12 threads), Windows 11 Home 10.0.22631.6199.
- Display: one monitor, 1920x1080 physical, scale factor 1.25 (overlay page reports `devicePixelRatio` 1.25, 1536x864 CSS px).
- App: kutty-jarvis debug build from branch `feat/pointing-overlay-uia` (`npx tauri build --debug --no-bundle`), cap 150, budget 1500 ms, node limit 5000. For the run only, hotkey registration was made non-fatal locally because another program already holds Ctrl+Alt+Space; that patch is not committed.
- Targets (all started by the test and closed afterwards):
  - Notepad (Windows 11 WinUI Notepad), empty document.
  - kutty-jarvis main window (Tauri 2 / WebView2).
  - File Explorer in a separate process (`explorer.exe /separate`), details view of a Documents folder.
  - Microsoft Edge 158 with a temporary profile on MDN's Accessibility page, plus the HTML spec index page as a dense-page test.
  - VS Code (Electron, Chromium 150) with a temporary profile, this repository open.
- Driver: the debug commands `debug_list_elements` and `debug_point_at`, called from scripts over the WebView2 debug port (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`). The overlay page's drawn state (cursor transform, ring opacity, `devicePixelRatio`) was read over the same port mid-hold.

## Method

1. For each app, list its main window 3 times (`processId` target) and record time, element count, nodes visited, candidates before the cap, and the partial and capped flags.
2. Point at 3 named elements (`holdMs` 2500). About 400 ms after the flight ends, read the overlay cursor's CSS translate, multiply by `devicePixelRatio`, and check that the resulting physical point is inside the element's UIA rectangle. Record the distance to the rectangle's centre.
3. While each gesture is on screen, list the foreground window (to check that the overlay never becomes foreground) and the target window again (to see whether the overlay changes the target's tree).
4. Cross-check DPI on our own window: compare the UIA rectangle of a DOM button with the rectangle computed from the DOM (`(screenX + rect.x) * devicePixelRatio`).
5. During development, the filter rules were tightened twice after seeing real output (see Results). Final numbers are from the last build; the dense Edge page ran on the build before the last two rules.

## Results

Final build, physical pixels, times in ms (3 runs):

| App | Listing time | Elements | Nodes visited | Capped | Partial | Offscreen subtrees skipped | Pointing (inside / error to centre) |
|---|---|---|---|---|---|---|---|
| Notepad | 39 / 40 / 39 | 25 | 40 | no | no | 4 | 3/3, 0 px |
| kutty-jarvis main | 41 / 38 / 43 | 19 | 38 | no | no | 2 | 3/3, 0 px |
| File Explorer (details view) | 172 / 177 / 195 | 83 | 198 | no | no | 2 | 3/3, 0 px |
| Edge, MDN page | 130 / 128 / 148 | 70 | 139 | no | no | 13 | 3/3, 0 px |
| VS Code | 139 / 133 / 135 | 127 | 239 | no | no | 5 | 3/3, 0 px |
| Edge, HTML spec index (earlier build) | 158 / 136 / 151 | 54 | 142 | no | no | 48 | not pointed |

Pointed elements: Notepad File menu, Bold and Settings buttons; kutty-jarvis "List foreground window" and "Point" buttons and the Minimize caption button; Explorer Back button, a file row, the search box; Edge Back button, address bar, an in-page link; VS Code Go Back button, a file tree item, Toggle Panel button.

Before the filter fixes (earlier builds, same windows):

| App | Candidates | Capped | Main noise |
|---|---|---|---|
| File Explorer | 167 | yes (150) | each row's 4 cells listed as `edit "Name"`, `edit "Date modified"`, ... |
| VS Code | 166 | yes (150) | icon-font glyph names, path groups inside tabs and tree items, duplicate group per activity tab |

Other observations:

- **Focus:** in all 15 gestures the foreground window during the hold was not the overlay (`kutty-jarvis pointer`), and it stayed the same window as before the gesture.
- **DPI:** overlay 1536x864 CSS x 1.25 = 1920x1080, the full monitor. A DOM button at UIA (630,517,96,50) computed from the DOM as (630,517,96,49): equal within rounding. The overlay never reported a scale mismatch.
- **Overlay hides our own tree:** while the overlay was visible, our own main window's list dropped from 19 elements (38 nodes) to 3 (11 nodes), while `document.visibilityState` stayed `visible`. It recovered once the overlay hid. Notepad, Explorer, Edge and VS Code lists were unchanged while the overlay showed.
- **Chromium builds its tree lazily:** the first listing of a fresh VS Code window returned 4 elements; the next, 2 s later, 166. The first Edge listing likewise had window chrome but no web document.
- **Foreground mode** (the default, no process id) was exercised with the first build on the desktop, the owner's terminal and Notepad: 10, 7 and 26 elements in 652, 36 and 50 ms. The first call took 652 ms, so there is a one-off UIA start-up cost. Contents of the owner's windows are not recorded.
- Neither the cap nor the time budget was hit by the final build in these apps; offscreen pruning keeps long web pages small (48 subtrees skipped on the spec index).

Raw data (sanitised; Explorer item names and personal strings redacted): [`data/0007-uia-enumeration-across-apps.json`](data/0007-uia-enumeration-across-apps.json).

## Analysis

- The hypothesis held for speed and accuracy: every app listed in under 200 ms, far inside the 1.5 s budget, and pointing was exact. UIA rectangles and the overlay share physical pixels, and the single division by the scale factor is correct at 125%.
- Noise, not speed, was the real problem, and it was app-specific: Explorer's details cells and VS Code's icon glyphs and path groups pushed both over the cap. Folding parts into their parent item (a row, a tab) and ignoring private-use glyphs brought them to 83 and 127 with no loss of real targets in a spot check. Expect more such rules per app family; this is where an eval pays off.
- Remaining noise seen in Edge: list items repeat the name of the link inside them (`listitem "Web"` + `link "Web"`). Harmless for now, but it doubles the tokens for navigation menus.
- The lazy Chromium tree means the buddy's first question about a Chromium/Electron app may get an almost empty list. A cheap mitigation is to retry once after a short delay when a `Chrome_WidgetWin_1` window yields only a handful of elements. Not implemented; the right delay is unmeasured.
- The overlay shrinking our own tree is probably Chromium's occlusion logic inside our shared WebView2 browser process (the overlay is a full-screen window of the same browser process). It only matters if we list our own window while pointing. We would not conclude it affects other Chromium apps: Edge and VS Code were unaffected here.
- We would **not** conclude that the 0 px error holds at other scales (100%, 150%, 175%), on secondary monitors or with mixed-DPI setups; only 125% on one monitor was tested.

## Not verified

- **Desktop screenshots of the cursor over each element were not taken.** Screen-capture and window-inspection helper commands were blocked in this session's sandbox, so accuracy was checked numerically from the overlay's own drawn state, plus the overlay webview's own render captured over the debug port. A person should watch a few gestures once.
- **Taskbar and Alt+Tab absence** follow from the window styles (`WS_EX_TOOLWINDOW`, `skipTaskbar`, unit-tested style computation), but were not observed on screen, and the live extended style bits were not read back.
- Time-budget partial results were tested only in unit tests; no live app was slow enough to trigger them.

## Conclusion

UIA enumeration is fast (under 0.2 s) and, after folding item parts and icon glyphs, compact enough (19 to 127 elements) for a model prompt in all five apps. The overlay put the cursor at the exact centre of all 15 targets at 125% scale without taking focus.

## Eval potential

- **Pointing regression (on-device):** for a fixed set of apps and named elements, list, point, read the overlay state; pass if the tip is inside the rectangle for 100% of targets. Needs a desktop session, so manual or on-device CI.
- **List quality:** store sanitised raw element lists like the data file and assert counts under the cap and no known noise patterns (`edit "Date modified"` under a list item, private-use names). Pure Rust, can run in CI from saved fixtures.
- **Element selection (next step for ADR 0005):** feed `render_for_prompt` output plus a question ("where do I make text bold?") to the local model; score whether the chosen number's rectangle matches the labelled answer. Target at least 90% (finding 0001's suggested threshold).

## Reproduce

```bash
cd app && npm run build && npx tauri build --debug --no-bundle
# If Ctrl+Alt+Space is taken by another app, make hotkey::register non-fatal locally first.
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 \
  cmd //c start "" src-tauri/target/debug/kutty-jarvis.exe
cmd //c start "" notepad.exe
# In the main window's webview (debug port 9333, page http://tauri.localhost/):
#   await __TAURI_INTERNALS__.invoke('debug_list_elements', { processId: <notepad pid> })
#   await __TAURI_INTERNALS__.invoke('debug_point_at', { id: 6, holdMs: 2500 })
# In the overlay webview (http://tauri.localhost/overlay.html), read
#   document.querySelector('.overlay-cursor').style.transform and devicePixelRatio.
```

Or by hand: in a debug build, click "List foreground window in 3 s", switch to the app, then enter an element number and click "Point".

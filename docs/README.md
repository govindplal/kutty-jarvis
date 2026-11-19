# Documentation

Everything we learn and decide is written down here. The goal is that a new contributor, a blog reader or an automated eval can reconstruct *what* we did, *why*, and *what evidence* backed it.

| Folder | What goes in it | Format |
|---|---|---|
| [`adr/`](adr/) | Architecture decision records: one decision each, with context, options and consequences. Immutable once accepted; supersede instead of editing. | [Template](adr/0000-template.md) |
| [`findings/`](findings/) | Experiment and spike write-ups: question, method, raw results, conclusion. Written so the experiment can be re-run and turned into an eval. | [Template](findings/0000-template.md) |
| [`research/`](research/) | Background research on the product landscape, models and prior art, with dated sources. | Free-form, cite sources |
| [`devlog/`](devlog/) | One chronological entry per working day: what happened, what surprised us, what broke. Raw material for blog posts. | `YYYY-MM-DD.md` |

## Rules

1. **Decision made? Write an ADR in the same PR that implements it.** Number sequentially, never reuse a number.
2. **Experiment run? Write a finding.** Record the exact versions, hardware, prompts and raw numbers. Save machine-readable results next to it in `findings/data/`.
3. **Something surprising or broken?** Put it in the devlog the same day, including mistakes. Failures are the best blog material.
4. **Link, do not duplicate.** ADRs cite findings; findings cite research; the devlog cites all three.
5. **Dates are absolute** (`2026-11-20`), never "yesterday".
6. **Mark confidence.** Say whether a claim is measured, sourced or assumed.

## Index

### Decisions
- [0001 Record architecture decisions](adr/0001-record-architecture-decisions.md)
- [0002 Build from scratch, clean-room](adr/0002-build-from-scratch-clean-room.md)
- [0003 Tauri over Electron and Python](adr/0003-tauri-over-electron-and-python.md)
- [0004 Local-first, provider-agnostic models](adr/0004-local-first-provider-agnostic-models.md)
- [0005 UI Automation first for pointing](adr/0005-ui-automation-first-pointing.md)
- [0006 Conventional commits and trunk-based flow](adr/0006-conventional-commits-and-trunk-based-flow.md)
- [0007 Speech stack: LFM3-Audio recognition, Piper speech output](adr/0007-speech-stack.md) (proposed)
- [0008 Tray app with a Ctrl+Alt+Space push-to-talk hotkey](adr/0008-tray-app-and-push-to-talk-hotkey.md)
- [0009 Capture the screen only on hotkey press, in memory only](adr/0009-capture-screen-on-hotkey-only.md)
- [0010 Record the microphone only while the key is held](adr/0010-record-microphone-only-while-key-held.md)

### Findings
- [0001 Vision grounding with LFM2.5-VL on CPU](findings/0001-vision-grounding-lfm25-vl-cpu.md)
- [0002 Local speech recognition and synthesis on CPU](findings/0002-local-speech-on-cpu.md)
- [0003 Smaller and newer vision models on CPU](findings/0003-small-vision-models-on-cpu.md) (partly invalid, see the finding)
- [0004 Recording start-up gap and fix](findings/0004-recording-start-latency.md)

### Research
- [0001 HeyClicky and the open-source landscape](research/0001-heyclicky-and-open-source-landscape.md)

### Devlog
- [2026-11-20](devlog/2026-11-20.md)

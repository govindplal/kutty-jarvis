# 0002. Build from scratch, clean-room

- Status: accepted
- Date: 2026-11-20
- Deciders: project owner
- Evidence: [research 0001](../research/0001-heyclicky-and-open-source-landscape.md)

## Context

HeyClicky is a closed-source Mac and Windows app. Its earlier version ([farzaa/clicky](https://github.com/farzaa/clicky)) is MIT-licensed and public, and several community forks exist (Flicky, ClickyX and others). We want a free, open-source product that runs locally on a CPU-only laptop, which none of the forks target.

## Options considered

1. **Fork an existing open-source clone.** Fastest start, but inherits cloud-first design (paid API keys for model, speech-to-text and text-to-speech) and a stack we did not choose.
2. **Reverse engineer the closed HeyClicky app** (for example with the `rea` tool). Rejected: the app's terms of service likely forbid it, and copying prompts or internals would taint an MIT project.
3. **Build from scratch**, using only public information: the MIT-licensed original architecture, public product behaviour and our own design.

## Decision

We build from scratch under the MIT licence. We may study the MIT-licensed `farzaa/clicky` snapshot for architecture ideas (for example the idea of a pointing tag in model output). We do not decompile, intercept or copy anything from the closed HeyClicky app. The project is named **kutty-jarvis**, not "Clicky", to avoid trademark confusion.

## Consequences

- Good: clean licence story, architecture tuned for local-first and CPU-only from day one.
- Cost: slower start than forking; we rebuild basics (overlay, hotkey, audio loop).
- Revisit if a fork converges on our goals and its maintainers welcome the changes.

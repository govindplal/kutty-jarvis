# 0001. Record architecture decisions

- Status: accepted
- Date: 2026-11-20
- Deciders: project owner, Claude Code
- Evidence: none needed

## Context

The project is built in short bursts, partly by an AI coding agent, across several sessions and devices. Reasoning that lives only in chat is lost. The owner also wants to publish blog posts and build evals from what we learn.

## Options considered

1. **Chat history only.** Zero effort, but not searchable, not shareable, not reviewable.
2. **Wiki or Notion.** Easy to write, but drifts from the code and is outside code review.
3. **ADRs and findings in the repository.** Reviewed in the same PR as the change, versioned, public.

## Decision

We keep architecture decision records in `docs/adr/`, experiment findings in `docs/findings/`, background research in `docs/research/` and a daily devlog in `docs/devlog/`, following the rules in [`docs/README.md`](../README.md).

## Consequences

- Good: decisions are reviewable, citable and public; findings can seed evals and blog posts.
- Cost: every decision and experiment needs a few minutes of writing in the same PR.
- Revisit if the docs fall out of date. Reviewers should block PRs that change a decision without an ADR.

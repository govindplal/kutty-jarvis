# 0006. Conventional commits and trunk-based flow

- Status: accepted
- Date: 2026-11-20
- Deciders: project owner
- Evidence: PR #1 (commitlint verified by rejecting a malformed message)

## Context

The owner wants the repository to be run like a professional team: small reviewable changes, a readable history, and automation that enforces the rules for humans and AI agents alike.

## Options considered

1. **Free-form commits, direct pushes to main.** Simple, but history is noisy and mistakes ship.
2. **GitFlow (develop, release branches).** Heavy for a small project.
3. **Trunk-based development with short-lived branches, pull requests and Conventional Commits.**

## Decision

- `main` is protected: pull request required, no force pushes, no deletions, linear history, conversations resolved.
- Short-lived branches named `<type>/<kebab-description>`; squash-merge; branch deleted on merge.
- Conventional Commits enforced locally by commitlint through a husky hook (header at most 72 characters, body lines at most 72) and a pre-push hook that validates branch names.
- Each important step is its own commit with a specific message; one logical change per commit.

## Consequences

- Good: readable history, automatic changelog potential, uniform behaviour for human and AI contributors.
- Cost: friction on small changes. The hooks rejected our own first commits for over-long body lines, which proved they work.
- Follow-up: add the same checks in CI so they cannot be bypassed locally.

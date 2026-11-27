# 0011. Rebase-merge pull requests instead of squash-merging

- Status: accepted
- Date: 2026-11-27
- Deciders: project owner
- Supersedes: the merge-method line of [ADR 0006](0006-conventional-commits-and-trunk-based-flow.md)

## Context

ADR 0006 chose squash-merge. Each branch already holds small commits with specific Conventional Commit messages, and squashing collapsed them into one. Squash merges also left the merged branches conflicting with `main` (see STATUS, "Learned this project").

## Options considered

1. **Keep squash-merge.** One commit per PR; loses the per-step history.
2. **Merge commits.** Keeps every commit, but needs the "require linear history" rule turned off.
3. **Rebase-merge.** Every branch commit lands on `main` individually and history stays linear.

## Decision

Rebase-merge. The repository allows only rebase merges; squash and merge commits are disabled. Branch protection (linear history, required checks, branch up to date) is unchanged.

## Consequences

- Good: each step keeps its own commit on `main`; no branch-versus-main conflicts after a merge.
- Cost: every commit on a branch must pass commitlint and stand on its own, because none of them is squashed away. Fix up messy commits before opening the PR.

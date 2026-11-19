# Contributing

## Branches

`main` is always releasable and protected: no direct pushes, changes land through pull requests.

Branch from `main` as `<type>/<short-kebab-description>`, using the same types as commits:

`feat/push-to-talk-hotkey`, `fix/overlay-flicker-multi-monitor`, `docs/contributing-guide`, `chore/commitlint-setup`

## Commits

We follow [Conventional Commits](https://www.conventionalcommits.org/), enforced by commitlint on every commit.

```
<type>(<optional scope>): <imperative summary, lower case, no trailing period>

<optional body: what and why, wrapped at 72 columns>

<optional footer: BREAKING CHANGE:, Closes #123>
```

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
- Header at most 72 characters. One logical change per commit.
- Explain *why* in the body when it is not obvious.

## Pull requests

- One feature or fix per PR, kept small.
- Tests and lint must pass before review.
- Squash-merge with a Conventional Commit title.

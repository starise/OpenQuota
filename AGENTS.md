## Agent skills

### Issue tracker

For GitHub issues and specs, read [the tracker instructions](docs/agents/issue-tracker.md).

### Triage labels

When applying a triage role, read [the label mapping](docs/agents/triage-labels.md).

### Domain docs

Before design or implementation, read [the domain documentation guide](docs/agents/domain.md).

## Browser tests

Run `pnpm test:e2e` when changing browser interactions such as keyboard navigation, focus, or drag and drop.
If Chromium is missing, install it with `pnpm exec playwright install --only-shell chromium`.

## Commit policy

All commits MUST use the Conventional Commits specs with these additional rules:

- Follow the 50/72 rule:
  - Subject: maximum 50 characters, excluding the scope and its parentheses.
  - Body (if present): maximum 72 characters per line.
- Each commit MUST represent a single logical change.
- Do NOT mix unrelated changes in the same commit.
- Commit messages MUST accurately describe the actual changes.

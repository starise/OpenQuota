# Issue tracker

OpenQuota issues and specs live in GitHub Issues for `starise/OpenQuota`. Pass
`--repo starise/OpenQuota` to every `gh issue` and `gh label` command. For
`gh api`, use an explicit `repos/starise/OpenQuota/...` path. Before writing,
verify that `gh api user --jq .login` returns `starise`.

## Issue operations

- Publish a spec or ticket with `gh issue create --repo starise/OpenQuota --title "..." --body-file <path>`.
- Read a ticket with `gh issue view <number> --repo starise/OpenQuota --json number,title,body,labels,comments`.
- Find tickets with `gh issue list --repo starise/OpenQuota` and the relevant state and label filters.
- Comment with `gh issue comment <number> --repo starise/OpenQuota --body-file <path>`.
- Change labels with `gh issue edit <number> --repo starise/OpenQuota` and the relevant
  `--add-label` or `--remove-label` option. Use the [triage mapping](triage-labels.md).
- Resolve an issue with `gh issue close <number> --repo starise/OpenQuota --comment "..."`.

## Sub-issues and blocking dependencies

- Add a child with `gh issue create --parent <parent> ...` or
  `gh issue edit <parent> --add-sub-issue <child> ...`.
- Record a dependency on the blocked ticket with `gh issue create --blocked-by <blocker> ...` or
  `gh issue edit <child> --add-blocked-by <blocker> ...`.
- Check the remaining blockers with
  `gh issue view <number> --repo starise/OpenQuota --json blockedBy --jq '[.blockedBy.nodes[] | select(.state == "OPEN")] | length'`.
  A ticket is ready to start when this count is zero.
- If native links are unavailable, record `Part of #<parent>` or `Blocked by: #<blocker>`
  in the child issue body and check each listed blocker before starting.

## Pull requests as a triage surface

**PRs as a request surface: no.**

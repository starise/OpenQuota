# Load model prices at runtime

OpenQuota will fetch the LiteLLM and models.dev catalogs in the background and keep a local cache
because provider quotas and token usage can be shown without those prices. The curated supplement
ships with the app and receives updates from `starise/OpenQuota`. To preserve spend estimates when
a catalog drops a model, keep its last known rate and use it after current exact prices and before
approximate matches.

## Consequences

- Builds can run without fetching either catalog. Upgrades reuse existing pricing caches. If a
  model used by a supported provider would lose a price that exists only in the old bundle, add
  it to the supplement when the rate can be verified.
- Missing prices leave all recorded tokens visible. Reported costs are preserved, partial spend
  is marked, and cost per million tokens is available only with complete pricing coverage.
  New prices take effect on the next usage refresh.

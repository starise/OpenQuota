# Kimi

OpenQuota tracks the Session (rolling five-hour) and Monthly quotas of a Kimi Code membership. It
also keeps the legacy Weekly metric when Kimi includes that pool in the response.

## What it tracks

| Metric  | Meaning                                                |
| ------- | ------------------------------------------------------ |
| Session | Usage remaining in the rolling 5-hour window           |
| Monthly | Usage remaining in the monthly total pool              |
| Weekly  | Legacy rolling 7-day pool, when Kimi reports the value |

Weekly is available in the on-demand metrics. Older Kimi responses may still include it; newer
responses that omit it do not produce a Weekly quota.

## Setup

Create a Kimi Code API key in the [Kimi Code Console](https://www.kimi.com/code/console), then add
it in **Customize** in OpenQuota. Saved keys are stored in the operating system's credential store.
OpenQuota also checks `KIMI_API_KEY` and `~/.config/openquota/kimi.json`; a key saved in the app
takes priority.

This provider uses the Kimi Code endpoint, `https://api.kimi.com/coding/v1`. Kimi Code keys are not
interchangeable with Kimi Open Platform keys.

## Troubleshooting

- **Add an API key** — add a Kimi Code key in Customize or provide it through a supported external source.
- **API key invalid** — create or verify the key in the [Kimi Code Console](https://www.kimi.com/code/console).
- **Usage unavailable** — check the connection and refresh again.

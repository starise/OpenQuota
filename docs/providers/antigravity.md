# Antigravity

OpenQuota tracks the shared Gemini and Claude quota pools reported by Antigravity.

## What it tracks

| Metric        | Meaning                                             |
| ------------- | --------------------------------------------------- |
| Session       | Shared Gemini pool in the rolling 5-hour window     |
| Weekly        | Shared Gemini pool in the weekly window             |
| Claude        | Shared non-Gemini pool in the rolling 5-hour window |
| Claude Weekly | Shared non-Gemini pool in the weekly window         |

Only the pools available to the signed-in account are shown.

## Sign-in and local data

Sign in through Antigravity or run `agy`. OpenQuota can read the quota from a compatible running
Antigravity language server or from the signed-in CLI's `/usage` report. The report requires `agy`
1.1.11 or later on `PATH` and works with the CLI closed. If neither option is available, OpenQuota
can reuse Antigravity's locally stored credentials. No API key is required.

## Troubleshooting

- **Not signed in** — open Antigravity or run `agy`, complete sign-in, then refresh OpenQuota.
- **Sign-in expired** — authenticate again in Antigravity or with `agy`.
- **Credentials could not be read** — reopen Antigravity so its local sign-in state can be restored.
- **Usage temporarily unavailable** — wait briefly and refresh again.

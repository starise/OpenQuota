# Antigravity token usage

## Documented sources

- [Model quotas](https://antigravity.google/docs/cli/commands/usage/) describes
  `/usage` as a quota report for model groups. On Windows, `agy 1.5.18`
  returned quota groups under `command.data.groups` and zero invocation
  tokens for the command itself.
- [Headless mode](https://antigravity.google/docs/cli/headless/) exposes token
  counts in JSON results and in streamed step updates. A result can accumulate
  multiple turns. The [CLI status line](https://antigravity.google/docs/cli/statusline/)
  also exposes live context counters and the transcript path.
- [Hooks](https://antigravity.google/docs/hooks/) documents persistent
  `brain/<conversation-id>/.system_generated/logs/transcript.jsonl` paths under
  `~/.gemini/antigravity-cli`, `~/.gemini/antigravity` (app), and
  `~/.gemini/antigravity-ide` (IDE). The documentation describes the path in
  the hook payload. It provides no versioned schema for transcript rows.
- [Sidecars](https://antigravity.google/docs/sidecars#agentapi) documents
  creating conversations and sending messages, with no history export
  command. The [SDK](https://antigravity.google/docs/sdk/lifecycle) exposes
  per-turn usage metadata for calls made through the SDK.

## Windows verification

Five existing CLI transcripts contained 17 `PLANNER_RESPONSE` rows with
per-step input, cache-read and output counts, a model identifier, a timestamp
and a step index. The app and IDE data directories contained saved
conversations but no `transcript.jsonl` files on this machine.

A new single-turn headless probe reported 12,713 input and 104 output tokens
in both the stream and transcript. It reported 99 thinking tokens in the
stream, already included in the output count. The transcript did not expose
a separate numeric thinking count.

A second probe sent two prompts through one NDJSON session. Both transcript
rows matched their corresponding completed stream steps:

| Step |  Input | Cache read | Output |
| ---- | -----: | ---------: | -----: |
| 1    | 12,711 |          0 |     64 |
| 3    |  2,978 |     10,662 |     38 |

The second streamed result accumulated the two turns. Its input count was
15,689, its cache-read count was 10,662, and its output count was 102. For
these steps, input and cache reads are separate quantities. Adding them gives
26,351 prompt tokens, including cached input. Adding the output gives 26,453
tokens processed. These observations apply to the tested client and model
(`gemini-4-hydrogen-medium`).

## Conversation databases

A read-only check of all ten CLI SQLite databases under
`~/.gemini/antigravity-cli/conversations/` decoded the Protobuf BLOBs in
`gen_metadata.data`. The seven databases with generation data contained
20 records with token counts, a model ID and timestamp metadata. All 20
matched their corresponding transcript steps for input, cache reads and
output. The three databases without transcripts contained no generation rows.
The two-turn probe also recorded thinking and response output separately:
59 + 5 and 33 + 5, respectively. Those sums reproduce the output counts above.
See [CodeBurn's storage format notes](https://github.com/getagentseal/codeburn/blob/main/docs/providers/antigravity.md#storage-format)
for the field layout used to guide the local check.

The local desktop and IDE conversation directories held byte-identical
`.pb` files. Their binary payloads were not decoded. CodeBurn reports SQLite
storage for newer desktop and IDE conversations and uses the running
language server for older `.pb` files. Both paths remain unverified locally
for these clients.

## OpenUsage comparison

The [historical OpenUsage scanner](https://github.com/robinebers/openusage/blob/60f29f9ec5d32536a12d4ee8843c76f7f4a55e6a/Sources/OpenUsage/Providers/Antigravity/AntigravityDbUsageScanner.swift)
reads `gen_metadata.data` from SQLite `.db` files under every
`~/.gemini/antigravity*/conversations/` directory. When the generation omits
its timestamp, it reads `steps.metadata` at the same index. It batches reads,
tracks database and WAL changes for incremental refreshes, and deduplicates
paths that resolve to the same file through symlinks. It prices usage by model,
using the display name to resolve placeholder model IDs where possible. The
scanner's discovery excludes `.pb` files.

OpenUsage removed local spend tracking in the
[API-only rewrite](https://github.com/robinebers/openusage/pull/1444).
[OpenUsage v0.8.0](https://github.com/robinebers/openusage/releases/tag/v0.8.0)
uses signed-in Google API quota data. The historical scanner remains a reference
for reading the local SQLite format.

## OpenQuota integration

The [SQLite reader](../../src-tauri/src/providers/antigravity/local_usage.rs)
checks the CLI, desktop and IDE conversation directories. It reads committed
WAL data in a read-only transaction. It counts each generation index once
per conversation ID, with the filename as a fallback when the SQLite
metadata lacks an ID. It uses the embedded timestamp when available, then
the referenced steps. The generation index is used for step lookup only
when the generation lists no steps. Records without a usable date are
skipped.

The reader uses aggregate output when present. It sums thinking and response
output when the aggregate is missing. Cache input remains separate, and
cache writes use the catalog's regular cache-write rate. The anonymized
fixture covers both output forms, unknown or malformed fields, duplicate
conversations, a live WAL transaction and an exclusive database lock.

An empty compatible database contributes no history. Unsupported databases
and records are skipped with a warning. Records larger than 4 MiB are
treated as unsupported. A temporarily inaccessible store triggers the
existing cached-history behavior. Antigravity's quota refresh keeps its
existing source selection and errors. A successful quota refresh also
refreshes local history.

The [runtime pricing decision](../adr/0001-load-model-prices-at-runtime.md)
allows token counts to remain visible when model prices are missing. Any
dollar amount derived from model rates would be estimated spend. Google
describes included personal-plan quota and overage credits separately in
[Plans](https://antigravity.google/docs/plans?hl=en).

The Windows CLI format was verified with `agy 1.5.18`. A live OpenQuota
provider refresh returned three quota windows and 295,001 tokens across the
local CLI databases. The token total matched the independent transcript
sum.

The desktop app
(`2.34.0`) and standalone IDE (`2.8.0`) were checked separately for local
conversation files. Both held legacy `.pb` files. The IDE's `chat` command
opened the IDE and produced a `.done` marker without a conversation
database. Windows UI automation was unavailable, which prevented a fresh
desktop conversation and confirmation of the IDE prompt in its UI.
OpenQuota does not read `.pb` files or JSONL transcripts. Remote sessions,
separate subagent stores and other model families remain unverified.

<h1 align="center">
  <img src="assets/openquota-icon.png" alt="OpenQuota logo" width="48" align="absmiddle">
  &nbsp;OpenQuota
</h1>

<p align="center">
  <strong>Track usage and limits across your AI coding tools.</strong><br>
  <em>Maintenance-first. Stability-focused.</em><br>
</p>

<p align="center">
  <a href="#project-philosophy"><img src="https://img.shields.io/badge/status-actively%20maintained-brightgreen" alt="Actively maintained"></a>
  <a href="https://github.com/starise/OpenQuota/actions/workflows/ci.yml"><img src="https://github.com/starise/OpenQuota/actions/workflows/ci.yml/badge.svg" alt="CI status"></a>
  <a href="https://github.com/starise/OpenQuota/releases/latest"><img src="https://img.shields.io/github/v/release/starise/OpenQuota" alt="Latest release"></a>
  <a href="https://github.com/starise/OpenQuota/releases"><img src="https://img.shields.io/github/downloads/starise/OpenQuota/total" alt="Total downloads"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT license"></a>
</p>

<p align="center">
  Windows &middot; macOS &middot; Linux
</p>

<p align="center">
  <a href="https://github.com/starise/OpenQuota/releases"><strong>Download</strong></a> &middot;
  <a href="#supported-providers">Supported providers</a> &middot;
  <a href="#features">Features</a> &middot;
  <a href="https://github.com/starise/OpenQuota/issues">Report an issue</a>
</p>

---

OpenQuota brings usage data from Claude Code, Codex, Cursor, Copilot, and other AI coding providers into one compact desktop dashboard. Monitor session and weekly limits, reset times, token usage, and estimated spend without switching between tools.

<p align="center">
  <img src="assets/openquota-demo.gif" alt="OpenQuota dashboard showing AI usage in light and dark themes" width="840">
</p>

## Download

Visit the [releases page](https://github.com/starise/OpenQuota/releases) for available builds and release notes.

| Platform    | Architecture          | Package                            |
| :---------- | :-------------------- | :--------------------------------- |
| **Windows** | x64 / ARM64           | NSIS installer (`.exe`)            |
| **macOS**   | Apple Silicon / Intel | Universal DMG (`.dmg`)             |
| **Linux**   | x64 / ARM64           | AppImage / Debian package (`.deb`) |

Windows installers use the `_x64-setup.exe` or `_arm64-setup.exe` suffix. The macOS `_universal.dmg` requires macOS 11 or later. Linux builds are provided as `.AppImage` or `.deb` packages when available.

### Updates and signing

OpenQuota supports automatic updates using cryptographically signed update payloads. Updater signatures are independent of native operating-system package signing. Check the release notes for updater availability and native-signing status.

> [!IMPORTANT]
> Releases without native signing may trigger Windows SmartScreen or macOS Gatekeeper warnings. On macOS, manual approval may be required in Privacy & Security. Download builds of this fork only from the [starise/OpenQuota releases page](https://github.com/starise/OpenQuota/releases).

## Supported providers

| Provider                                         | Available metrics                                                                                  |
| :----------------------------------------------- | :------------------------------------------------------------------------------------------------- |
| [**Claude Code**](docs/providers/claude.md)      | Multiple accounts, session and weekly limits, model-specific usage, token history, estimated spend |
| [**Codex**](docs/providers/codex.md)             | Session and weekly limits, credits, token history, model breakdown, estimated spend                |
| [**Cursor**](docs/providers/cursor.md)           | Total, Auto and API usage, credits, token history, estimated spend                                 |
| [**Antigravity**](docs/providers/antigravity.md) | Shared Gemini and Claude quota pools                                                               |
| [**Copilot**](docs/providers/copilot.md)         | Premium requests, extra usage, chat and completion quotas, organization billing                    |
| [**Devin**](docs/providers/devin.md)             | Daily and weekly limits, reset times, extra usage balance                                          |
| [**Grok**](docs/providers/grok.md)               | Weekly allowance, extra usage status, token history, estimated spend                               |
| [**OpenCode**](docs/providers/opencode.md)       | OpenCode Go session, weekly and monthly spend caps, local hosted usage history                     |
| [**OpenRouter**](docs/providers/openrouter.md)   | Credit balance, daily, weekly and monthly spend                                                    |
| [**Z.ai**](docs/providers/zai.md)                | GLM Coding Plan session, weekly and web-search quotas                                              |
| [**Kimi**](docs/providers/kimi.md)               | Kimi Code session and weekly quotas                                                                |
| [**MiniMax**](docs/providers/minimax.md)         | Token Plan session and weekly quotas                                                               |

Most providers use credentials already available on your computer. OpenRouter, Z.ai, Kimi, and MiniMax require API keys, which can be added in Customize and are stored in your operating system's credential store.

Codex subscription limits require a ChatGPT login and are not available in API-key-only sessions.

## Features

- **Tray or floating dashboard** — View quotas in a compact popup or keep the panel open anywhere on your desktop.
- **Pinned metrics** — Keep important values visible in the system tray or macOS menu bar.
- **Used or remaining quota** — Choose how usage is displayed.
- **Usage history** — Review today, yesterday, and the last 30 days of token usage and estimated spend.
- **Pacing alerts** — See whether your current usage is likely to last until the next reset.
- **Custom layouts** — Reorder providers and metrics, hide rows, and choose what stays visible.
- **Desktop integration** — Launch at login, use a global shortcut, and follow the system theme.
- **Fast refresh** — Cached values appear immediately while providers refresh in the background.

## Privacy

OpenQuota runs locally, with no required account, cloud backend, analytics, or usage telemetry of its own. It communicates with supported providers as needed to retrieve usage information. API keys configured in the app are stored in the operating system's credential store.

## Project philosophy

**Keep OpenQuota useful, reliable, and up to date.**

The goal of this independent continuation is to preserve the original application's simplicity and functionality while maintaining compatibility with an evolving ecosystem of AI coding tools.

Development prioritizes:

- Bug fixes and stability improvements.
- Dependency and security updates.
- Compatibility with evolving provider APIs and services.
- Small, meaningful quality-of-life improvements.
- Preservation of the original user experience whenever practical.

New features are welcome when they provide clear value without introducing unnecessary complexity or long-term maintenance costs. There is no commitment to a fixed release schedule.

## Development

### Requirements

- Node.js 22 or later
- pnpm 11.11.0
- Stable Rust toolchain
- [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/)

### Run locally

```sh
git clone https://github.com/starise/OpenQuota.git
cd OpenQuota
pnpm install --frozen-lockfile
pnpm tauri dev
```

### Quality checks

Run the complete verification suite:

```sh
pnpm verify
```

### Build

```sh
pnpm build:installer             # Windows
pnpm build:linux                 # Linux
pnpm tauri build --bundles dmg   # macOS
```

See [docs/releasing.md](docs/releasing.md) for release, updater, and optional native-signing requirements.

## Contributing

Bug reports, fixes, provider compatibility updates, and focused improvements are welcome.

- [Open an issue](https://github.com/starise/OpenQuota/issues) to report a bug or suggest an improvement.
- [Submit a pull request](https://github.com/starise/OpenQuota/pulls) to contribute a change.
- Read [CONTRIBUTING.md](CONTRIBUTING.md) before contributing.
- Follow [SECURITY.md](SECURITY.md) for private vulnerability reporting.

---

<p align="center">
  <sub>Originally created by <a href="https://github.com/deviffyy">deviffyy</a> &middot; Maintained by <a href="https://github.com/starise">starise</a></sub>
</p>

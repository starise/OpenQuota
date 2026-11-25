# Releasing OpenQuota

OpenQuota treats updater signatures and native operating-system signatures as separate trust
layers. Updater artifacts must always be signed with `TAURI_SIGNING_PRIVATE_KEY`. Native Windows
and macOS signing are independent opt-ins because they require externally provisioned certificates.
If the updater key is encrypted, also configure `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The updater
key is project-generated and does not require a paid certificate authority or signing account.

## Protect the signing credentials

Create the `release-signing` GitHub environment before running the release workflow. Under
**Deployment branches and tags**, allow the `main` branch and `v*` tags. Protect creation and updates
of the release tags with a tag ruleset.

Add the updater key, its optional password, and any enabled native-signing secrets to
`release-signing`. Remove copies of those secrets from the repository or organization scope after
the migration. Keep the native-signing opt-ins and signer subject as repository variables.
See [GitHub's environment setup guide](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments).

## Default release policy

Leave both native-signing repository variables unset or set them to `false`:

- `ENABLE_WINDOWS_NATIVE_SIGNING`
- `ENABLE_MACOS_NATIVE_SIGNING`

The release workflow then builds an unsigned Windows installer and an ad-hoc-signed, unnotarized
macOS application. Package installation and startup smoke tests still run, but Authenticode,
Gatekeeper, and notarization checks are skipped. The workflow emits warnings, and the download
documentation describes the unavailable native trust layers.

This default does not weaken updater verification. Tauri updater signatures are still generated,
uploaded, and verified with the bundled public key before publication.

## Enabling Windows native signing

Set `ENABLE_WINDOWS_NATIVE_SIGNING` to `true` only after configuring all of the following:

| Kind               | Name                     |
| ------------------ | ------------------------ |
| Environment secret | `ES_USERNAME`            |
| Environment secret | `ES_PASSWORD`            |
| Environment secret | `ES_CREDENTIAL_ID`       |
| Environment secret | `ES_TOTP_SECRET`         |
| Actions variable   | `WINDOWS_SIGNER_SUBJECT` |

This enables the reviewed SSL.com CodeSignTool configuration. The workflow then requires a valid,
timestamped Authenticode signature on both the installer and installed executable. Missing or
incorrect values stop the release rather than silently producing an unsigned Windows artifact.

## Enabling macOS native signing

Set `ENABLE_MACOS_NATIVE_SIGNING` to `true` only after configuring all of the following environment
secrets:

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_ID`
- `APPLE_PASSWORD`
- `APPLE_TEAM_ID`

`APPLE_PASSWORD` is an app-specific password used for notarization, not the account's normal login
password.

The direct-download DMG uses a Developer ID Application certificate, not an App Store Distribution
certificate. When enabled, the workflow requires the expected team identity, hardened runtime,
secure timestamp, Gatekeeper approval, and a valid notarization staple. Missing or incorrect values
stop the release rather than falling back to ad-hoc signing.

Both opt-ins accept only the exact strings `true` and `false`. An invalid value stops validation so a
typo cannot silently change release trust policy.

## Run and recover a release

Push a `vMAJOR.MINOR.PATCH` tag on `main` or start the **Release** workflow from `main` with an
existing tag.
Release validation runs `pnpm verify`, the same command used locally. Publication requires the
Windows and Linux build and smoke matrix to succeed and their updater signatures to verify.

GitHub Actions builds the macOS universal package on an Intel runner. The release includes macOS
when the DMG smoke test and updater signature verification succeed. When the runner or package
fails validation, Windows and Linux can still be published. macOS keeps its previous verified
version in `latest-macos.json`, which is carried forward unchanged. The first release without a
verified Mac package has no macOS manifest. A published release is never backfilled by a rerun.

**CI** and **macOS CI** run code checks on pull requests and pushes to `main`. **Release** builds
and smoke-tests the packages. To run the macOS code checks manually, choose `macos-26-intel` for
Intel or `macos-26` or `macos-15` for Apple Silicon.

For a transient failure, use **Re-run failed jobs** on the original release run. If you need to
change the signing policy or replace a draft asset, start a full **Release** run for the tag so
that all packages are built and smoke-tested again.

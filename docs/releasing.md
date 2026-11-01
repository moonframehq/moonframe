# Releasing

## Workflows

| Workflow | Runs on | Does |
| --- | --- | --- |
| `ci.yml` | every pull request, pushes to `main` | zizmor on the workflows; formatting, `Cargo.lock`, Luau types and Wasm providers; clippy, tests, the headless `--check` and the component catalogue on macOS, Linux x86_64/ARM64 and Windows x86_64 |
| `package.yml` | pull requests touching packaging, each release | optimized builds and the unsigned packages for every platform, each checked headlessly against the app it ships |
| `supply-chain.yml` | dependency changes, weekly | cargo-deny: advisories, bans, licenses, sources; advisories for the Wasm providers' own lockfiles |
| `release.yml` | `just release` | the release, below |

Every action is pinned to a commit SHA (Dependabot moves them, after a
seven-day cooldown), every job declares the permissions it needs and no
more, checkouts never keep credentials, and no job that feeds a release uses
a cache. `just audit` runs the same zizmor, actionlint and cargo-deny checks
locally.

## Making a release

1. Set `version` under `[workspace.package]` in `Cargo.toml` and merge it to
   `main`. The release is tagged `v<version>` and refuses a version that
   already has a tag or a release.
2. On an up-to-date, clean checkout of `main`, run `just release`.

The workflow then:

1. runs zizmor on itself and checks the requested commit is `main`'s HEAD;
2. builds every package (`package.yml`) and the CycloneDX SBOMs, while it
   waits for CI to pass on that same commit; nothing below runs until it has;
3. signs the app with the Developer ID, notarizes and staples it, and builds
   the disk image around it, signed, notarized and stapled too (when signing
   is set up, see below);
4. writes `SHA256SUMS`, signs every file with Sigstore (keyless, as this
   workflow), and records GitHub attestations of SLSA build provenance and
   of the SBOM for every file;
5. verifies all of that as a user would, creates the tag and a draft release
   with generated notes, checks the uploaded files against `SHA256SUMS`, and
   publishes it.

Only the `sign-macos`, `attest` and `publish` jobs enter the `release`
environment. Protect it (Settings → Environments) so it only accepts
`main`, and add yourself as a required reviewer if releases should wait for
an approval.

## What a release contains

| File | For |
| --- | --- |
| `Moonframe-<v>-universal-apple-darwin.dmg` | macOS 13+, Apple Silicon and Intel: drag Moonframe onto the Applications shortcut |
| `Moonframe-<v>-{x86_64,aarch64}-unknown-linux-gnu.tar.gz` | Linux: run `moonframe` from the unpacked folder |
| `Moonframe-<v>-x86_64-pc-windows-msvc.zip` | Windows: run `moonframe.exe` from the unpacked folder |
| `Moonframe-<v>.cdx.json`, `Moonframe-<v>-provider-*.cdx.json` | the SBOMs of the executable and of each prebuilt Wasm provider |
| `SHA256SUMS` | the checksum of every file above |
| `*.sigstore.json` | the Sigstore bundle of each file |

The Linux and Windows folders hold the executable with the built-in app
beside it in `app/`, where the executable looks for it.

## Verifying a download

```sh
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify Moonframe-<v>-universal-apple-darwin.dmg --repo penso/moonframe
cosign verify-blob Moonframe-<v>-universal-apple-darwin.dmg \
  --bundle Moonframe-<v>-universal-apple-darwin.dmg.sigstore.json \
  --certificate-identity https://github.com/penso/moonframe/.github/workflows/release.yml@refs/heads/main \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Setting up macOS signing

Without it, releases still publish, but the disk image's app is signed ad
hoc: Gatekeeper refuses to open it until the user allows it in System
Settings → Privacy & Security. To sign and notarize, add to the `release`
environment:

| Name | Kind | Value |
| --- | --- | --- |
| `MACOS_SIGNING_IDENTITY` | variable | `Developer ID Application: Name (TEAMID)`; setting it turns signing on |
| `MACOS_CERTIFICATE_P12_BASE64` | secret | the Developer ID Application certificate and key, exported as .p12, base64 |
| `MACOS_CERTIFICATE_PASSWORD` | secret | that .p12's password |
| `APPLE_API_KEY_ID` | variable | an App Store Connect API key's ID, for notarization |
| `APPLE_API_ISSUER_ID` | variable | its issuer ID |
| `APPLE_API_PRIVATE_KEY` | secret | its `.p8` contents |

`MACOS_SIGNING_IDENTITY` is read outside the environment too, to decide
whether to sign, so set it as a repository variable as well.

The app runs under the hardened runtime with the entitlements in
`assets/macos/Moonframe.entitlements`: wasmtime compiles the Wasm providers
to executable memory, which the hardened runtime otherwise refuses.

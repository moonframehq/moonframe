# Releasing

## Workflows

| Workflow | Runs on | Does |
| --- | --- | --- |
| `ci.yml` | every pull request, pushes to `main` | only what the changed files can affect (a `Changes` job decides; a website- or docs-only change runs no build): zizmor on the workflows; formatting, `Cargo.lock`, Luau types (the app and the examples) and WebAssembly (WASM) providers; clippy, tests, the headless `--check` of the app and every example app, and the component catalogue on macOS, Linux x86_64/ARM64 and Windows x86_64 |
| `package.yml` | pull requests touching packaging, each release | optimized builds and the unsigned packages for every platform, each checked headlessly against the app it ships |
| `supply-chain.yml` | dependency changes, weekly | cargo-deny: advisories, bans, licenses, sources; advisories for the WASM providers' own lockfiles |
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
   waits for CI to pass on that same commit; nothing below runs until it has.
   To get macOS runners sooner, it cancels the CI runs of `main`'s other
   commits, and re-runs its own commit's once if it finds it cancelled;
3. signs the app with the Developer ID, notarizes and staples it, and builds
   the disk image around it, signed, notarized and stapled too; and signs
   the Windows executable with Authenticode (each when its signing is set
   up, see below);
4. writes `SHA256SUMS`, signs every file with Sigstore (keyless, as this
   workflow), and records GitHub attestations of SLSA build provenance and
   of the SBOM for every file;
5. verifies all of that as a user would, creates the tag and a draft release
   with generated notes, checks the uploaded files against `SHA256SUMS`, and
   publishes it.
6. points the Homebrew cask, `Casks/moonframe.rb` in
   [moonframehq/homebrew-tap](https://github.com/moonframehq/homebrew-tap), at the
   published disk image (only for a notarized disk image and a version
   without a pre-release suffix).

Only the `sign-macos`, `sign-windows`, `attest` and `publish` jobs enter
the `release` environment. Only the `homebrew` job enters the `homebrew`
environment, which holds `HOMEBREW_TAP_SSH_KEY`: the private half of a
deploy key with write access to moonframehq/homebrew-tap and nothing else. Protect it (Settings → Environments) so it only accepts
`main`, and add yourself as a required reviewer if releases should wait for
an approval.

## What a release contains

| File | For |
| --- | --- |
| `Moonframe-<v>-universal-apple-darwin.dmg` | macOS 13+, Apple Silicon and Intel: drag Moonframe onto the Applications shortcut |
| `Moonframe-<v>-{x86_64,aarch64}-unknown-linux-gnu.tar.gz` | Linux: run `moonframe` from the unpacked folder |
| `Moonframe-<v>-x86_64-pc-windows-msvc.zip` | Windows: run `moonframe.exe` from the unpacked folder |
| `Moonframe-<v>.cdx.json`, `Moonframe-<v>-provider-*.cdx.json` | the SBOMs of the executable and of each prebuilt WASM provider |
| `SHA256SUMS` | the checksum of every file above |
| `*.sigstore.json` | the Sigstore bundle of each file |

The Linux and Windows folders hold the executable with the built-in app
beside it in `app/`, where the executable looks for it.

## Verifying a download

```sh
sha256sum --check --ignore-missing SHA256SUMS
gh attestation verify Moonframe-<v>-universal-apple-darwin.dmg --repo moonframehq/moonframe
cosign verify-blob Moonframe-<v>-universal-apple-darwin.dmg \
  --bundle Moonframe-<v>-universal-apple-darwin.dmg.sigstore.json \
  --certificate-identity https://github.com/moonframehq/moonframe/.github/workflows/release.yml@refs/heads/main \
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

The disk image opens on a laid-out window: the app and an Applications
shortcut over a background with an arrow between them, no toolbar, and the
app's icon on the volume. `scripts/release/make-dmg.sh` builds it with
dmgbuild, pinned by hash in `assets/macos/dmg/requirements.txt` and
installed as wheels into a throwaway virtualenv, because only a layout
written against the volume being built keeps its background. The window is
`assets/macos/dmg/settings.py`; the background is drawn by
`scripts/draw-dmg-background.swift`.

The app runs under the hardened runtime with the entitlements in
`assets/macos/Moonframe.entitlements`: wasmtime compiles the WASM providers
to executable memory, which the hardened runtime otherwise refuses.

## Setting up Windows signing

Without it, the zip's `moonframe.exe` is unsigned and SmartScreen warns on
first launch. It is signed with Azure Artifact Signing, under the publisher
`Fabien Penso`, by the account and certificate profile every one of the
author's apps shares. There is no key, certificate or secret to store: the
`sign-windows` job exchanges the `release` environment's GitHub OIDC token
for an Azure token as the `moonframe-signing` app registration, whose
federated credential trusts only
`repo:moonframehq@351295873/moonframe@1441198486:environment:release` (GitHub
names the owner and repository with their ids too, so a rename or transfer
needs a new credential) and which holds the *Artifact Signing Certificate Profile Signer* role on
that profile. Add to the `release` environment, all as variables:

| Name | Value |
| --- | --- |
| `AZURE_CLIENT_ID` | the `moonframe-signing` app registration's application ID |
| `AZURE_TENANT_ID` | its tenant |
| `AZURE_SUBSCRIPTION_ID` | the subscription holding the signing account |
| `ARTIFACT_SIGNING_ENDPOINT` | the signing account's regional endpoint |
| `ARTIFACT_SIGNING_ACCOUNT` | the signing account |
| `ARTIFACT_SIGNING_PROFILE` | the certificate profile |

and, as a repository variable (setting it turns signing on),
`WINDOWS_SIGNING_SUBJECT`: the certificate's exact subject, which the job
requires of every signature, with a timestamp, before it zips the folder
again.

When the publisher's identity validation in Azure expires, signing fails
until it is renewed (Artifact Signing account → Identity validations).

## Homebrew

`brew install moonframehq/tap/moonframe` installs the disk image's app and links
its `moonframe` command. The cask is `homebrew/Casks/moonframe.rb` here,
with the version and checksum left as `@VERSION@` and `@SHA256@`:
`scripts/release/render-cask.sh` fills them in from the published disk
image, and `scripts/release/update-homebrew.sh` commits the result to the
tap. It refuses to go back to an older version or to change one already
there, and it never runs the tap's Ruby. Do not edit the tap's copy by hand:
the next release overwrites it.

To replace the deploy key, generate a new one, add its public half to
moonframehq/homebrew-tap (Settings → Deploy keys, with write access), set the
private half as the `homebrew` environment's `HOMEBREW_TAP_SSH_KEY`, and
delete the old key.

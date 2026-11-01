# Updates

An app on Moonframe can update itself from its GitHub releases. Name the
repository it is released from, and the app menu gets **Check for
Updates…**, under About:

- It asks GitHub for the repository's newest release
  (`/repos/OWNER/REPO/releases/latest`: published, not a draft or
  pre-release) and compares the release's tag with the running version.
- If the release is newer, a dialog offers it. On macOS, an app running from
  its own `.app` gets **Install and Relaunch**: the release's disk image (or
  zip) downloads, is checked against the release's `SHA256SUMS` when there is
  one, must hold an app with the same bundle id, signed by the same team as
  the running app, and takes the old app's place once it quits; then the
  new one opens. Anywhere else (Linux, Windows, a run from source) the
  dialog offers **Download**, which opens the build for this platform in
  the browser, and **Release Notes**, which opens the release's page.
- A release build (a bundle, an install) also checks by itself once a day,
  and says something only when there is a newer release. Runs from source
  check only when asked.

The starter checks `penso/moonframe`.

<img src="screenshots/check-for-updates.png" alt="The update dialog: Moonframe 20261028.1 is available, You have 0.1.0, with Download, Release Notes and Later buttons" width="380">

## Naming the repository

An app on the library names it, with its version:

```rust
moonframe::Moonframe::new("notes")
    .title("Notes")
    .version(env!("CARGO_PKG_VERSION"))
    .updates("example/notes")
    .run()
```

An app package (a profile) names it in `moonframe.toml`, with the version it
compares releases with; `just bundle-app NAME` writes that version into the
bundle too:

```toml
[app]
name = "notes"
title = "Notes"
version = "1.2.0"
updates = "example/notes"
```

A profile without `updates` has no Check for Updates: Moonframe's own
updates are the starter's. A profile running in the starter's `.app` (not a
bundle of its own) never replaces that `.app`; it opens the release instead.

A shell can also place the check itself, in a menu or on a button, with the
command `check-for-updates`:

```lua
{ title = "Help", items = { { label = "Check for Updates…", command = "check-for-updates" } } }
```

## Releasing so apps find the update

Follow GitHub's usual release path:

1. Bump the version (`Cargo.toml`, or the manifest's `version`).
2. Tag the commit `v1.2.3` (the `v` is optional) and publish a release for
   the tag. Drafts and pre-releases are never offered.
3. Attach the builds, named for their platform:

   | Platform | Name it like | Picked by |
   | --- | --- | --- |
   | macOS | `Notes-1.2.3-universal-apple-darwin.dmg`, or a `.zip` naming the Mac (`darwin`, `macos`, `mac`) | `.dmg` first; `universal` or this Mac's architecture (`aarch64`/`arm64`, `x86_64`) |
   | Linux | `Notes-1.2.3-x86_64-unknown-linux-gnu.tar.gz`, `.AppImage` | `linux` and the architecture |
   | Windows | `Notes-1.2.3-x86_64-pc-windows-msvc.zip`, `.msi` | `windows`/`msvc` and the architecture |

   A build naming only another architecture is never picked.
4. Optionally attach `SHA256SUMS` (`sha256sum * > SHA256SUMS`): a download
   that does not match its line is refused.

Moonframe's own release workflow (see [releasing.md](releasing.md)) already
publishes assets named this way, with `SHA256SUMS`.

The disk image must hold the `.app` at its top, with the same bundle id as
the running one (`dev.moonframe.app.NAME` for a profile's bundle) and a
signature `codesign --verify` accepts. When the running app is signed with
a Developer ID, the update must be signed by the same team: an update from
anyone else, or one signed ad hoc, is refused. An app signed ad hoc names no
signer to hold its updates to, so for it the checksum and bundle id are the
only checks: sign apps others download with a Developer ID (Gatekeeper
wants that and notarization anyway).
The app's folder must be writable by the user for the swap; when it is
not, the release's page opens instead.

# What is stable

Moonframe is at 0.1, and depends on a pre-release GPUI. Nothing is frozen, and
there is no compatibility promise yet. This says what to expect, so you can
choose where to lean. Pin a commit: `Cargo.lock` does this for a library app.

| Surface | Expect |
| --- | --- |
| Luau `ui.*`, `mod`, messages, `app:*` requests | Stable in spirit. Keys are added; a rename is rare, and `--check` reports it. |
| Components (`use "name"`) | Props are added; looks change. |
| Provider lines (`process`, `socket`) and `provider.toml` | Stable: other programs depend on them. |
| Frame ring layout | Versioned by its magic; a change makes a new version. |
| `moonframe.toml`, `PAYLOAD`, bundle scripts | Fairly stable. |
| Rust `HostView`, `Native`, `AsyncNative`, `Moonframe` builder | Mostly stable; signatures may change. |
| `moonframe::gpui_kit` (GPUI itself) | Changes with every GPUI update. Keep Rust views small. |
| `ui.video` props | New, and most likely to move. |
| Stylesheet classes of components | Change with the components. |

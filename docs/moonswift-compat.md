# MoonSwift compatibility

[MoonSwift](https://github.com/moonframehq/moonswift) runs Moonframe's Luau
mods natively on iPhone, iPad and macOS with SwiftUI. A mod that stays inside
the shared core, the files, `mod { ... }`, topics, providers, the `app` keys
and the elements and tokens both hosts draw, runs unchanged on both. The
shared core is written down in MoonSwift's
[`plans/moonframe-compat.md`](https://github.com/moonframehq/moonswift/blob/main/plans/moonframe-compat.md).

A portable mod picks a layout from `app.renderer` (`gpui` or `rune` here,
`swiftui` on MoonSwift) and `app.window.platform` (`ios` on a phone or
tablet), which `types/moonframe.d.luau` declares.

## The conformance suite

`conformance/` holds small apps in the shared core, each with the steps to
play and what every mod should draw after them. `conformance/README.md` is
the format. MoonSwift keeps the same folder and plays it in its own tests;
Moonframe's copy is the reference, which MoonSwift's `just conformance-sync`
copies.

Moonframe plays it in `crates/moonframe-core/src/tests/conformance.rs`: each
case opens on its folder alone, light, with nothing saved and no providers,
and every step must leave no problem.

```sh
just conformance        # play every case
just conformance bless  # fill each `expect` left null from what Moonframe draws
```

To add a case, add a folder under `conformance/cases/`, give its `expect`
trees as `null`, bless it, read what was written, and copy the folder to
MoonSwift, whose tests must agree without blessing. To change what a case
expects, set the tree back to `null` and bless again.

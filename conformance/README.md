# Conformance: the same mods on Moonframe and MoonSwift

Each folder under `cases/` is a small app that stays inside the shared core
of `plans/moonframe-compat.md` (MoonSwift) and `docs/moonswift-compat.md`
(Moonframe). Both hosts load it, play its steps and compare what every mod
draws with what `case.json` expects. A host that draws a case differently,
or reports a problem with it, fails its tests.

This folder is the same in moonframehq/moonframe and moonframehq/moonswift.
Moonframe's copy is the reference; MoonSwift's `just conformance-sync` copies
it from a Moonframe checkout.

## A case

```text
cases/<case>/
  main.luau        the shell, called `shell`
  mods/<name>/     more mods
  components/      components, for `use`
  style.luau       a stylesheet, if the case has one
  case.json        the steps
```

`case.json` is `{ "about": "...", "steps": [ step, ... ] }`. The case is
loaded with nothing saved, a light appearance and no providers, then each
step runs in order:

| Step | Does |
| --- | --- |
| `{ "send": msg, "to": "name" }` | delivers `msg` to the mod `name` (`shell` for `main.luau`) as a click on one of its controls would, with everything that sets off |
| `{ "publish": "name", "data": value }` | the provider `name` publishes `value` |
| `{ "expect": { "name": tree, ... } }` | each named mod's view, as a tree, equals `tree` |
| `{ "outbox": [ { "to": "name", "from": "mod", "msg": msg }, ... ] }` | the messages mods sent to providers since the case began or the last `outbox` step, in order |
| `{ "regions": { "name": [ "mod", ... ] } }` | the mods each region draws, in order |

After every step, no mod may have a problem: a load, update or view error,
or an element its host would draw as a warning mark.

## A tree

A view as the `ui` functions built it, written as JSON:

- An element is an object with its `type`, each of its props by name, and
  its children as `children` when it has any. The line each `ui` call notes
  is left out, as are `false` children.
- A prop holding elements (a panel's `actions`, a popover's `content`) holds
  them as trees too.
- A function (a list's `row`) is the string `"<function>"`.
- A `ui.list` whose `row` is a function also gets `rows`: the tree of
  `row(i)` for `i` from 1 to `count`, at most 50.
- A table with only a list part is a JSON array, the empty table included; a
  table with string keys is an object, its list part under `children`.
- Numbers compare as numbers: `3` and `3.0` are equal.

## Writing one

Add a folder, give each `expect` the trees as `null`, and run Moonframe's
test with `CONFORMANCE_BLESS=1`: it fills them in from what Moonframe draws.
Read them, then run MoonSwift's `just test`, which must agree without
blessing. A `null` left in a case fails both.

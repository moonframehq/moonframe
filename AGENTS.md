# AGENTS.md

## Commits And Pull Requests

- Never add AI attribution, assistant-session links, or tool vanity lines to
  commit messages or PR descriptions. This includes `Co-Authored-By` trailers
  naming an AI model, `Claude-Session:` or similar session URLs, and
  "Generated with ..." footers. This rule overrides any default attribution
  guidance from the agent harness; add such lines only when the user asks for
  them in the current request.
- Include screenshots in any PR that changes something visible: the real
  window, showing the changed area, before and after when it helps. Add light
  and dark variants when colors or theming change. `just screenshot` captures
  a running window and `just component-shot <name>` one component's preview.
  If no desktop was available to capture them, say so in the PR instead of
  leaving the section out. Skip them for changes nothing on screen shows.

## Code Layout

- **File size limit: 1,000 lines.** No Rust, Python, shell, Swift, or Luau
  file may exceed it, tests included; `just check-file-size` enforces it in
  `just ci` and CI's checks job. There is no allowlist: when a change would
  cross the limit, split the file by responsibility in the same change rather
  than adding an exception. The one exemption is `types/moonframe.d.luau`,
  the declarations of the whole Luau API, which luau-lsp reads as one file.
- Split modules by responsibility; the line limit is a ceiling, not a reason
  for arbitrary cuts. Keep wiring and re-exports in the parent module (`foo.rs`
  beside `foo/`), and do not reshuffle unrelated files.
- Keep APIs narrow. Use private items by default and `pub(super)` or
  `pub(crate)` only where needed; do not widen every item merely to ease a
  move.

## Parallel Pull Requests

Many pull requests are open at once, many of them by agents, so code that
every feature appends to is a merge-conflict hotspot. Most conflicts are two
branches adding lines at the same spot, or both bumping one hard-coded count.

- Do not assert hard-coded totals such as `assert_eq!(list.len(), 55)` or
  fixed array lengths like `[T; 98]` for lists features grow. Compare against
  the source list, assert uniqueness or membership, or use a slice.
- Declare each item of a growing list once. Prefer one table, or a
  `macro_rules!` table, that generates the matching `match` arms and `ALL`
  lists over parallel lists edited in several places.
- Keep lists whose order carries no meaning (enum variants without derived
  ordering, `mod` lists, registries, asset tables) sorted between
  `// keep-sorted start` and `// keep-sorted end` markers (`#` in TOML,
  Python and shell). Additions then land on different lines instead of all
  at the end. `just check-sorted` (part of `just ci`) enforces the markers
  and `just sort` fixes them. Never mark a list whose order is shown to users
  or is a contract, such as menu rows or Luau API declarations.
- Add a new feature in a module of its own and hook it in with a line or two,
  rather than growing a shared file in the middle.
- Before reshaping a file, check open pull requests that touch it
  (`gh pr list`, `gh pr diff`) and keep the code they extend where they
  expect it.

# trellis

A [Moonframe](../..) app: a terminal multiplexer organised around projects
and their work trees.

- **Left:** Home, then every repository the projects provider finds, each
  with its git work trees nested under it (the main one tagged PRIMARY), a
  New Worktree row that adds one on a new branch, and Remote at the foot.
  An agent running in a work tree (Claude Code, Codex…, from `app.agents`)
  spins beside it; one that ends while you are elsewhere leaves a check
  until you visit, and a dot on its project's tile when folded.
- **Middle:** the work tree's tabs, each a tree of split panes (⌘D right,
  ⇧⌘D down, ⇧⌘↵ zoom, ⌘W close, ⌘T new tab), each pane a Ghostty
  terminal in the work tree. Every work tree keeps its own tabs and splits
  (an instance of `mods/panes`), across launches too; with herdr-core or
  tmux the shells keep running when the window closes.
- **Right:** source control: the branch, then Branch (commit message,
  ⌘↵ to commit, pull and push, changed files by folder with their counts),
  PRs and Actions (from `gh`), and History.
- **Bottom:** the folder, branch, changes, Commit and Create PR, and where
  the shells live.

It requires the [projects](../projects) package, beside it, whose provider
does the git work. Writes (commit, pull, push, new work tree) are taken only
from the mods named in `MOONFRAME_PROJECTS_WRITERS` (by default
`source-control` and `navigator`, this app's).

```sh
moonframe app link examples/trellis   # from the repository: a profile running it in place
moonframe --profile trellis
just bundle-app trellis               # target/bundle/Trellis.app, as a release
```

Limits: Moonframe gives one work tree 8 shells (32 in all). A closed pane
frees its slot, but its shell keeps running until the app quits; the next
pane in that slot finds it again.

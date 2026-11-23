# Strata

A git browser on Moonframe: your repositories, the history of the one you
open as a graph, and each commit's message and diff. It only reads: it
never checks out, commits or pushes.

```sh
moonframe app link examples/strata
moonframe --profile strata
just bundle-app strata               # target/bundle/Strata.app
```

It requires the [projects](../projects) package beside it.

- **Repositories** (left, ⌘B or the panel button folds it): those opened
  lately, then every git repository the projects provider finds under your
  project folders, grouped by folder, with its branch and how many files
  changed. A click opens one; the folder button, or File > Open
  Repository… (⌘O), opens any other. Started outside a repository (from the
  Finder), it reopens the last one, or asks for one.
- **History** (⌘1): the commits of every branch, tag and HEAD, newest first,
  as a graph with their branches and tags, up to 500. Up and Down step
  through them. The field above (⌘F) finds commits by message, author or
  id among them.
- **Branches** (⌘2, or the branch button in the title bar): the local
  branches, most recent first, with how far each is from its upstream. A
  click shows the commit at its tip.
- **The commit** (right): its subject and message, author, age and id
  (click it to copy the full one), its parents (a click goes there) and the
  branches and tags on it, then every file it changed as a diff, one open
  at a time. A merge shows what it brought against its first parent.
  ⇧⌘D, or the header's switch, puts the old and new lines side by side.

The data comes from the built-in `git` provider (`crates/moonframe-git`):
the log with `{ log = 500 }`, a commit with `{ show = sha }` and one of its
files' diff with `{ show_diff = path }`. `providers/git` names the
repositories mod among its `writers`, which only lets it open a
repository.

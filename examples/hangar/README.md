# hangar

A [Moonframe](../..) app for running several coding agents on one repository,
each on a branch and a worktree of its own:

- **Left:** the bays, one per worktree: what runs there, changed files, how far
  it is from its upstream, and its pull request.
- **Middle:** a terminal in the bay you pick, kept while you visit others. Start
  any agent CLI there.
- **Right:** that bay's changes, over the repository's history as a graph.

Hangar opens the repository it is started in. Started anywhere else, as the
app is from the Finder, it reopens the last repository it showed, or asks for
one: **Open Repository…** picks a folder (one outside any git repository is
refused, with the reason), and the repositories opened before are listed
under it. **File > Switch Repository…** (⇧⌘O) goes to another one later, and
**File > Open Repository…** (⌘O) to one not in the list; right-click a recent
repository to drop it from the list.

**+** makes a new bay from a branch name. Right-click to remove one: its branch
stays, and git refuses while it has uncommitted changes.

A bay's dot comes from its shell: a spinner while a command runs, and out of
sight a dot once it finishes or a program prints a notice (OSC 9 or 777). zsh
and fish report this by themselves; bash needs a line in `~/.bashrc`.

```sh
moonframe app link examples/hangar   # from the repository: a profile running it in place
cd ~/code/my-project && moonframe --profile hangar
```

Moonframe's releases also ship it as an app of its own, with nothing else to
install (docs/releasing.md):

```sh
brew install moonframehq/tap/moonframe-hangar
```

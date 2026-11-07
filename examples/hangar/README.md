# hangar

A [Moonframe](../..) app for running several coding agents on one repository,
each on a branch and a worktree of its own:

- **Left:** the bays, one per worktree: what runs there, changed files, how far
  it is from its upstream, and its pull request.
- **Middle:** a terminal in the bay you pick, kept while you visit others. Start
  any agent CLI there.
- **Right:** that bay's changes, over the repository's history as a graph.

**+** makes a new bay from a branch name. Right-click to remove one: its branch
stays, and git refuses while it has uncommitted changes.

A bay's dot comes from its shell: a spinner while a command runs, and out of
sight a dot once it finishes or a program prints a notice (OSC 9 or 777). zsh
and fish report this by themselves; bash needs a line in `~/.bashrc`.

```sh
moonframe app link examples/hangar   # from the repository: a profile running it in place
cd ~/code/my-project && moonframe --profile hangar
```

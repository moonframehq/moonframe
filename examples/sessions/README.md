# sessions

A [Moonframe](../..) app for coding sessions across your repositories:

- **Left:** every git repository under your project folders, grouped by
  where it lives, newest first, with the agent running in it, its age, its
  branch and its changes.
- **Middle:** a shell in the session you pick, kept while you visit others,
  with its branch, changes and last commit.
- **Right:** its changes (last commit, uncommitted, or the whole branch) as
  a tree of folders, or all its files; a file opens in a sheet with its
  code highlighted.

It requires the [projects](../projects) package, beside it.

```sh
moonframe app link examples/sessions   # from the repository: a profile running it in place
moonframe --profile sessions
just bundle-app sessions               # target/bundle/Sessions.app, as a release
```

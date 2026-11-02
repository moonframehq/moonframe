# switchyard

A [Moonframe](../..) workspace for coding agents:

- **Left:** your repositories and their worktrees, with each worktree's
  branch, changes and age, and a dot while an agent works in it.
- **Middle:** chats with coding agents (Claude, Codex, Copilot, Gemini,
  OpenCode), one tab each. A chat runs in herdr-gpui's `herdr-acp`
  program, which the built-in `chat` provider starts with no view of its
  own and attaches to over herdr's chat protocol, so a chat keeps going
  when the app quits and comes back at the next launch. The composer picks
  the chat's model, thinking level and mode, and turns on auto-accept.
- **`!` commands:** a message starting with `!` runs in the worktree
  through the built-in `shell` provider instead, with paths completed as
  you type. Its card shows the output in colour, live, with Stop (or Esc),
  copy, run again, open a terminal, and **Add to chat**, which sends the
  output with your next message.
- **Under the chat:** the worktree's setup and run scripts, and a terminal
  in it.
- **Right:** the worktree's files, its changes (last commit, uncommitted or
  the whole branch) and its checks (pull requests and workflow runs, from
  `gh`).

It requires the [projects](../projects) package beside it, and Herdr's
`herdr-acp` (in Herdr.app, or on PATH; else set `program` in
`providers/chat/provider.toml`).

```sh
moonframe app link examples/switchyard   # from the repository: a profile running it in place
moonframe --profile switchyard
```

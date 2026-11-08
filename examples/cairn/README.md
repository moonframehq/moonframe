# Cairn

An agent workspace on Moonframe, in the dark: sessions with a coding agent on
the left, the chat in the middle, and a workbench with the session folder's
changes and a terminal on the right.

```sh
moonframe app link examples/cairn
moonframe --profile cairn
just bundle-app cairn               # target/bundle/Cairn.app, as a release
```

It requires the [projects](../projects) package beside it.

- **Sessions** start in a folder: New session offers the folders you used
  and your projects, or any folder. The sidebar lists them, with a spinner
  while the agent works, a mark when it waits on you, and a dot for an answer
  you have not read. It owns the list (renames, deletions, what is unread) and
  shares it on the topic `cairn/sessions`.
- **The chat** sends each message to a coding agent through the built-in
  `acp` provider (`providers/acp`): Claude by default, or Codex, Gemini,
  OpenCode or Copilot, picked in the composer from those installed. Each
  message is one run of the agent in the session's folder, and a follow-up
  sends the earlier requests and answers with it. The transcript shows the
  tools it used, its answer, and Stop while it works. The provider keeps
  the newest 30 runs across launches, so older turns of a long session fall
  away.
- **Permissions**: the agent may read and search without asking; every edit
  or command waits for your answer in the chat, allowed once at most. To let
  it do more, copy `providers/acp` to `~/.config/moonframe/providers/acp`
  and change `allow` there.
- **No agent installed**: the chat, Home and the status bar say so, and what
  to install (`claude-agent-acp`, `codex-acp`, `gemini`, `opencode` or
  `copilot`, or Node, for `npx` to fetch an adapter). A sign-in the agent
  needs is reported in the run.
- **Files Changed** is the session folder's git changes (uncommitted, the
  branch, or the last commit) from the `projects` provider, for folders
  among the git repositories under your project folders; pick a file to read
  it. The **+** opens a terminal in the folder.
- **The composer** completes `@` from the changed files and `/` from its
  commands, attaches files with **+**, and its mic is `dictation`, so
  `providers/speech` lists the chat.
- **Commands** (⌘⇧P, or the status bar) is a palette of every session and
  action.

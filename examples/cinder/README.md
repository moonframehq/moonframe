# cinder

**Cinder**, a [Moonframe](../..) app in the style of a desktop coding-agent app
in code mode, dark. Its mascot is Cinder, a little pixel cinder drawn from a
grid (`components/cinder-sprite.luau`).

- **Title bar:** the traffic lights, the sidebar toggle, back and forward
  through the sessions you opened, and a chat / code switch; the session's
  title; What's new, a terminal, a preview page and a menu on the right.
- **Sidebar:** New, Search (filters the sessions as you type), the sessions you started here with an agent, then
  your CLI sessions grouped by repository, and the agent new sessions get. A
  group folds from its heading. Each CLI session shows a green pull-request
  mark when it opened one, and a laptop (on this computer) or a cloud (its
  folder is gone). Chat mode has no chats yet: it says so.
- **New session:** Cinder on the "Import N CLI sessions" pill, which lists
  the most recent ones to open; the composer at the bottom, with the folder
  to start in, the agent (and the model it last said it uses) under it.
- **A CLI session:** its transcript, read-only: your messages in bubbles,
  the agent's text with lists and code blocks, and its tool calls folded
  into one line each (click to open).
- **A session with an agent:** typing in a new session sends it to a real
  coding agent (Claude by default, or Codex, Gemini, OpenCode or Copilot),
  started in the folder picked under the composer. Its reply streams in
  with its tool calls folded as in a transcript, and its name and model
  show above. Reading is allowed on its own; each edit, command or other
  change asks you first, with the agent's own options (allowing always is
  never offered). The send button turns into Stop while it works. Type
  under it to follow up: each follow-up is a new run of the agent, in the
  same folder, told what was said before.

## The agent

`providers/acp` is the built-in `acp` native provider
(`crates/moonframe-acp`): it starts the agent's Agent Client Protocol
command in the folder you picked, sends your words, and shares what the
run does as `app.acp`; finished sessions are kept and listed again at the
next launch. Its settings let the composer (`mods/canvas`) start runs and
pick their folder, and answer questions from tools that only look, so every
edit still waits on you; a mod can't ask for more. Claude runs through its
ACP adapter, `claude-agent-acp`, or through `npx` when only Node is
installed (the first session downloads it); it uses your Claude Code
sign-in, so run `claude` and `/login` once if it asks. With no agent at all,
the new session page says what to install, and **Look again** checks after
you do. Send feedback (the sparkles) uses the same provider to change this
app.

## Where the CLI sessions come from

`providers/history` (a native provider, `crates/moonframe-transcripts`) reads
the coding-agent CLI's transcripts in `~/.claude/projects` (a folder per
project, a JSONL file per session), **read-only**: the 60 most recent
sessions, titled by the CLI's own title, a summary, or the first thing
typed, and grouped by the repository they ran in (worktrees under their
repository). Opening one reads its transcript (the last 150 turns). It never
writes a file. With no such folder it lists none (Claude sessions started
in Cinder land there too). `CINDER_SAMPLE=1` in the environment shares
sample sessions instead, for screenshots.

```sh
moonframe app link examples/cinder   # from the repository
moonframe --profile cinder
CINDER_SAMPLE=1 moonframe --profile cinder   # sample sessions only
just bundle-app cinder                # target/bundle/Cinder.app
```

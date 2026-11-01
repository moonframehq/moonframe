# cinder

**Cinder**, a [Moonframe](../..) app in the style of a desktop coding-agent app
in code mode, dark. Its mascot is Cinder, a little pixel cinder drawn from a
grid (`components/cinder-sprite.luau`).

- **Title bar:** the traffic lights, the sidebar toggle, back and forward
  through the sessions you opened, and a chat / code switch; the session's
  title; What's new, a terminal, a preview page and a menu on the right.
- **Sidebar:** New, Search (filters the sessions as you type), the places
  (not in this example), then your sessions grouped by repository. A group
  folds from its heading. Each session shows a green pull-request mark when
  it opened one, and a laptop (on this computer) or a cloud (its folder is
  gone). In chat mode the sidebar lists sample chats instead.
- **New session:** Cinder on the "Import N CLI sessions" pill, which lists
  the most recent ones to open; the composer at the bottom, with the folder
  to start in, the effort and Auto under it.
- **A session:** its transcript, read-only: your messages in bubbles, the
  agent's text with lists and code blocks, and its tool calls folded into
  one line each (click to open).
- **The demo agent:** typing in a new session runs a **scripted demo**, not
  a model: it answers by rules (tests, a bug, anything else), labelled as
  such, and writes nothing.

## Where the sessions come from

`providers/history` (a Python process provider) reads the coding-agent CLI's
transcripts in `~/.claude/projects` (a folder per project, a JSONL file per
session), **read-only**: the 60 most recent sessions, titled by the CLI's
own title, a summary, or the first thing typed, and grouped by the
repository they ran in (worktrees under their repository). Opening one
reads its transcript (the last 150 turns). It never writes a file. With no
such folder, or `CINDER_SAMPLE=1` in the environment, it shares sample
sessions (the screenshot uses them).

```sh
moonframe app link examples/cinder   # from the repository
moonframe --profile cinder
CINDER_SAMPLE=1 moonframe --profile cinder   # sample sessions only
just bundle-app cinder                # target/bundle/Cinder.app
```

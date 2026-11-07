# pilot

**Pilot**, a [Moonframe](../..) desktop app for the
[pi](https://github.com/earendil-works/pi) coding agent. It is a shell around
pi, not another agent: each thread is a real `pi --mode rpc` process over pi's
own session files, so threads, models, sign-ins and skills carry over to the
pi CLI and back.

- **Projects are optional:** a thread can work in a project folder, or under
  **No project** in an empty folder of Pilot's own, for a question or a chat.
- **Left:** New thread, the pages, and the threads by day or by project. A
  spinner while one runs, a dot when something is new, a warning when it needs
  you. Right-click to pin, rename, archive or delete.
- **Middle:** the conversation. A run's steps show as it goes (tools with
  their output and diffs, thinking, the answer streaming in) and fold under
  one line afterwards, with a card of what that run changed. Fork from any
  prompt, into the same checkout or a new worktree.
- **Composer:** Enter sends, or queues a follow-up during a run; ⌘Enter steers
  the run; `@` completes files; `/` completes commands, skills and prompts;
  `!command` runs in the shell, and `!!command` keeps it out of the context.
  Attach or drop pictures. Pick the model and thinking level per thread.
- **Right:** the workbench. **Changes** compares uncommitted work, the branch
  against its base, or a single run, with staging file by file and a commit
  box. **Files** browses and edits. **Terminal** is a real shell in the
  checkout. **Worktrees** lists them.
- **Pages:** Scheduled (prompts pi runs by itself while the app is open),
  Skills and Extensions (on and off, in pi's own settings), and Settings.
- **Keys:** ⌘K search, ⌘P files, ⌘N new thread, Ctrl-Tab recent threads,
  ⌘1 to ⌘9 sidebar threads, ⌘J terminal, ⌘R changes, ⌘E files, ⌘F find,
  ⌘⇧M model, ⌘. stop, ⌥⌘B workbench.
- Notifications and a Dock badge when a background thread finishes, fails or
  needs you. An extension's questions appear above the composer.

```sh
moonframe app link examples/pilot   # from the repository: a profile running it in place
moonframe --profile pilot           # or `just demo` and pick it from the gallery
```

It needs [Node](https://nodejs.org) and pi. It finds pi on your login shell's
PATH or in mise's installs, and offers to install a private copy with npm when
there is none. Connect a model under **Settings**: an API key goes straight
into pi's `auth.json`; a browser sign-in is pi's own `/login`, run in the
terminal tab, as pi only offers it in its terminal interface. Pilot notices
the new credential by itself.

## How it fits together

- `providers/pi` (Node, no dependencies) runs pi and publishes `app.pi`:
  `lib/engine.mjs` is the model, `lib/live.mjs` follows one pi's events,
  `lib/timeline.mjs` turns messages into what the window draws,
  `lib/publish.mjs` keeps that under the host's limits, and `lib/commands.mjs`
  lists the messages it takes. What the app adds to pi (projects, pins,
  schedules, each run's before and after) is in
  `~/.local/share/moonframe/pilot/state.json`; a conversation is only ever in
  pi's session file.
- `providers/repo` (Node) is the checkout of the thread on screen: `app.repo`.
- `mods/watch` is the only mod that hears the pi provider. A message re-runs
  the view of the mod it reaches, and pi publishes on every streamed word, so
  it passes on what really changed. Pages are instances for the same reason: a
  mod's own view runs whenever what it reads changes, drawn or not.

## Tests

`tests/` runs the providers end to end with the real pi on your PATH and a
mock model (`mock-llm.mjs`, a small OpenAI-compatible server), in folders of
their own, so they need no account and touch none of your sessions:

```sh
node --test examples/pilot/tests/*.test.mjs
```

`node examples/pilot/tests/seed.mjs DIR` fills a folder with real threads to
look at, for a window started with `PI_CODING_AGENT_DIR=DIR/agent` and
`PILOT_DATA=DIR/data`.

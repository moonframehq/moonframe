# opencode

A demo agent workspace on Moonframe, in the dark: sessions on the left, a chat
in the middle, and a workbench of tabs on the right. Everything runs on
sample data, and the agent is scripted: it says so when you write to it.

```sh
moonframe app link examples/opencode
moonframe --profile opencode
```

- **Sessions** live in `providers/sessions/sessions.json`, a static provider.
  The sidebar owns the list (renames, new and deleted sessions, what is unread)
  and shares it on the topic `opencode/sessions`.
- **Hello and Echo** are the real part. `mods/hello` is a small service that
  publishes on `opencode/greeter`. Echo asks it by message; the top bar, the
  chat's header and the toast over the composer read what comes back. Write
  `hello Kit` in the chat to reach it from there too, and restart it from the
  Hello tab to see the generation change.
- **The composer** completes `@` from the session's changed files and `/` from
  two commands. Its mic is `dictation`, so `providers/speech` lists the chat.
- **Playground** (⌘⇧D) is a palette of every session and action.

To connect a real agent, replace the canned answer in `mods/chat` with the
built-in `acp` or `chat` provider, as `examples/switchyard` does.
